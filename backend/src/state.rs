//! El estado compartido que se inyecta en los manejadores.
//!
//! Siete piezas y todas se clonan en cada petición —es lo que hace axum con el
//! estado—, así que ninguna copia nada gordo: la configuración va detrás de un
//! `Arc` porque no cambia desde el arranque, el pool de SQLite ya es un `Arc` por
//! dentro —y clonarlo es lo que hace que todas las peticiones hablen por **las
//! mismas** conexiones—, el cliente de systemd, el progreso de los trabajos y la
//! fuente de registros van detrás de un `Arc<dyn …>` porque son objetos que guardan
//! su propia conexión —o su propio canal—, y los dos canales son clones de
//! manejadores de tokio, que por dentro son un `Arc`.

use std::net::SocketAddr;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{broadcast, watch};

use crate::auth::AuthState;
use crate::config::Config;
use crate::registros::FuenteDeRegistros;
use crate::systemd::{ProgresoDeTrabajos, SystemdClient, UnitStatus};

/// Cuántas instantáneas caben en el canal de eventos antes de perder la más vieja.
///
/// Perder una no rompe nada —cada evento lleva la instantánea **completa**, así que
/// el siguiente la sustituye—, pero la capacidad no puede ser cero: con cero no
/// habría eventos y el panel se quedaría congelado sin que nada lo delatara.
pub const CAPACIDAD_DE_EVENTOS: usize = 32;

/// Lo que cada manejador recibe del servidor.
#[derive(Debug, Clone)]
pub struct AppState {
    /// La configuración ya resuelta al arrancar.
    ///
    /// No se vuelve a leer del entorno en ninguna petición: lo que el proceso
    /// decidió al arrancar es lo que vale, y la ruta de salud publica justo eso.
    pub config: Arc<Config>,
    /// El pool de SQLite, con el esquema ya al día.
    ///
    /// Ninguna ruta lo usa todavía —en esta entrega el estado de las unidades es
    /// de systemd, no de podpot—, pero está en el estado porque es el único
    /// sitio por el que pasa todo lo compuesto al arrancar: la primera ruta que
    /// escriba en la base no tendrá que tocar esa composición.
    pub pool: SqlitePool,
    /// El cliente de systemd: el real, o el doble si la configuración lo pidió.
    pub systemd: Arc<dyn SystemdClient>,
    /// El progreso de los trabajos de systemd, para el canal de trabajos.
    ///
    /// Va detrás de un `Arc<dyn …>` por la misma razón que el cliente: las pruebas
    /// no pueden depender de que la máquina que compila tenga un bus en pie, así que
    /// la capa API sólo conoce el puerto —[`ProgresoDeTrabajos`]— y la
    /// implementación real o el doble entran por aquí. Es un **trazo del mismo
    /// objeto** que `systemd`: en el cliente real los dos son el mismo `DbusSystemdClient`.
    pub trabajos: Arc<dyn ProgresoDeTrabajos>,
    /// La fuente de los registros del contenedor de una quadlet.
    ///
    /// Va detrás de un `Arc<dyn …>` por la misma razón que el cliente de systemd:
    /// las pruebas no pueden depender de que haya un Podman con su socket en pie,
    /// así que la capa API sólo conoce el puerto —[`FuenteDeRegistros`]— y la
    /// implementación real o el doble entran por aquí.
    pub registros: Arc<dyn FuenteDeRegistros>,
    /// Las instantáneas que publica el fan-out, para el canal en vivo.
    ///
    /// Lleva **instantáneas del dominio** y no JSON: así el dominio no conoce HTTP
    /// y la capa API traduce con el mismo recorte que el listado
    /// (`GET /api/v1/units`), de forma que el contenido del stream y el de la
    /// consulta son idénticos por construcción. Es `broadcast` porque los
    /// suscriptores son varios —cada cliente del stream— y ninguno manda a los
    /// demás.
    pub eventos: broadcast::Sender<Vec<UnitStatus>>,
    /// El cierre del proceso: cuando vale `true`, el servidor se está apagando.
    ///
    /// Es un `watch` —un valor que se puede leer muchas veces y del que se
    /// suscriben tantos receptores como haga falta— porque el cierre no es un
    /// mensaje que se consume: es un estado que hay que poder mirar en cualquier
    /// momento. Los flujos SSE lo esperan para terminar y no dejar colgado el
    /// apagado ordenado, que espera a las conexiones en curso.
    pub apagado: watch::Receiver<bool>,
    /// El estado de la autenticación: `None` cuando no se monta ninguna capa.
    ///
    /// Es `Some` **sólo** en modo `Oidc`, y por eso es un `Option` y no un campo
    /// vacío: el modo se lee en el tipo, y quien tenga un [`AuthState`] en la
    /// mano sabe que hay autenticación sin consultar la configuración. Lo deriva
    /// [`AppState::new`] de la configuración ya resuelta.
    pub auth: Option<AuthState>,
}

impl AppState {
    /// Compone el estado con las piezas ya construidas.
    ///
    /// No construye ninguna: quien arranca decide de dónde sale cada una —el
    /// `DbusSystemdClient` contra el bus de usuario, o el doble de desarrollo; la
    /// fuente de registros contra el socket de Podman, o el doble de las pruebas—
    /// y el estado se queda con la que le den. Así no hay dos sitios que sepan
    /// leer el entorno, ni una decisión que sólo se pueda tomar con el proceso
    /// entero.
    ///
    /// Los canales entran hechos por la misma razón: sus extremos «de escritura»
    /// —el emisor de eventos y el disparador del cierre— tienen que quedarse fuera
    /// del estado, en quien arranca, y un canal se construye con sus dos mitades a
    /// la vez.
    ///
    /// La autenticación **no** entra por parámetro: se deriva de la configuración
    /// —es lo que hace que el modo se lea en un solo sitio— y sólo existe en modo
    /// `Oidc`.
    pub fn new(
        config: Config,
        pool: SqlitePool,
        systemd: Arc<dyn SystemdClient>,
        trabajos: Arc<dyn ProgresoDeTrabajos>,
        registros: Arc<dyn FuenteDeRegistros>,
        eventos: broadcast::Sender<Vec<UnitStatus>>,
        apagado: watch::Receiver<bool>,
    ) -> Self {
        let auth = AuthState::from_config(&config);

        Self {
            config: Arc::new(config),
            pool,
            systemd,
            trabajos,
            registros,
            eventos,
            apagado,
            auth,
        }
    }

    /// La dirección en la que escucha el proceso.
    pub fn bind_address(&self) -> SocketAddr {
        self.config.bind_address
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::systemd::{FaseDeJob, ProgresoDeJob, UnitName};
    use crate::test_support::entorno;

    /// El estado lleva el puerto del progreso de trabajos: quien se suscribe a él
    /// recibe lo que publique la fuente que se le inyectó —el doble, sin bus—.
    ///
    /// Es lo que mide que el cableado esté hecho: con el puerto fuera del estado, la
    /// capa API no tendría de dónde suscribirse y el endpoint de trabajos no
    /// existiría.
    #[tokio::test]
    async fn el_estado_lleva_el_puerto_del_progreso_de_trabajos() {
        let entorno = entorno().await;
        let mut suscriptor = entorno.state().trabajos.observar();

        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");
        entorno
            .trabajos()
            .emitir(ProgresoDeJob::nuevo(7, unidad, "start"));

        let recibido = tokio::time::timeout(Duration::from_secs(1), suscriptor.recv())
            .await
            .expect("el evento tiene que llegar antes del plazo")
            .expect("el canal sigue vivo mientras el estado exista");

        assert_eq!(recibido.job, 7);
        assert_eq!(recibido.phase, FaseDeJob::New);
    }
}
