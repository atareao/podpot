//! La ruta de creación de quadlets.
//!
//! `POST /api/v1/quadlets` recibe la **entrada tipada** ([`QuadletDraft`]) y
//! contesta con la envoltura de la casa: `{data}` con el nombre de unidad
//! generado, o `{error}` con el código del tipo de fallo. La ruta no decide
//! nada: el contrato y la validación viven en [`crate::quadlet`] y la traducción
//! de errores a HTTP en [`crate::error`].
//!
//! El cuerpo se lee como bytes y se deserializa a mano —en vez de con un
//! extractor `Json`— porque un `Json` que falle contesta con su propio texto
//! plano, y aquí toda respuesta fallida tiene que llevar la envoltura.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;

use crate::api::response::ApiResponse;
use crate::error::AppError;
use crate::quadlet::{Quadlet, QuadletDraft, QuadletKind, service};
use crate::state::AppState;
use crate::systemd::UnitName;

/// Lo que devuelve una creación correcta.
///
/// Lleva el nombre de fichero, el de unidad y el tipo, y **no** la ruta escrita:
/// dónde vive la quadlet es un detalle del sistema de ficheros del usuario y la
/// API no lo publica —como tampoco publica la procedencia en el listado—.
#[derive(Debug, Serialize)]
pub struct QuadletOutcome {
    /// El nombre del fichero escrito (`web.container`).
    file_name: String,
    /// El nombre de la unidad que systemd genera (`web.service`).
    unit_name: UnitName,
    /// El tipo de quadlet creada.
    kind: QuadletKind,
}

impl From<Quadlet> for QuadletOutcome {
    fn from(quadlet: Quadlet) -> Self {
        Self {
            file_name: quadlet.file_name,
            unit_name: quadlet.unit_name,
            kind: quadlet.kind,
        }
    }
}

/// `POST /api/v1/quadlets` — crea una quadlet de un tipo soportado.
pub async fn crear(
    State(state): State<AppState>,
    cuerpo: Bytes,
) -> Result<(StatusCode, ApiResponse<QuadletOutcome>), AppError> {
    // El cuerpo se deserializa a mano, y no con un extractor `Json`, porque un
    // `Json` que falle contesta con su propio texto plano: aquí hasta el rechazo
    // por cuerpo mal formado tiene que llevar la envoltura de la casa.
    let draft: QuadletDraft = serde_json::from_slice(&cuerpo).map_err(|causa| {
        // El `Display` de serde puede repetir el valor que llegó; un `4xx` de la
        // casa explica el rechazo sin citarlo, así que el detalle se queda en el
        // registro y al cliente le va un mensaje genérico.
        tracing::debug!(error = %causa, "cuerpo de creación de quadlet rechazado");

        AppError::BadRequest("el cuerpo no es una creación de quadlet válida".to_owned())
    })?;

    let quadlet = service::crear(
        &state.config.quadlet_dir,
        state.systemd.as_ref(),
        &state.pool,
        draft,
    )
    .await?;

    Ok((StatusCode::CREATED, ApiResponse::new(quadlet.into())))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::http::{Method, StatusCode};
    use serde_json::json;

    use crate::systemd::SystemdError;
    use crate::systemd::fake::FakeSystemd;
    use crate::test_support::{
        DobleQueFalla, entorno, entorno_con, peticion, peticion_json, peticion_texto,
    };

    /// Una petición válida de contenedor.
    fn peticion_valida(name: &str) -> serde_json::Value {
        json!({
            "name": name,
            "kind": "container",
            "image": "docker.io/library/nginx:latest",
        })
    }

    /// Escenario «Creación correcta»: `201` con el nombre de unidad generado, y el
    /// fichero queda escrito en el directorio configurado.
    #[tokio::test]
    async fn crear_una_quadlet_valida_da_201_con_el_nombre_de_unidad() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            peticion_valida("web"),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(cuerpo["data"]["unit_name"], "web.service");
        assert_eq!(cuerpo["data"]["file_name"], "web.container");
        assert_eq!(cuerpo["data"]["kind"], "container");
        assert!(
            cuerpo["data"].get("path").is_none(),
            "la respuesta no puede publicar la ruta del fichero: {cuerpo}"
        );

        let contenido = std::fs::read_to_string(entorno.quadlet_dir().join("web.container"))
            .expect("el fichero tiene que quedar escrito");

        assert!(
            contenido.contains("Image=docker.io/library/nginx:latest"),
            "el fichero no lleva la imagen: {contenido}"
        );
    }

    /// Escenario «Nombre duplicado»: `409` y el fichero de antes intacto.
    #[tokio::test]
    async fn un_nombre_duplicado_da_409() {
        let entorno = entorno().await;
        std::fs::write(entorno.quadlet_dir().join("web.container"), "lo de antes")
            .expect("el fichero previo se escribe");

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            peticion_valida("web"),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(cuerpo["error"]["code"], "duplicate");
        assert_eq!(
            std::fs::read_to_string(entorno.quadlet_dir().join("web.container"))
                .expect("el original sigue ahí"),
            "lo de antes"
        );
    }

    /// Escenario «Nombre o tipo inválidos»: `400` con el código que toca.
    #[tokio::test]
    async fn un_nombre_invalido_da_400() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            peticion_valida("web malo"),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(cuerpo["error"]["code"], "invalid_name");
        assert!(
            !cuerpo.to_string().contains("web malo"),
            "el 400 no puede repetir el valor que llegó: {cuerpo}"
        );
    }

    /// `.desktop` no es un tipo creable: `400` con `invalid_kind`.
    #[tokio::test]
    async fn un_tipo_no_creable_da_400() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            json!({ "name": "web", "kind": "desktop" }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(cuerpo["error"]["code"], "invalid_kind");
    }

    /// Escenario «Contenido libre rechazado»: un cuerpo de fichero arbitrario no
    /// encaja en el contrato y no se escribe nada.
    #[tokio::test]
    async fn un_cuerpo_de_fichero_libre_da_400_y_no_escribe() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            json!({
                "name": "web",
                "kind": "container",
                "image": "nginx",
                "content": "[Container]\nImage=lo-que-sea\n",
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(cuerpo["error"]["code"], "bad_request");
        assert_eq!(
            std::fs::read_dir(entorno.quadlet_dir())
                .expect("el directorio se puede leer")
                .count(),
            0,
            "no puede quedar ningún fichero escrito"
        );
    }

    /// Un campo que el tipo exige y la petición no trae es una entrada mal
    /// formada, no un fichero a medias.
    #[tokio::test]
    async fn un_campo_que_falta_da_400() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            json!({ "name": "web", "kind": "container" }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(cuerpo["error"]["code"], "bad_request");
    }

    /// Escenario «Fallo de recarga»: `502`.
    #[tokio::test]
    async fn una_recarga_que_falla_da_502() {
        let entorno = entorno_con(Arc::new(DobleQueFalla::con(|| {
            SystemdError::ReloadFailed {
                causa: Box::new(std::io::Error::other("el gestor dijo que no")),
            }
        })))
        .await;

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            peticion_valida("web"),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(cuerpo["error"]["code"], "systemd_reload_failed");
        assert!(
            !cuerpo.to_string().contains("el gestor dijo que no"),
            "el 502 no puede publicar la causa: {cuerpo}"
        );
    }

    /// Escenario «Unidad rechazada»: el gestor contestó que no, y eso también es
    /// un `502`.
    #[tokio::test]
    async fn una_unidad_rechazada_da_502() {
        let entorno = entorno_con(Arc::new(DobleQueFalla::con(|| {
            SystemdError::UnitRejected {
                unit: "web.service".to_owned(),
                causa: Box::new(std::io::Error::other("UnitMasked")),
            }
        })))
        .await;

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            peticion_valida("web"),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(cuerpo["error"]["code"], "systemd_unit_rejected");
    }

    /// Escenario «Fallo de transporte»: `503`.
    #[tokio::test]
    async fn un_fallo_de_transporte_da_503() {
        let entorno = entorno_con(Arc::new(DobleQueFalla::con(|| SystemdError::Transport {
            causa: Box::new(std::io::Error::other(
                "did not receive a reply for /org/freedesktop/systemd1",
            )),
        })))
        .await;

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            peticion_valida("web"),
        )
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(cuerpo["error"]["code"], "systemd_unavailable");
        assert!(
            !cuerpo.to_string().contains("systemd1"),
            "el 503 no puede publicar object paths: {cuerpo}"
        );
    }

    /// Escenario «Valor con estructura»: un valor con salto de línea no puede
    /// inyectar directivas; `400` y nada escrito.
    #[tokio::test]
    async fn un_valor_con_salto_de_linea_da_400_y_no_escribe() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            json!({
                "name": "web",
                "kind": "container",
                "image": "nginx\nUser=root",
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(cuerpo["error"]["code"], "invalid_value");
        assert!(
            !cuerpo.to_string().contains("User=root"),
            "el 400 no puede repetir el valor que llegó: {cuerpo}"
        );
        assert_eq!(
            std::fs::read_dir(entorno.quadlet_dir())
                .expect("el directorio se puede leer")
                .count(),
            0,
            "un valor inyectado no puede dejar nada escrito"
        );
    }

    /// Escenario «Fallo de escritura»: `500` con `write_failed`, sin publicar el
    /// detalle. Se fuerzan las condiciones retirando el directorio de escritura
    /// —ya validado al leer la configuración— para que `create_new` falle: la
    /// escritura no inventa directorios.
    #[tokio::test]
    async fn una_escritura_que_falla_da_500() {
        let entorno = entorno().await;
        let dir = entorno.quadlet_dir().to_path_buf();

        std::fs::remove_dir_all(&dir).expect("el directorio temporal de escritura se puede borrar");

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            peticion_valida("web"),
        )
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(cuerpo["error"]["code"], "write_failed");
        assert!(
            !cuerpo.to_string().contains(&dir.display().to_string()),
            "el 500 no puede publicar la ruta del fichero: {cuerpo}"
        );
    }

    /// Escenario «Fallbacks preservados en la ruta de creación»: el `405` de la
    /// casa sigue en pie sobre la ruta nueva.
    #[tokio::test]
    async fn un_metodo_no_permitido_sobre_quadlets_da_405() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/quadlets").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");

        // Y el fallback del panel no se come la ruta nueva.
        let (status, _) = peticion_texto(&entorno.app(), Method::GET, "/api/v1/quadlets").await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    /// El doble de la casa sigue siendo un `SystemdClient` de verdad: la ruta
    /// recibe el cliente por el estado y no lo construye.
    #[tokio::test]
    async fn la_ruta_usa_el_cliente_del_estado() {
        let entorno = entorno_con(Arc::new(FakeSystemd::new())).await;

        let (status, _) = peticion_json(
            &entorno.app(),
            Method::POST,
            "/api/v1/quadlets",
            json!({ "name": "web-net", "kind": "network", "network_name": "web-net" }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            std::fs::read_to_string(entorno.quadlet_dir().join("web-net.network"))
                .expect("el fichero de red tiene que quedar escrito"),
            "[Network]\nNetworkName=web-net\n"
        );
    }
}
