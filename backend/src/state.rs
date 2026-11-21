//! El estado compartido que se inyecta en los manejadores.
//!
//! Cinco piezas y todas se clonan en cada petición —es lo que hace axum con el
//! estado—, así que ninguna copia nada gordo: la configuración va detrás de un
//! `Arc` porque no cambia desde el arranque, el pool de SQLite ya es un `Arc` por
//! dentro —y clonarlo es lo que hace que todas las peticiones hablen por **las
//! mismas** conexiones—, el cliente de systemd va detrás de un `Arc<dyn …>` porque
//! es el objeto el que guarda la conexión de D-Bus, y los dos canales son clones
//! de manejadores de tokio, que por dentro son un `Arc`.

use std::net::SocketAddr;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{broadcast, watch};

use crate::config::Config;
use crate::systemd::{SystemdClient, UnitStatus};

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
}

impl AppState {
    /// Compone el estado con las piezas ya construidas.
    ///
    /// No construye ninguna: quien arranca decide de dónde sale cada una —el
    /// `DbusSystemdClient` contra el bus de usuario, o el doble de desarrollo— y
    /// el estado se queda con la que le den. Así no hay dos sitios que sepan leer
    /// el entorno, ni una decisión que sólo se pueda tomar con el proceso entero.
    ///
    /// Los canales entran hechos por la misma razón: sus extremos «de escritura»
    /// —el emisor de eventos y el disparador del cierre— tienen que quedarse fuera
    /// del estado, en quien arranca, y un canal se construye con sus dos mitades a
    /// la vez.
    pub fn new(
        config: Config,
        pool: SqlitePool,
        systemd: Arc<dyn SystemdClient>,
        eventos: broadcast::Sender<Vec<UnitStatus>>,
        apagado: watch::Receiver<bool>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            pool,
            systemd,
            eventos,
            apagado,
        }
    }

    /// La dirección en la que escucha el proceso.
    pub fn bind_address(&self) -> SocketAddr {
        self.config.bind_address
    }
}
