//! El estado compartido que se inyecta en los manejadores.
//!
//! Va detrás de un `Arc` en cada pieza para que clonar el estado —que es lo que
//! hace axum en cada petición— no copie la configuración ni el cliente de
//! systemd, y para que todas las peticiones hablen por **la misma** conexión.

use std::net::SocketAddr;
use std::sync::Arc;

use crate::config::Config;

/// Lo que cada manejador recibe del servidor.
#[derive(Debug, Clone)]
pub struct AppState {
    /// La configuración ya resuelta al arrancar.
    ///
    /// No se vuelve a leer del entorno en ninguna petición: lo que el proceso
    /// decidió al arrancar es lo que vale, y la ruta de salud publica justo eso.
    pub config: Arc<Config>,
}

impl AppState {
    /// Construye el estado a partir de la configuración resuelta.
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// La dirección en la que escucha el proceso.
    pub fn bind_address(&self) -> SocketAddr {
        self.config.bind_address
    }
}
