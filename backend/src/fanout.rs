//! El fan-out: convierte los avisos del vigilante en instantáneas publicadas.
//!
//! Es la junta entre las dos mitades del canal en vivo. El vigilante
//! ([`UnitWatcher`]) dice «algo cambió» y no transporta datos; el estado
//! ([`AppState`]) lleva instantáneas completas. Esta tarea es lo que va de uno al
//! otro: por cada aviso, vuelve a preguntar el listado y publica el resultado.
//!
//! Que el aviso no traiga datos es lo que hace que esta tarea sea tan corta: no
//! hay que decidir **qué** cambió —una unidad que arranca, una quadlet nueva, un
//! `daemon-reload`— ni mantener un estado incremental que pueda divergir de
//! systemd. Se recalcula la instantánea entera por el mismo camino que usa el
//! listado, y publicarla es idempotente: quien la reciba sustituye la suya.

use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::state::AppState;
use crate::systemd::UnitWatcher;

/// Arranca la tarea del fan-out y devuelve su trazo.
///
/// Se suscribe **antes** de lanzar la tarea, y eso no es un detalle: el canal de
/// avisos no guarda historia, así que una suscripción perezosa perdería los avisos
/// que llegaran entre el arranque y la primera vuelta del bucle.
pub fn arrancar(estado: AppState, vigilante: &dyn UnitWatcher) -> JoinHandle<()> {
    let avisos = vigilante.subscribe();

    tokio::spawn(ejecutar(estado, avisos))
}

/// El bucle del fan-out: por cada aviso, recalcula la instantánea y la publica.
///
/// Termina —sin dejar nada colgado— cuando el proceso se apaga o cuando el
/// vigilante se va, que es lo que pasa si se cae el bus.
pub async fn ejecutar(estado: AppState, mut avisos: broadcast::Receiver<()>) {
    let AppState {
        systemd,
        eventos,
        mut apagado,
        ..
    } = estado;

    loop {
        tokio::select! {
            biased;

            // El cierre se mira primero: apagándose no se recalcula nada más.
            cierre = apagado.wait_for(|apagado| *apagado) => {
                let _ = cierre;
                return;
            }
            aviso = avisos.recv() => match aviso {
                Ok(()) => {}
                // El fan-out se quedó atrás y el canal tiró avisos viejos. No
                // hay nada que recuperar: el recálculo es de la instantánea
                // entera, así que este aviso ya la pone al día.
                Err(broadcast::error::RecvError::Lagged(omitidos)) => {
                    tracing::debug!(
                        omitidos,
                        "el fan-out se saltó avisos de cambio: se recalcula igual"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => return,
            },
        }

        match systemd.quadlets().await {
            // Sin suscriptores el envío falla, y no es un error: nadie mira.
            Ok(instantanea) => {
                let _ = eventos.send(instantanea);
            }
            Err(error) => tracing::warn!(
                %error,
                "no se pudo recalcular la instantánea de quadlets: el panel no se refrescará \
                 hasta el siguiente cambio"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::systemd::{SystemdClient, UnitAction, UnitName, fake::FakeSystemd};
    use crate::test_support::entorno_con_vigilante;

    use super::arrancar;

    /// El plazo de las esperas de estas pruebas: de sobra para una tarea en
    /// memoria, corto para que una prueba rota no cuelgue la suite.
    const ESPERA: Duration = Duration::from_secs(5);

    /// Escenario «Cambio detectado por señal»: un aviso del vigilante hace que el
    /// fan-out recalcule la instantánea y la publique.
    ///
    /// La instantánea que se comprueba es la **recalculada**, no la del aviso: el
    /// aviso no lleva datos, así que lo que se mide es que el cambio ocurrido en el
    /// doble —una quadlet que arranca— está en lo publicado.
    #[tokio::test]
    async fn un_aviso_publica_la_instantanea_recalculada() {
        let doble = Arc::new(FakeSystemd::demo());
        let (entorno, vigilante) = entorno_con_vigilante(doble.clone()).await;

        // Suscribirse antes de arrancar: el canal no guarda historia.
        let mut instantaneas = entorno.state().eventos.subscribe();
        let fanout = arrancar(entorno.state().clone(), vigilante.as_ref());

        let web = UnitName::parse("podpot-demo-web.service").expect("nombre de la muestra");
        doble
            .action(&web, UnitAction::Start)
            .await
            .expect("el doble conoce la quadlet de muestra");
        doble.notificar();

        let instantanea = tokio::time::timeout(ESPERA, instantaneas.recv())
            .await
            .expect("el aviso tiene que producir una instantánea")
            .expect("el canal sigue vivo mientras el fan-out exista");

        let web = instantanea
            .iter()
            .find(|unidad| unidad.name.as_str() == "podpot-demo-web.service")
            .expect("la instantánea lleva las dos quadlets de muestra");

        assert_eq!(
            web.active_state,
            crate::systemd::ActiveState::Active,
            "la instantánea tiene que reflejar el arranque, no el estado anterior"
        );

        entorno.cerrar();
        tokio::time::timeout(ESPERA, fanout)
            .await
            .expect("el fan-out tiene que terminar al apagarse")
            .expect("el fan-out no puede entrar en pánico");
    }

    /// Escenario «Apagado con un cliente conectado» (la mitad del fan-out): al
    /// dispararse el cierre, el bucle termina en vez de quedarse esperando avisos.
    ///
    /// Es lo que impide que el apagado ordenado se quede colgado: `serve` espera a
    /// las tareas en curso, y una que no termina nunca lo bloquearía.
    #[tokio::test]
    async fn el_cierre_detiene_el_fanout() {
        let doble = Arc::new(FakeSystemd::demo());
        let (entorno, vigilante) = entorno_con_vigilante(doble).await;

        let fanout = arrancar(entorno.state().clone(), vigilante.as_ref());

        entorno.cerrar();

        tokio::time::timeout(ESPERA, fanout)
            .await
            .expect("el cierre tiene que terminar el fan-out")
            .expect("el fan-out no puede entrar en pánico");
    }

    /// Y el cierre gana aunque no llegue ningún aviso: el bucle no espera a que
    /// haya trabajo para mirar el cierre.
    #[tokio::test]
    async fn el_cierre_detiene_el_fanout_aunque_no_haya_avisos() {
        let doble = Arc::new(FakeSystemd::demo());
        let (entorno, vigilante) = entorno_con_vigilante(doble).await;

        // Un estado ya apagado antes de arrancar: el fan-out no puede quedarse
        // esperando un aviso que no va a llegar.
        entorno.cerrar();
        let fanout = arrancar(entorno.state().clone(), vigilante.as_ref());

        tokio::time::timeout(ESPERA, fanout)
            .await
            .expect("un fan-out que arranca apagado tiene que terminar enseguida")
            .expect("el fan-out no puede entrar en pánico");
    }
}
