//! El canal en vivo de los jobs: `GET /api/v1/jobs/stream`.
//!
//! Es la otra ruta de la API que no contesta un documento único: devuelve un flujo
//! `text/event-stream` que no termina hasta que el proceso se apaga. Comparte con
//! el canal de unidades (`GET /api/v1/units/stream`) la excepción a la envoltura
//! única —una respuesta continua no tiene un `data` único—, pero **no** comparte lo
//! que viaja por dentro:
//!
//! * el canal de unidades empuja **instantáneas** completas, idempotentes: el
//!   último evento vale por todos los anteriores;
//! * éste empuja **transiciones**: que un job acaba de nacer o de morir. Un evento
//!   perdido no lo corrige el siguiente —son cosas distintas—, pero tampoco deja un
//!   estado divergente, porque el estado de la unidad viaja por el canal de
//!   unidades.
//!
//! Por eso al conectar **no hay instantánea inicial**: un job «en curso» es
//! efímero y no hay nada que contar hasta que systemd diga que ha creado uno. Lo
//! único que el flujo manda al abrirse es el `retry`.

use std::time::Duration;

use axum::extract::State;
use axum::response::Sse;
use axum::response::sse::{Event, KeepAlive};
use futures_util::stream::{self, Stream};
use tokio::sync::{broadcast, watch};

use crate::api::response::ApiResponse;
use crate::state::AppState;
use crate::systemd::ProgresoDeJob;

/// El nombre del evento que viaja por el flujo.
///
/// El panel lo escucha por su nombre (`addEventListener("job", …)`): sin nombre,
/// el navegador lo entregaría como un `message` genérico y el cliente no podría
/// distinguirlo de un latido mal formado.
const EVENTO: &str = "job";

/// Cuánto espera el cliente antes de reconectar si el flujo se corta.
///
/// Tres segundos es lo que el navegador usará entre reintentos tras una caída de
/// red: ni un martilleo contra el servidor ni una espera que deje el panel muerto.
const RETRY: Duration = Duration::from_secs(3);

/// Cada cuánto se manda un latido cuando no hay nada que contar.
///
/// Un proxy con la conexión ociosa la corta —algunos a los pocos segundos—, así que
/// el flujo manda un comentario de vez en cuando para que la conexión no parezca
/// muerta. Quince segundos es el valor por defecto de axum y el que usan la mayoría
/// de los proxies como umbral.
const LATIDO: Duration = Duration::from_secs(15);

/// `GET /api/v1/jobs/stream` — el progreso de los jobs, en vivo.
///
/// No hay nada que calcular antes de abrir el flujo, así que la ruta no puede
/// fallar por el gestor: el canal ya existe cuando el cliente se construye y lo
/// único que se hace aquí es suscribirse a él. El primer cuadro —el del `retry`— lo
/// pone el propio flujo.
pub async fn stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, std::io::Error>> + Send + 'static> {
    flujo(state.trabajos.observar(), state.apagado.clone(), LATIDO)
}

/// Compone la respuesta SSE a partir del canal de trabajos y el cierre.
///
/// El intervalo del latido entra por parámetro —y no se lee del módulo— para que la
/// prueba pueda pedir uno de milisegundos y medir el latido sin esperar quince
/// segundos.
fn flujo(
    trabajos: broadcast::Receiver<ProgresoDeJob>,
    apagado: watch::Receiver<bool>,
    latido: Duration,
) -> Sse<impl Stream<Item = Result<Event, std::io::Error>> + Send + 'static> {
    Sse::new(eventos(trabajos, apagado))
        .keep_alive(KeepAlive::new().interval(latido).text("latido"))
}

/// El flujo de eventos: primero el `retry`, después una por cada transición.
///
/// Termina cuando el proceso se apaga (el canal de cierre lo dice) o cuando la
/// fuente de trabajos desaparece. Terminar es lo que impide que el apagado ordenado
/// se quede colgado esperando a una conexión que nunca acabaría.
fn eventos(
    trabajos: broadcast::Receiver<ProgresoDeJob>,
    apagado: watch::Receiver<bool>,
) -> impl Stream<Item = Result<Event, std::io::Error>> + Send + 'static {
    stream::unfold(
        (true, trabajos, apagado),
        |(mut retry_pendiente, mut trabajos, mut apagado)| async move {
            // El `retry` sale en la primera vuelta, sin esperar a ningún job: es lo
            // único que hay que decirle a un cliente que acaba de conectar, y así lo
            // sabe antes de que ocurra la primera transición —que puede tardar—.
            if retry_pendiente {
                retry_pendiente = false;

                return Some((Ok(solo_retry()), (retry_pendiente, trabajos, apagado)));
            }

            loop {
                // El `select!` produce **un** valor y su ámbito se cierra antes de
                // tocar los canales: así el préstamo del `wait_for` no impide
                // devolver los receptores al estado del flujo.
                let siguiente = tokio::select! {
                    biased;

                    // El cierre se mira primero: apagándose no se espera a más
                    // transiciones, se termina el flujo.
                    cierre = apagado.wait_for(|apagado| *apagado) => {
                        // Si el disparador se soltó (`Err`), tampoco hay a quién
                        // servir: el flujo acaba igual.
                        let _ = cierre;
                        None
                    }
                    aviso = trabajos.recv() => match aviso {
                        Ok(progreso) => Some(progreso),
                        // El cliente se quedó atrás y el canal tiró transiciones
                        // viejas: no hay nada que recuperar —cada evento es un
                        // hecho—, y el estado que dejaron lo entrega el canal de
                        // unidades.
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        // La fuente se fue: no habrá más transiciones y el flujo
                        // acaba.
                        Err(broadcast::error::RecvError::Closed) => None,
                    },
                };

                let progreso = siguiente?;

                return Some((evento(progreso), (retry_pendiente, trabajos, apagado)));
            }
        },
    )
}

/// El cuadro de apertura: sólo el `retry`, sin datos ni nombre de evento.
///
/// No es una instantánea —no lleva ningún job—: es la forma de decirle al cliente
/// cuánto esperar antes de reconectar sin tener que esperar a la primera transición.
/// Los navegadores procesan el `retry` y no despachan ningún evento, porque un
/// evento sin `data` no los dispara.
fn solo_retry() -> Event {
    Event::default().retry(RETRY)
}

/// Traduce el progreso de un job a un evento SSE con la envoltura `{data}`.
///
/// Es la misma envoltura que el resto de la API —y que el canal de unidades—, así
/// que el `data` de un evento de job es idéntico a lo que el panel esperaría si lo
/// pidiera por HTTP: un sólo tipo para tratarlo.
///
/// El evento **no** repite el `retry`: es un ajuste de la conexión, no del evento,
/// y ya lo fijó el cuadro de apertura. Repetirlo en cada transición no cambiaría
/// nada —el navegador se queda con el último— y añadiría ruido a cada cuadro.
fn evento(progreso: ProgresoDeJob) -> Result<Event, std::io::Error> {
    // Serializar un identificador, un nombre y dos cadenas no puede fallar, pero si
    // fallara no se puede fingir que hay evento: se devuelve el fallo y el flujo
    // termina, que es lo único honesto.
    let cuerpo = serde_json::to_string(&ApiResponse::new(progreso)).map_err(|error| {
        tracing::error!(%error, "no se pudo serializar el progreso de un job");
        std::io::Error::other(error)
    })?;

    Ok(Event::default().event(EVENTO).data(cuerpo))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::BodyDataStream;
    use axum::http::{Method, StatusCode, header};
    use axum::response::IntoResponse;
    use futures_util::StreamExt;
    use tokio::sync::broadcast;

    use crate::api::response::ApiResponse;
    use crate::systemd::{FaseDeJob, ProgresoDeJob, ResultadoDeJob, UnitName};
    use crate::test_support::{
        cookie_de_sesion, entorno, entorno_oidc, header_texto, peticion, peticion_con_cookie,
        peticion_cruda, peticion_cruda_con_cookie,
    };

    use super::{LATIDO, RETRY, flujo};

    /// El plazo de las esperas: de sobra para un flujo en memoria, corto para que
    /// una prueba rota no cuelgue la suite.
    const ESPERA: Duration = Duration::from_secs(5);

    /// Lee del cuerpo hasta tener un cuadro SSE completo —termina en línea en
    /// blanco—.
    async fn leer_cuadro(cuerpo: &mut BodyDataStream) -> String {
        let mut cuadro = String::new();

        loop {
            let trozo = tokio::time::timeout(ESPERA, cuerpo.next())
                .await
                .expect("el flujo tiene que mandar algo antes del plazo")
                .expect("el flujo sigue abierto")
                .expect("el cuerpo del flujo no puede fallar");

            cuadro.push_str(&String::from_utf8_lossy(&trozo));

            if cuadro.contains("\n\n") {
                return cuadro;
            }
        }
    }

    /// El JSON del campo `data` de un cuadro SSE.
    fn data_del_cuadro(cuadro: &str) -> serde_json::Value {
        let json = cuadro
            .lines()
            .find_map(|linea| linea.strip_prefix("data: "))
            .unwrap_or_else(|| panic!("el cuadro no traía campo `data`: {cuadro:?}"));

        serde_json::from_str(json).expect("el `data` del evento es JSON")
    }

    /// El progreso de una acción de prueba sobre una unidad de prueba.
    fn progreso_de_prueba(fase: FaseDeJob) -> ProgresoDeJob {
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        match fase {
            FaseDeJob::New => ProgresoDeJob::nuevo(7, unidad, "start"),
            FaseDeJob::Done => {
                ProgresoDeJob::terminado(7, unidad, "start", ResultadoDeJob::from_dbus("failed"))
            }
        }
    }

    /// Escenario «Stream de trabajos disponible»: el flujo se abre con su tipo de
    /// contenido, que es lo que hace que el navegador lo trate como `EventSource`.
    #[tokio::test]
    async fn el_flujo_de_trabajos_se_anuncia_como_text_event_stream() {
        let entorno = entorno().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/jobs/stream").await;

        assert_eq!(respuesta.status(), StatusCode::OK);
        assert_eq!(
            header_texto(&respuesta, header::CONTENT_TYPE).as_deref(),
            Some("text/event-stream"),
            "el flujo tiene que anunciarse como text/event-stream"
        );

        // El primer cuadro se lee para que el flujo quede ejercido de verdad.
        let mut cuerpo = respuesta.into_body().into_data_stream();
        let _ = leer_cuadro(&mut cuerpo).await;
    }

    /// Escenario «Sin instantánea inicial»: al conectar no llega ningún job —sólo
    /// el cuadro del `retry`—, porque el canal es de transiciones y un job en curso
    /// es efímero.
    #[tokio::test]
    async fn el_primer_cuadro_solo_indica_el_retry_y_no_lleva_jobs() {
        let entorno = entorno().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/jobs/stream").await;
        let mut cuerpo = respuesta.into_body().into_data_stream();

        let primero = leer_cuadro(&mut cuerpo).await;

        assert!(
            primero.contains(&format!("retry: {}", RETRY.as_millis())),
            "el flujo tiene que indicar el retry con el que reconectar: {primero:?}"
        );
        assert!(
            !primero.contains("event: job"),
            "no puede haber instantánea inicial de jobs: {primero:?}"
        );
        assert!(
            !primero.contains("data: "),
            "el cuadro del retry no lleva datos: {primero:?}"
        );
    }

    /// Escenario «Job recién creado» y «Job terminado»: cada transición publicada
    /// en el canal sale como un evento `job` con su fase y su resultado.
    #[tokio::test]
    async fn cada_transicion_publicada_sale_como_un_evento_job() {
        let entorno = entorno().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/jobs/stream").await;
        let mut cuerpo = respuesta.into_body().into_data_stream();

        // El primero es el cuadro del `retry`: se consume para llegar a los eventos.
        let _ = leer_cuadro(&mut cuerpo).await;

        entorno
            .trabajos()
            .emitir(progreso_de_prueba(FaseDeJob::New));
        entorno
            .trabajos()
            .emitir(progreso_de_prueba(FaseDeJob::Done));

        let alta = leer_cuadro(&mut cuerpo).await;
        assert!(
            alta.contains("event: job"),
            "el evento tiene que ir nombrado `job`: {alta:?}"
        );
        assert_eq!(data_del_cuadro(&alta)["data"]["phase"], "new");

        let baja = leer_cuadro(&mut cuerpo).await;
        assert_eq!(data_del_cuadro(&baja)["data"]["phase"], "done");
        assert_eq!(
            data_del_cuadro(&baja)["data"]["result"],
            "failed",
            "el resultado del job viaja en el evento de baja: {baja:?}"
        );
    }

    /// Escenario «El stream de trabajos no usa la envoltura de documento único»: el
    /// `data` de cada evento es la misma envoltura `{data}` que el resto de la API,
    /// con la forma del progreso del job.
    #[tokio::test]
    async fn el_data_del_evento_usa_la_envoltura_de_la_casa() {
        let entorno = entorno().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/jobs/stream").await;
        let mut cuerpo = respuesta.into_body().into_data_stream();
        let _ = leer_cuadro(&mut cuerpo).await;

        let progreso = progreso_de_prueba(FaseDeJob::Done);
        entorno.trabajos().emitir(progreso.clone());

        let cuadro = leer_cuadro(&mut cuerpo).await;
        let de_la_casa =
            serde_json::to_value(ApiResponse::new(&progreso)).expect("el progreso se serializa");

        assert_eq!(
            data_del_cuadro(&cuadro),
            de_la_casa,
            "el `data` del evento tiene que ser la misma envoltura `{{data}}` del resto"
        );
    }

    /// Escenario «Latido y reconexión»: sin transiciones, el flujo sigue mandando
    /// latidos para que un proxy no dé la conexión por muerta.
    ///
    /// El latido se mide con un intervalo de milisegundos para no esperar los quince
    /// segundos de producción: es la razón por la que el intervalo entra por
    /// parámetro.
    #[tokio::test]
    async fn sin_transiciones_el_flujo_manda_latidos() {
        let entorno = entorno().await;
        let state = entorno.state().clone();

        let respuesta = flujo(
            state.trabajos.observar(),
            state.apagado.clone(),
            Duration::from_millis(1),
        )
        .into_response();

        let mut cuerpo = respuesta.into_body().into_data_stream();

        let retry = leer_cuadro(&mut cuerpo).await;
        assert!(
            retry.contains("retry: "),
            "el flujo tiene que indicar el retry: {retry:?}"
        );

        let latido = leer_cuadro(&mut cuerpo).await;
        assert!(
            latido.contains(": latido"),
            "sin transiciones, el flujo tiene que mandar un latido: {latido:?}"
        );
    }

    /// Un cliente **atrasado** no rompe el flujo ni le inventa un evento: el canal
    /// tira las transiciones viejas (`Lagged`), que no se pueden recuperar —cada una
    /// es un hecho, no un estado—, y el flujo sigue vivo entregando la siguiente.
    ///
    /// El atraso se provoca con un canal de capacidad uno y dos transiciones
    /// seguidas sin que nadie lea: la primera se queda sin sitio y el canal la
    /// descarta. Así la prueba no depende del tamaño del canal de producción ni de
    /// adivinar cuántas emisiones hacen falta para desbordarlo.
    #[tokio::test]
    async fn un_cliente_atrasado_no_rompe_el_flujo() {
        let entorno = entorno().await;
        let (emisor, receptor) = broadcast::channel(1);

        let respuesta = flujo(
            receptor,
            entorno.state().clone().apagado.clone(),
            Duration::from_millis(1),
        )
        .into_response();

        let mut cuerpo = respuesta.into_body().into_data_stream();

        // El cuadro del `retry` se consume para llegar a los eventos.
        let _ = leer_cuadro(&mut cuerpo).await;

        // Dos transiciones seguidas con el flujo sin leer: la capacidad de una se
        // queda corta y la primera cae.
        emisor
            .send(progreso_de_prueba(FaseDeJob::New))
            .expect("el flujo mantiene su receptor vivo");
        emisor
            .send(progreso_de_prueba(FaseDeJob::Done))
            .expect("el flujo mantiene su receptor vivo");

        // El flujo no se ha roto: sigue sirviendo, y lo que entrega es la
        // transición que no se perdió.
        let cuadro = leer_cuadro(&mut cuerpo).await;
        assert!(
            cuadro.contains("event: job"),
            "tras un atraso el flujo tiene que seguir sirviendo eventos: {cuadro:?}"
        );
        assert_eq!(
            data_del_cuadro(&cuadro)["data"]["phase"],
            "done",
            "la transición que sobrevivió al atraso es la última: {cuadro:?}"
        );
    }

    /// Escenario «Apagado con un cliente conectado»: al dispararse el cierre, el
    /// flujo termina en vez de quedarse colgado y bloquear el apagado ordenado.
    #[tokio::test]
    async fn el_cierre_termina_el_flujo_de_trabajos() {
        let entorno = entorno().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/jobs/stream").await;
        let mut cuerpo = respuesta.into_body().into_data_stream();

        // El flujo está vivo: manda su cuadro de retry.
        let _ = leer_cuadro(&mut cuerpo).await;

        entorno.cerrar();

        let fin = tokio::time::timeout(ESPERA, cuerpo.next())
            .await
            .expect("el cierre tiene que terminar el flujo antes del plazo");

        assert!(
            fin.is_none(),
            "tras el cierre, el flujo tiene que acabar sin más eventos: {fin:?}"
        );
    }

    /// Escenario «Método no permitido en el stream de trabajos»: `GET` es el único
    /// admitido, y cualquier otro método contesta `405` con la envoltura habitual.
    #[tokio::test]
    async fn un_metodo_distinto_de_get_da_405_con_la_envoltura() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::POST, "/api/v1/jobs/stream").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// Escenario «Sin sesión en modo OIDC»: la ruta queda bajo el gate, así que sin
    /// cookie contesta `401` con la envoltura y **sin** abrir el flujo.
    #[tokio::test]
    async fn sin_sesion_el_flujo_de_trabajos_da_401_y_no_se_abre() {
        let entorno = entorno_oidc().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/jobs/stream").await;

        assert_eq!(respuesta.status(), StatusCode::UNAUTHORIZED);

        let tipo = header_texto(&respuesta, header::CONTENT_TYPE).unwrap_or_default();
        assert!(
            !tipo.starts_with("text/event-stream"),
            "sin sesión no puede abrirse el flujo: {tipo}"
        );

        let (_, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/jobs/stream").await;
        assert_eq!(cuerpo["error"]["code"], "unauthorized");
    }

    /// Y con sesión válida el flujo se abre: el *gate* protege la ruta sin cerrarla
    /// a quien ha entrado.
    #[tokio::test]
    async fn con_sesion_el_flujo_de_trabajos_se_abre() {
        let entorno = entorno_oidc().await;

        let respuesta = peticion_cruda_con_cookie(
            &entorno.app(),
            Method::GET,
            "/api/v1/jobs/stream",
            &cookie_de_sesion(),
        )
        .await;

        assert_eq!(respuesta.status(), StatusCode::OK);
        assert!(
            header_texto(&respuesta, header::CONTENT_TYPE)
                .is_some_and(|tipo| tipo.starts_with("text/event-stream")),
            "con sesión el flujo debe abrirse"
        );
    }

    /// El 405 de la ruta de trabajos es el de la casa **también** con sesión: el
    /// *gate* no lo convierte en `401`.
    #[tokio::test]
    async fn el_405_de_la_ruta_de_trabajos_se_preserva_con_sesion() {
        let entorno = entorno_oidc().await;

        let (status, cuerpo) = peticion_con_cookie(
            &entorno.app(),
            Method::DELETE,
            "/api/v1/jobs/stream",
            &cookie_de_sesion(),
        )
        .await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// El latido de producción es un valor sensato —ni un martilleo ni una conexión
    /// que un proxy pueda dar por muerta—, y el `retry` también.
    #[test]
    fn el_latido_y_el_retry_son_valores_sensatos() {
        assert!(
            LATIDO >= Duration::from_secs(5),
            "un latido muy corto es ruido"
        );
        assert!(
            LATIDO <= Duration::from_secs(60),
            "un latido muy largo deja que un proxy corte la conexión"
        );
        assert!(
            RETRY >= Duration::from_secs(1),
            "un retry muy corto martillea"
        );
    }

    /// El doble del progreso del entorno está detrás de un `Arc`: es la puerta que
    /// usan las pruebas de este módulo, y si se soltara el canal moriría.
    #[tokio::test]
    async fn el_entorno_da_el_doble_de_trabajos() {
        let entorno = entorno().await;
        let doble: &Arc<crate::systemd::fake::FakeSystemd> = entorno.trabajos();

        doble.emitir(progreso_de_prueba(FaseDeJob::New));
    }
}
