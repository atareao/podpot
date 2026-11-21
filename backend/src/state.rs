//! El estado compartido que se inyecta en los manejadores.
//!
//! Tres piezas y las tres se clonan en cada petición —es lo que hace axum con el
//! estado—, así que ninguna copia nada gordo: la configuración va detrás de un
//! `Arc` porque no cambia desde el arranque, el pool de SQLite ya es un `Arc` por
//! dentro —y clonarlo es lo que hace que todas las peticiones hablen por **las
//! mismas** conexiones— y el cliente de systemd va detrás de un `Arc<dyn …>`
//! porque es el objeto el que guarda la conexión de D-Bus, no cada petición.

use std::net::SocketAddr;
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::config::Config;
use crate::systemd::SystemdClient;

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
}

impl AppState {
    /// Compone el estado con las piezas ya construidas.
    ///
    /// No construye ninguna: quien arranca decide de dónde sale cada una —el
    /// `DbusSystemdClient` contra el bus de usuario, o el doble de desarrollo— y
    /// el estado se queda con la que le den. Así no hay dos sitios que sepan leer
    /// el entorno, ni una decisión que sólo se pueda tomar con el proceso entero.
    pub fn new(config: Config, pool: SqlitePool, systemd: Arc<dyn SystemdClient>) -> Self {
        Self {
            config: Arc::new(config),
            pool,
            systemd,
        }
    }

    /// La dirección en la que escucha el proceso.
    pub fn bind_address(&self) -> SocketAddr {
        self.config.bind_address
    }
}
