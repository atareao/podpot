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
use crate::quadlet::QuadletError;
use crate::registros::RegistrosError;
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

/// El código de una recarga de systemd que el gestor no pudo hacer.
const CODE_SYSTEMD_RELOAD_FAILED: &str = "systemd_reload_failed";

/// El código de una unidad que no tiene contenedor del que leer registros.
const CODE_UNIT_WITHOUT_CONTAINER: &str = "unit_without_container";

/// El código de una API de Podman que no contesta.
const CODE_PODMAN_UNAVAILABLE: &str = "podman_unavailable";

/// El código de una API de Podman que contesta algo que no se puede usar.
const CODE_PODMAN_RESPONSE: &str = "podman_response";

/// El código de una quadlet que ya existe con ese nombre.
const CODE_DUPLICATE: &str = "duplicate";

/// El código de un nombre de quadlet que no se puede usar.
const CODE_INVALID_NAME: &str = "invalid_name";

/// El código de un tipo de quadlet que no se puede crear.
const CODE_INVALID_KIND: &str = "invalid_kind";

/// El código de un valor de campo que no se puede aceptar.
const CODE_INVALID_VALUE: &str = "invalid_value";

/// El código de una quadlet que no se pudo escribir.
const CODE_WRITE_FAILED: &str = "write_failed";

/// El código de una quadlet que no se pudo leer.
const CODE_READ_FAILED: &str = "read_failed";

/// El código de un fallo del propio servidor.
const CODE_INTERNAL: &str = "internal";

/// El código de una petición sin sesión válida.
///
/// Es el que el *gate* de la superficie de negocio devuelve, y el que el panel
/// mira para distinguir «sin sesión» de cualquier otro fallo y volver a la
/// portada.
const CODE_UNAUTHORIZED: &str = "unauthorized";

/// El código de un fallo de un servicio de terceros (el proveedor de identidad).
const CODE_BAD_GATEWAY: &str = "bad_gateway";

/// El mensaje con el que el cliente recibe un fallo del proveedor de identidad.
///
/// Es genérico por la misma razón que el interno: la causa —el `Display` de
/// `reqwest`, una URL, un estado HTTP ajeno— es cosa del operador y acaba en el
/// registro, no en el cuerpo de la respuesta.
pub const BAD_GATEWAY_MESSAGE: &str = "el proveedor de identidad no está disponible";

/// El mensaje con el que el cliente recibe que la API de Podman no contesta.
///
/// Es propio y no el genérico del `500` porque el desenlace no es el mismo: la
/// petición está bien y lo que no está disponible es Podman, así que lo que quien
/// pregunta puede hacer es arrancarlo y reintentar.
pub const PODMAN_UNAVAILABLE_MESSAGE: &str = "la API de Podman del usuario no está disponible";

/// El mensaje con el que el cliente recibe que Podman contestó algo inservible.
///
/// No publica la causa —el estado o el cuerpo que llegó son del operador— pero sí
/// distingue el desenlace: no es que Podman esté caído, es que contestó mal.
pub const PODMAN_RESPONSE_MESSAGE: &str = "la API de Podman contestó algo que no se puede usar";

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

    /// No hay sesión válida para esta ruta.
    ///
    /// El motivo se explica —«hace falta una sesión válida»— pero **no** se
    /// distingue «no hay cookie» de «la cookie no vale»: decir cuál de las dos es
    /// contaría a quien no ha entrado si su sesión caducó o si la firma no
    /// cuadra. El detalle se queda en el registro.
    #[error("{0}")]
    Unauthorized(String),

    /// Un servicio de terceros —el proveedor de identidad— falló.
    ///
    /// Es el desenlace del descubrimiento, del canje del código o del JWKS: la
    /// petición de podpot está bien y lo que no responde o responde mal es el
    /// proveedor. La causa viaja en el error y acaba en el registro, nunca en el
    /// cuerpo.
    #[error("{0}")]
    BadGateway(String),

    /// La capa de systemd falló. Cada variante suya tiene su propia traducción.
    #[error(transparent)]
    Systemd(#[from] SystemdError),

    /// La creación de una quadlet falló. Cada variante tiene su traducción.
    #[error(transparent)]
    Quadlet(#[from] QuadletError),

    /// La lectura de los registros del contenedor falló.
    #[error(transparent)]
    Registros(#[from] RegistrosError),

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
            // El 401 explica que hace falta sesión, no por qué la que llegó no
            // vale: el detalle se queda en el registro de `session::verify`.
            Self::Unauthorized(motivo) => {
                tracing::debug!(motivo = %motivo, "petición sin sesión válida");
                (StatusCode::UNAUTHORIZED, CODE_UNAUTHORIZED, motivo)
            }
            // El 502 no publica la causa del proveedor: es del operador y va al
            // registro.
            Self::BadGateway(causa) => {
                tracing::error!(error = %causa, "el proveedor de identidad falló");
                (
                    StatusCode::BAD_GATEWAY,
                    CODE_BAD_GATEWAY,
                    BAD_GATEWAY_MESSAGE.to_owned(),
                )
            }
            Self::Systemd(causa) => traducir_systemd(causa),
            Self::Quadlet(causa) => traducir_quadlet(causa),
            Self::Registros(causa) => traducir_registros(causa),
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
        SystemdError::ReloadFailed { causa } => {
            tracing::error!(
                error = %causa,
                "el gestor de systemd no pudo recargar su configuración"
            );

            (
                StatusCode::BAD_GATEWAY,
                CODE_SYSTEMD_RELOAD_FAILED,
                "el gestor de systemd no pudo recargar su configuración".to_owned(),
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

/// Traduce un fallo de la creación de una quadlet.
///
/// La frontera es la misma que en systemd: lo que quien llama puede arreglar
/// —un nombre que ya está en uso, un nombre o un tipo que no valen— sale con su
/// `4xx` y **sin** repetir el valor que llegó; lo que falla del lado del servidor
/// —la escritura, el registro— sale con el mensaje genérico del `500` y el
/// detalle se queda en el registro —la ruta del fichero no se publica—. Los
/// fallos de systemd se delegan en [`traducir_systemd`], que ya los clasifica.
fn traducir_quadlet(causa: QuadletError) -> (StatusCode, &'static str, String) {
    match causa {
        QuadletError::Duplicate { file_name } => {
            tracing::debug!(fichero = %file_name, "la quadlet ya existe");

            (
                StatusCode::CONFLICT,
                CODE_DUPLICATE,
                "ya existe una quadlet con ese nombre en el directorio de quadlets".to_owned(),
            )
        }
        QuadletError::InvalidName { stem } => {
            tracing::debug!(nombre = %stem, "nombre de quadlet rechazado");

            (
                StatusCode::BAD_REQUEST,
                CODE_INVALID_NAME,
                "el nombre de la quadlet no es válido".to_owned(),
            )
        }
        QuadletError::InvalidKind { kind } => {
            tracing::debug!(tipo = %kind, "tipo de quadlet rechazado");

            (
                StatusCode::BAD_REQUEST,
                CODE_INVALID_KIND,
                "el tipo de quadlet no es de los que se pueden crear: se espera \
                 `container`, `volume`, `network` o `pod`"
                    .to_owned(),
            )
        }
        QuadletError::MissingField { kind, field } => (
            StatusCode::BAD_REQUEST,
            CODE_BAD_REQUEST,
            format!("el tipo `{kind}` exige el campo `{field}`"),
        ),
        QuadletError::InvalidValue { field } => {
            tracing::debug!(campo = field, "valor de campo rechazado");

            (
                StatusCode::BAD_REQUEST,
                CODE_INVALID_VALUE,
                format!("el campo `{field}` tiene un valor no válido"),
            )
        }
        QuadletError::WriteFailed { file_name, causa } => {
            tracing::error!(fichero = %file_name, error = %causa, "no se pudo escribir la quadlet");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                CODE_WRITE_FAILED,
                INTERNAL_MESSAGE.to_owned(),
            )
        }
        QuadletError::ReadFailed { file_name, causa } => {
            tracing::error!(fichero = %file_name, error = %causa, "no se pudo leer la quadlet");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                CODE_READ_FAILED,
                INTERNAL_MESSAGE.to_owned(),
            )
        }
        QuadletError::Persistencia { file_name, causa } => {
            tracing::error!(
                fichero = %file_name,
                error = %causa,
                "no se pudo registrar la creación de la quadlet"
            );

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                CODE_INTERNAL,
                INTERNAL_MESSAGE.to_owned(),
            )
        }
        QuadletError::NotFound { file_name } => {
            tracing::debug!(fichero = %file_name, "la quadlet no existe");

            (
                StatusCode::NOT_FOUND,
                CODE_NOT_FOUND,
                "no existe una quadlet con ese nombre en el directorio de quadlets".to_owned(),
            )
        }
        QuadletError::Registro { file_name, causa } => {
            tracing::error!(
                fichero = %file_name,
                error = %causa,
                "no se pudo registrar la operación sobre la quadlet"
            );

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                CODE_INTERNAL,
                INTERNAL_MESSAGE.to_owned(),
            )
        }
        QuadletError::Systemd(causa) => traducir_systemd(causa),
    }
}

/// Traduce un fallo de la lectura de registros.
///
/// La frontera es la de siempre: una unidad sin contenedor es un `4xx` —no ha
/// fallado nada, el recurso no está— y explica el motivo nombrando la unidad; un
/// Podman que no contesta es un `503` —la petición está bien y falta poder
/// atenderla—; y una respuesta inservible es un `502` —el fallo está del otro
/// lado—. Los `5xx` **no** publican la causa: lleva la ruta del socket y el
/// `Display` de la biblioteca, que son del operador y acaban en el registro.
fn traducir_registros(causa: RegistrosError) -> (StatusCode, &'static str, String) {
    match causa {
        RegistrosError::SinContenedor { unidad } => (
            StatusCode::NOT_FOUND,
            CODE_UNIT_WITHOUT_CONTAINER,
            format!("la unidad `{unidad}` no tiene contenedor del que leer registros"),
        ),
        RegistrosError::Socket { socket, causa } => {
            tracing::error!(
                socket = %socket,
                error = %causa,
                "no se pudo hablar con la API de Podman"
            );

            (
                StatusCode::SERVICE_UNAVAILABLE,
                CODE_PODMAN_UNAVAILABLE,
                PODMAN_UNAVAILABLE_MESSAGE.to_owned(),
            )
        }
        RegistrosError::Podman { causa } => {
            tracing::error!(
                error = %causa,
                "la API de Podman contestó algo que no se puede usar"
            );

            (
                StatusCode::BAD_GATEWAY,
                CODE_PODMAN_RESPONSE,
                PODMAN_RESPONSE_MESSAGE.to_owned(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    use crate::test_support::body_json;

    use super::{
        AppError, INTERNAL_MESSAGE, PODMAN_RESPONSE_MESSAGE, PODMAN_UNAVAILABLE_MESSAGE,
        SYSTEMD_UNAVAILABLE_MESSAGE, SystemdError,
    };
    use crate::quadlet::QuadletError;
    use crate::registros::RegistrosError;

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

    /// Una recarga que el gestor no pudo hacer es un `502` sin publicar la causa.
    #[tokio::test]
    async fn una_recarga_fallida_da_502_sin_filtrar_la_causa() {
        let respuesta = AppError::Systemd(SystemdError::ReloadFailed {
            causa: Box::new(std::io::Error::other(
                "org.freedesktop.systemd1.TooManyJobs: no hay sitio para el job",
            )),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::BAD_GATEWAY);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "systemd_reload_failed");
        assert!(
            !cuerpo.to_string().contains("TooManyJobs"),
            "el 502 no puede publicar la causa del gestor: {cuerpo}"
        );
    }

    /// Escenario «Fallo de escritura»: `500` con el mensaje genérico y **sin**
    /// publicar la ruta del fichero.
    #[tokio::test]
    async fn un_fallo_de_escritura_da_500_sin_publicar_la_ruta() {
        let respuesta = AppError::Quadlet(QuadletError::WriteFailed {
            file_name: "web.container".to_owned(),
            causa: std::io::Error::other("permiso denegado en /home/tester/.config"),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "write_failed");
        assert_eq!(cuerpo["error"]["message"], INTERNAL_MESSAGE);
        assert!(
            !cuerpo.to_string().contains("/home/tester"),
            "el 500 no puede publicar la ruta del fichero: {cuerpo}"
        );
    }

    /// Un nombre de quadlet inválido es un `400` que no repite el valor.
    #[tokio::test]
    async fn un_nombre_de_quadlet_invalido_da_400_sin_repetir_el_valor() {
        let respuesta = AppError::Quadlet(QuadletError::InvalidName {
            stem: "../escape".to_owned(),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::BAD_REQUEST);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "invalid_name");
        assert!(
            !cuerpo.to_string().contains("escape"),
            "el 400 repitió el valor que llegó: {cuerpo}"
        );
    }

    /// Escenario «Nombre duplicado»: un `409`.
    #[tokio::test]
    async fn un_nombre_duplicado_da_409() {
        let respuesta = AppError::Quadlet(QuadletError::Duplicate {
            file_name: "web.container".to_owned(),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::CONFLICT);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "duplicate");
    }

    /// Escenario «Valor con estructura»: un valor de campo con caracteres de
    /// control es un `400` con `invalid_value`, que **nombra el campo** y no el
    /// valor.
    #[tokio::test]
    async fn un_valor_de_campo_invalido_da_400_nombrando_el_campo() {
        let respuesta =
            AppError::Quadlet(QuadletError::InvalidValue { field: "image" }).into_response();

        assert_eq!(respuesta.status(), StatusCode::BAD_REQUEST);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "invalid_value");
        assert!(
            cuerpo["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("image"),
            "el 400 tiene que nombrar el campo: {cuerpo}"
        );
    }

    // --- Los registros del contenedor ----------------------------------------

    /// Escenario «Unidad sin contenedor»: `404` con su código propio, nombrando la
    /// unidad y **sin** abrir ningún flujo.
    #[tokio::test]
    async fn una_unidad_sin_contenedor_da_404_nombrandola() {
        let respuesta = AppError::Registros(RegistrosError::SinContenedor {
            unidad: "datos-volume.service".to_owned(),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::NOT_FOUND);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "unit_without_container");
        assert!(
            cuerpo["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("datos-volume.service"),
            "el 404 tiene que nombrar la unidad: {cuerpo}"
        );
    }

    /// Un Podman que no contesta es un `503`: la petición está bien y lo que falta
    /// es poder atenderla. La causa —la ruta del socket, el `Display` de la
    /// biblioteca— **no** se publica.
    #[tokio::test]
    async fn un_podman_que_no_contesta_da_503_sin_publicar_la_causa() {
        let respuesta = AppError::Registros(RegistrosError::Socket {
            socket: "/run/user/1000/podman/podman.sock".to_owned(),
            causa: Box::new(std::io::Error::other("la ruta secreta del operador")),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::SERVICE_UNAVAILABLE);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "podman_unavailable");
        assert_eq!(cuerpo["error"]["message"], PODMAN_UNAVAILABLE_MESSAGE);
        assert!(
            !cuerpo.to_string().contains("secreta"),
            "el 5xx no puede publicar la causa: {cuerpo}"
        );
    }

    /// Y una respuesta inservible es un `502`: la petición llegó y lo que falló
    /// está del otro lado.
    #[tokio::test]
    async fn una_respuesta_de_podman_inservible_da_502() {
        let respuesta = AppError::Registros(RegistrosError::Podman {
            causa: Box::new(std::io::Error::other("un estado que no era 200")),
        })
        .into_response();

        assert_eq!(respuesta.status(), StatusCode::BAD_GATEWAY);

        let cuerpo = body_json(respuesta).await;

        assert_eq!(cuerpo["error"]["code"], "podman_response");
        assert_eq!(cuerpo["error"]["message"], PODMAN_RESPONSE_MESSAGE);
    }
}
