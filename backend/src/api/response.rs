//! La envoltura única de las respuestas de la API.
//!
//! Dos formas y ninguna más:
//!
//! * éxito — `{ "data": <payload> }`
//! * error — `{ "error": { "code": "<snake_case>", "message": "<texto>" } }`

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// La envoltura de una respuesta con datos: `{ "data": <payload> }`.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    /// El contenido de la respuesta.
    pub data: T,
}

impl<T> ApiResponse<T> {
    /// Envuelve el contenido de una respuesta correcta.
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

/// La envoltura de una respuesta fallida: `{ "error": { "code", "message" } }`.
///
/// El código HTTP viaja **fuera** del JSON —en la cabecera—, así que no se
/// serializa: duplicarlo en el cuerpo sólo daría dos sitios que pueden
/// contradecirse.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// El estado HTTP, que es cabecera y no cuerpo.
    #[serde(skip)]
    status: StatusCode,
    /// El error, con su código estable y su mensaje.
    error: ErrorBody,
}

/// El error de dentro de la envoltura.
#[derive(Debug, Serialize)]
struct ErrorBody {
    /// Un código estable en `snake_case`, pensado para que el panel decida sin
    /// mirar el texto.
    code: &'static str,
    /// El texto para una persona. Nunca lleva detalles internos: el `code` es lo
    /// que se interpreta y el mensaje lo que se enseña.
    message: String,
}

impl ErrorResponse {
    /// Compone la envoltura de error con su código HTTP.
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            error: ErrorBody {
                code,
                message: message.into(),
            },
        }
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (self.status, Json(self)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use serde::Serialize;

    use crate::test_support::body_json;

    use super::{ApiResponse, ErrorResponse};

    #[derive(Serialize)]
    struct Carga {
        valor: u8,
    }

    /// Escenario «Respuesta de negocio correcta»: `200` y los datos dentro de
    /// `data`.
    #[tokio::test]
    async fn el_exito_envuelve_los_datos_en_data() {
        let respuesta = ApiResponse::new(Carga { valor: 7 }).into_response();

        assert_eq!(respuesta.status(), StatusCode::OK);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["data"]["valor"], 7);
        assert!(
            cuerpo.get("error").is_none(),
            "una respuesta correcta no puede traer error: {cuerpo}"
        );
        assert_eq!(
            cuerpo.as_object().expect("el cuerpo es un objeto").len(),
            1,
            "la envoltura de éxito tiene exactamente un campo: {cuerpo}"
        );
    }

    /// El error lleva su código en `snake_case` y su mensaje, y nada más: el
    /// estado va en la cabecera y no se repite dentro.
    #[tokio::test]
    async fn el_error_envuelve_codigo_y_mensaje() {
        let respuesta = ErrorResponse::new(
            StatusCode::CONFLICT,
            "conflict",
            "la unidad ya está en ese estado",
        )
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::CONFLICT);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(
            cuerpo["error"]
                .as_object()
                .expect("el error es un objeto")
                .len(),
            2,
            "la envoltura de error tiene exactamente code y message: {cuerpo}"
        );
        assert_eq!(cuerpo["error"]["code"], "conflict");
        assert_eq!(
            cuerpo["error"]["message"],
            "la unidad ya está en ese estado"
        );
        assert_eq!(
            cuerpo.as_object().expect("el cuerpo es un objeto").len(),
            1,
            "el cuerpo de un error tiene exactamente el campo error: {cuerpo}"
        );
    }
}
