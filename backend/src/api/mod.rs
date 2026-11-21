//! El servidor HTTP de podpot.
//!
//! Todo lo que cuelga de `/api/v1/` responde con la misma envoltura: los datos
//! dentro de `data` y los fallos dentro de `error`.

pub mod health;
pub mod response;

use axum::Router;
use axum::routing::get;

use crate::error::AppError;
use crate::state::AppState;

/// Compone el router de la aplicación con su estado.
///
/// Las dos trampas que el router cierra a mano son las de la envoltura única:
/// una ruta que no existe y un método que no está declarado contestan por
/// defecto con un cuerpo vacío, y el contrato del §1 exige la envoltura **también
/// ahí**, así que las dos se sustituyen por manejadores propios.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health::health))
        .fallback(ruta_inexistente)
        .method_not_allowed_fallback(metodo_no_permitido)
        .with_state(state)
}

/// El `404` de la casa: una ruta que no existe, con la envoltura de error.
async fn ruta_inexistente() -> AppError {
    AppError::NotFound("la ruta solicitada no existe".to_owned())
}

/// El `405` de la casa: la ruta existe pero el método no está declarado.
async fn metodo_no_permitido() -> AppError {
    AppError::MethodNotAllowed("el método no está permitido en esta ruta".to_owned())
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, StatusCode};

    use crate::test_support::{entorno, peticion};

    /// Escenario «Salud disponible»: sin autenticación, `200` y el modo de escucha.
    #[tokio::test]
    async fn la_ruta_de_salud_esta_viva_y_dice_donde_escucha() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/health").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cuerpo["data"]["status"], "ok");
        assert_eq!(cuerpo["data"]["bind_address"], "127.0.0.1:8080");
    }

    /// Escenario «Recurso desconocido»: también en los errores, la envoltura.
    #[tokio::test]
    async fn una_ruta_inexistente_da_404_con_la_envoltura() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/no-existe").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            cuerpo["error"]["code"].as_str().is_some(),
            "el 404 tiene que traer la envoltura de error: {cuerpo}"
        );
        assert!(
            cuerpo["error"]["message"].as_str().is_some(),
            "el 404 tiene que explicar el motivo: {cuerpo}"
        );
    }

    /// Escenario «Método no permitido»: `405` con la misma envoltura.
    #[tokio::test]
    async fn un_metodo_no_permitido_sobre_una_ruta_existente_da_405() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::POST, "/api/v1/health").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }
}
