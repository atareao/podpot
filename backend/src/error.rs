//! El error tipado de la aplicación y su traducción a respuesta HTTP.
//!
//! Toda respuesta fallida de `/api/v1/` sale por aquí, con la misma forma:
//! `{ "error": { "code": "<snake_case>", "message": "<texto>" } }`.
//!
//! La regla que gobierna la traducción es la de siempre y no admite excepciones:
//! un `5xx` **no publica su causa** —el texto de la causa puede llevar rutas del
//! despliegue, object paths de D-Bus o el `Display` de una consulta, que son
//! cosas del operador y no de quien pregunta—, y un `4xx` explica su motivo
//! nombrando el campo o la unidad, nunca el valor que llegó.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::api::response::ErrorResponse;
use crate::config::ConfigError;
use crate::systemd::SystemdError;

/// El mensaje con el que el cliente recibe un fallo interno.
///
/// Es deliberadamente genérico: el texto de la causa se queda en el registro del
/// servidor, porque es donde acaban las rutas y los detalles de las bibliotecas.
pub const INTERNAL_MESSAGE: &str = "error interno del servidor";

/// El mensaje con el que el cliente recibe que systemd no contesta.
///
/// Es propio y no el genérico del `500` porque el desenlace no es el mismo: la
/// petición está bien y lo que no está disponible es el gestor de systemd, así
/// que lo que quien pregunta puede hacer es reintentar más tarde.
pub const SYSTEMD_UNAVAILABLE_MESSAGE: &str = "el gestor de systemd del usuario no está disponible";

/// El código del rechazo por una entrada que no se puede atender.
const CODE_BAD_REQUEST: &str = "bad_request";

/// El código de un recurso que no existe.
const CODE_NOT_FOUND: &str = "not_found";

/// El código de un método que no está declarado para la ruta.
const CODE_METHOD_NOT_ALLOWED: &str = "method_not_allowed";

/// El código de una unidad que el gestor no conoce.
const CODE_UNIT_NOT_FOUND: &str = "unit_not_found";

/// El código de una operación que systemd no pudo completar.
const CODE_SYSTEMD_OPERATION_FAILED: &str = "systemd_operation_failed";

/// El código de una operación que el gestor rechazó.
const CODE_SYSTEMD_UNIT_REJECTED: &str = "systemd_unit_rejected";

/// El código de una operación que no terminó en plazo.
const CODE_SYSTEMD_TIMEOUT: &str = "systemd_timeout";

/// El código de un gestor de systemd que no contesta.
const CODE_SYSTEMD_UNAVAILABLE: &str = "systemd_unavailable";

/// El código de un fallo del propio servidor.
const CODE_INTERNAL: &str = "internal";

/// El error de la aplicación, con el código HTTP que le corresponde.
#[derive(Debug, Error)]
pub enum AppError {
    /// La petición traía algo que no se puede atender: el motivo sí se explica.
    #[error("{0}")]
    BadRequest(String),

    /// El recurso pedido no existe.
    #[error("{0}")]
    NotFound(String),

    /// El método HTTP no está declarado para esta ruta.
    #[error("{0}")]
    MethodNotAllowed(String),

    /// La capa de systemd falló. Cada variante suya tiene su propia traducción.
    #[error(transparent)]
    Systemd(#[from] SystemdError),

    /// La configuración no permite atender la petición.
    ///
    /// Por construcción sólo se puede disparar al atender una petición: el
    /// arranque no pasa por el router, así que un error de configuración aquí es
    /// un fallo del servidor.
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// Fallo interno: el detalle va al registro y al cliente le llega un mensaje
    /// genérico.
    #[error("{0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            // El cliente necesita saber qué traía mal la petición: el motivo se
            // explica, y se repite a `debug` para que quede la traza del rechazo.
            Self::BadRequest(motivo) => {
                tracing::debug!(motivo = %motivo, "petición rechazada");
                (StatusCode::BAD_REQUEST, CODE_BAD_REQUEST, motivo)
            }
            Self::NotFound(motivo) => (StatusCode::NOT_FOUND, CODE_NOT_FOUND, motivo),
            Self::MethodNotAllowed(motivo) => (
                StatusCode::METHOD_NOT_ALLOWED,
                CODE_METHOD_NOT_ALLOWED,
                motivo,
            ),
            Self::Systemd(causa) => traducir_systemd(causa),
            Self::Config(causa) => {
                tracing::error!(error = %causa, "error de configuración al atender la petición");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    CODE_INTERNAL,
                    INTERNAL_MESSAGE.to_owned(),
                )
            }
            Self::Internal(causa) => {
                tracing::error!(error = %causa, "fallo interno al atender la petición");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    CODE_INTERNAL,
                    INTERNAL_MESSAGE.to_owned(),
                )
            }
        };

        ErrorResponse::new(status, code, message).into_response()
    }
}

/// Traduce un fallo de la capa de systemd al código y el mensaje que le tocan.
///
/// La frontera es la que separa «lo que pidió quien llama está mal» de «el
/// sistema no pudo»: una unidad que no existe es un `404`; que el gestor diga
/// que **no** a la operación —enmascarada, no cargable, transacción destructiva—
/// es un `502`, y una operación que no llega a su estado, otro `502`: en los dos
/// el fallo está en el gestor, no en podpot. Un bus que no contesta es un `503`,
/// porque la petición está bien y lo que falta es poder atenderla ahora.
///
/// Los `5xx` publican un mensaje propio que **nombra la unidad** y no copia ni el
/// texto de la causa ni el resultado crudo del job.
fn traducir_systemd(causa: SystemdError) -> (StatusCode, &'static str, String) {
    match causa {
        SystemdError::UnitNotFound { unit } => (
            StatusCode::NOT_FOUND,
            CODE_UNIT_NOT_FOUND,
            format!("la unidad `{unit}` no existe"),
        ),
        SystemdError::InvalidUnitName { name } => {
            // El valor que llegó se queda en el registro: un `4xx` explica el
            // motivo nombrando el campo, no repitiendo lo que mandó quien llama.
            tracing::debug!(nombre = %name, "nombre de unidad rechazado");

            (
                StatusCode::BAD_REQUEST,
                CODE_BAD_REQUEST,
                "el nombre de unidad no es un nombre de systemd válido".to_owned(),
            )
        }
        SystemdError::OperationFailed {
            unit,
            action,
            result,
        } => {
            tracing::error!(
                unit = %unit,
                accion = action,
                resultado = %result,
                "systemd no llevó la unidad al estado pedido"
            );

            (
                StatusCode::BAD_GATEWAY,
                CODE_SYSTEMD_OPERATION_FAILED,
                format!("no se pudo completar la operación sobre la unidad `{unit}`"),
            )
        }
        SystemdError::UnitRejected { unit, causa } => {
            tracing::error!(
                unit = %unit,
                error = %causa,
                "el gestor de systemd rechazó la operación"
            );

            (
                StatusCode::BAD_GATEWAY,
                CODE_SYSTEMD_UNIT_REJECTED,
                format!("el gestor de systemd rechazó la operación sobre la unidad `{unit}`"),
            )
        }
        SystemdError::TimedOut { unit } => {
            tracing::error!(unit = %unit, "la operación de systemd no terminó en plazo");

            (
                StatusCode::GATEWAY_TIMEOUT,
                CODE_SYSTEMD_TIMEOUT,
                format!("la operación sobre la unidad `{unit}` no terminó a tiempo"),
            )
        }
        SystemdError::Transport { causa } => {
            tracing::error!(error = %causa, "no se pudo hablar con el gestor de systemd del usuario");

            (
                StatusCode::SERVICE_UNAVAILABLE,
                CODE_SYSTEMD_UNAVAILABLE,
                SYSTEMD_UNAVAILABLE_MESSAGE.to_owned(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    use crate::test_support::body_json;

    use super::{AppError, INTERNAL_MESSAGE, SYSTEMD_UNAVAILABLE_MESSAGE, SystemdError};

    /// Requisito «Envoltura JSON única»: un rechazo de la petición explica su
    /// motivo y viaja con la envoltura de error.
    #[tokio::test]
    async fn una_peticion_invalida_da_400_con_el_motivo() {
        let respuesta =
            AppError::BadRequest("la acción `saltar` no existe".to_owned()).into_response();

        assert_eq!(respuesta.status(), StatusCode::BAD_REQUEST);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "bad_request");
        assert_eq!(cuerpo["error"]["message"], "la acción `saltar` no existe");
        assert!(
            cuerpo.get("data").is_none(),
            "un error no puede traer datos: {cuerpo}"
        );
    }

    /// Escenario «Recurso desconocido»: `404` con la envoltura y un error
    /// descriptivo.
    #[tokio::test]
    async fn un_recurso_desconocido_da_404_con_la_envoltura() {
        let respuesta =
            AppError::NotFound("la ruta solicitada no existe".to_owned()).into_response();

        assert_eq!(respuesta.status(), StatusCode::NOT_FOUND);

        let cuerpo = body_json(respuesta).await;
        assert_eq!(cuerpo["error"]["code"], "not_found");
    }

    /// Escenario «Método no permitido»: `405` con la misma envoltura.
    #[tokio::test]
    async fn un_metodo_no_permitido_da_405_con_la_envoltura() {
        let respuesta =
            AppError::MethodNotAllowed("método no permitido".to_owned()).into_response();

        assert_eq!(respuesta.status(), StatusCode::METHOD_NOT_ALLOWED);

        let cuerpo = body_json(respuesta).await;
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// Escenario «Unidad inexistente»: el error nombra la unidad y **no** filtra
    /// detalles internos.
    #[tokio::test]
    async fn una_unidad_inexistente_da_404_nombrando_la_unidad() {
        let respuesta = AppError::Systemd(SystemdError::UnitNotFound {
            unit: "fantasma.service".to_owned(),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::NOT_FOUND);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "unit_not_found");
        assert!(
            cuerpo["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("fantasma.service"),
            "el error tiene que nombrar la unidad: {cuerpo}"
        );
    }

    /// Un nombre de unidad que no lo es se rechaza como entrada, no como recurso
    /// ausente.
    #[tokio::test]
    async fn un_nombre_de_unidad_invalido_da_400() {
        let respuesta = AppError::Systemd(SystemdError::InvalidUnitName {
            name: "lo-que-sea".to_owned(),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::BAD_REQUEST);

        let cuerpo = body_json(respuesta).await;
        assert_eq!(cuerpo["error"]["code"], "bad_request");
    }

    /// Requisito «Apagado ordenado»/trampa de D-Bus: un fallo de transporte llega
    /// como `503` genérico, sin object paths ni texto de la biblioteca.
    #[tokio::test]
    async fn un_fallo_de_transporte_no_filtra_object_paths() {
        let objeto = "/org/freedesktop/systemd1/unit/fantasma_2eservice";
        let respuesta = AppError::Systemd(SystemdError::Transport {
            causa: Box::new(std::io::Error::other(format!(
                "did not receive a reply for {objeto}"
            ))),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::SERVICE_UNAVAILABLE);

        let texto = serde_json::to_string(&body_json(respuesta).await).expect("el cuerpo es JSON");

        assert!(
            !texto.contains("systemd1"),
            "el 503 publicó el object path de D-Bus: {texto}"
        );
        assert!(
            !texto.contains("did not receive a reply"),
            "el 503 publicó el texto de la biblioteca: {texto}"
        );

        let cuerpo: serde_json::Value =
            serde_json::from_str(&texto).expect("el cuerpo es un objeto JSON");

        assert_eq!(cuerpo["error"]["code"], "systemd_unavailable");
        assert_eq!(cuerpo["error"]["message"], SYSTEMD_UNAVAILABLE_MESSAGE);
    }

    /// Un nombre de unidad inválido da `400` y **no** repite el valor que llegó:
    /// un `4xx` explica el motivo nombrando el campo, no devolviendo la entrada.
    #[tokio::test]
    async fn un_nombre_de_unidad_invalido_no_repite_el_valor() {
        let respuesta = AppError::Systemd(SystemdError::InvalidUnitName {
            name: "  <script>alert(1)</script>.service".to_owned(),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::BAD_REQUEST);

        let texto = serde_json::to_string(&body_json(respuesta).await).expect("el cuerpo es JSON");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&texto).expect("es JSON")["error"]["code"],
            "bad_request"
        );
        assert!(
            !texto.contains("script"),
            "el 400 repitió el valor que llegó: {texto}"
        );
    }

    /// Un gestor que rechaza la operación da `502` nombrando la unidad, sin
    /// publicar la negativa del gestor.
    #[tokio::test]
    async fn un_rechazo_del_gestor_da_502_sin_filtrar_la_causa() {
        let respuesta = AppError::Systemd(SystemdError::UnitRejected {
            unit: "centinela-db.service".to_owned(),
            causa: Box::new(std::io::Error::other(
                "org.freedesktop.systemd1.UnitMasked: Unit is masked",
            )),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::BAD_GATEWAY);

        let texto = serde_json::to_string(&body_json(respuesta).await).expect("el cuerpo es JSON");

        assert!(texto.contains("centinela-db.service"), "{texto}");
        assert!(
            !texto.contains("UnitMasked"),
            "el 502 publicó la negativa del gestor: {texto}"
        );
    }

    /// Un fallo interno no publica su causa: el detalle va al registro.
    #[tokio::test]
    async fn un_fallo_interno_no_publica_la_causa() {
        let respuesta =
            AppError::Internal("la consulta falló en /srv/podpot/data/podpot.db".to_owned())
                .into_response();

        assert_eq!(respuesta.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "internal");
        assert_eq!(cuerpo["error"]["message"], INTERNAL_MESSAGE);
    }

    /// Un fallo del gestor de systemd que no es «no existe» no se confunde con un
    /// `5xx` del propio podpot: la culpa es de la unidad que no llegó a su estado.
    #[tokio::test]
    async fn una_operacion_que_no_llega_a_su_estado_da_502() {
        let respuesta = AppError::Systemd(SystemdError::OperationFailed {
            unit: "rota.service".to_owned(),
            action: "start",
            result: "failed".to_owned(),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::BAD_GATEWAY);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "systemd_operation_failed");
        assert_eq!(
            cuerpo["error"]["message"],
            "no se pudo completar la operación sobre la unidad `rota.service`",
            "el 502 nombra la unidad y no repite el resultado crudo del job: {cuerpo}"
        );
    }

    /// Y una operación que no termina en plazo lo dice con un `504`.
    #[tokio::test]
    async fn una_operacion_que_no_termina_en_plazo_da_504() {
        let respuesta = AppError::Systemd(SystemdError::TimedOut {
            unit: "lenta.service".to_owned(),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::GATEWAY_TIMEOUT);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "systemd_timeout");
        assert!(
            cuerpo["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("lenta.service"),
            "el error tiene que nombrar la unidad: {cuerpo}"
        );
    }
}
