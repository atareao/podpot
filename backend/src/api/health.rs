//! La ruta pública de salud.
//!
//! Es lo que un orquestador consulta para saber si el proceso está en pie, así
//! que no lleva autenticación y no toca la base ni el bus de D-Bus: un monitor
//! que dependiera de systemd no podría distinguir «podpot está roto» de
//! «systemd está ocupado».

use axum::extract::State;
use serde::Serialize;

use crate::api::response::ApiResponse;
use crate::state::AppState;

/// El cuerpo de la respuesta de salud.
#[derive(Debug, Serialize)]
pub struct Health {
    /// Estado del proceso: `ok` mientras se pueda atender.
    pub status: &'static str,
    /// La versión del binario, tomada del propio paquete.
    pub version: &'static str,
    /// El modo de escucha, tal y como lo resolvió la configuración.
    pub bind_address: String,
    /// El modo de autenticación activo: `oidc` o `disabled`.
    ///
    /// Es lo que el panel consulta —antes de llamar a ninguna ruta protegida—
    /// para decidir si monta la guarda de sesión: en modo `oidc` sin sesión, la
    /// primera llamada a una ruta de negocio daría `401` sin haber intentado
    /// entrar, así que el modo tiene que poder leerse de una ruta pública.
    pub auth_mode: &'static str,
}

/// `GET /api/v1/health` — el proceso está vivo y escucha donde dice.
pub async fn health(State(state): State<AppState>) -> ApiResponse<Health> {
    ApiResponse::new(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        bind_address: state.bind_address().to_string(),
        auth_mode: state.config.auth.as_str(),
    })
}
