//! Ayudas compartidas por las pruebas de la casa.
//!
//! Vive detrás de `#[cfg(test)]`: no forma parte del contrato del crate y no
//! compila en el binario de producción.

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::config::Config;
use crate::state::AppState;

/// Lo que una prueba necesita para ejercer la aplicación.
pub struct Entorno {
    state: AppState,
}

impl Entorno {
    /// El router de la aplicación, listo para `oneshot`.
    pub fn app(&self) -> Router {
        crate::api::router(self.state.clone())
    }

    /// El estado, para las pruebas que no pasan por HTTP.
    pub fn state(&self) -> &AppState {
        &self.state
    }
}

/// Un entorno de prueba con la configuración mínima que arranca.
pub async fn entorno() -> Entorno {
    Entorno {
        state: AppState::new(config_de_prueba()),
    }
}

/// La configuración que usan las pruebas: loopback en el puerto por defecto y el
/// doble de systemd, para no depender del bus real.
pub fn config_de_prueba() -> Config {
    Config::from_source(|clave| match clave {
        "BIND_ADDRESS" => Some("127.0.0.1:8080".to_owned()),
        "DATABASE_URL" => Some("sqlite::memory:".to_owned()),
        "PODPOT_SYSTEMD" => Some("fake".to_owned()),
        _ => None,
    })
    .expect("la configuración de prueba tiene que ser válida")
}

/// Lanza una petición contra el router y devuelve el código y el cuerpo JSON.
pub async fn peticion(app: &Router, method: Method, uri: &str) -> (StatusCode, serde_json::Value) {
    let peticion = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("la petición de prueba se construye siempre");

    let respuesta = app
        .clone()
        .oneshot(peticion)
        .await
        .expect("el router no puede fallar al atender");

    let status = respuesta.status();

    (status, body_json(respuesta).await)
}

/// Lee el cuerpo de una respuesta como JSON.
pub async fn body_json(respuesta: Response) -> serde_json::Value {
    let bytes = respuesta
        .into_body()
        .collect()
        .await
        .expect("el cuerpo de la respuesta se puede leer")
        .to_bytes();

    serde_json::from_slice(&bytes).expect("el cuerpo de una respuesta de la API es JSON")
}
