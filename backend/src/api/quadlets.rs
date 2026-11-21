//! Las rutas de las quadlets: lectura, creación, edición y borrado.
//!
//! `POST /api/v1/quadlets` recibe la **entrada tipada** ([`QuadletDraft`]) y
//! contesta con la envoltura de la casa: `{data}` con el nombre de unidad
//! generado, o `{error}` con el código del tipo de fallo. La ruta no decide
//! nada: el contrato y la validación viven en [`crate::quadlet`] y la traducción
//! de errores a HTTP en [`crate::error`].
//!
//! `GET`, `PATCH` y `DELETE /api/v1/quadlets/{name}` operan sobre una quadlet
//! existente identificada por su **nombre de fichero** —no por el de la unidad—:
//! leer devuelve su tipo y sus campos con la forma plana del cuerpo de la
//! creación, editar reescribe desde el mismo struct tipado y borrar elimina el
//! fichero. Ninguna renombra.
//!
//! El cuerpo se lee como bytes y se deserializa a mano —en vez de con un
//! extractor `Json`— porque un `Json` que falle contesta con su propio texto
//! plano, y aquí toda respuesta fallida tiene que llevar la envoltura.

use axum::body::Bytes;
use axum::extract::{Path, State};
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

/// Deserializa el cuerpo de una edición desde los bytes crudos.
///
/// Es el mismo trato que en [`crear`] y por lo mismo: un `Json` que falle contesta
/// con su propio texto plano y aquí hasta el rechazo tiene que llevar envoltura.
/// El `Display` de serde puede citar el valor recibido, así que el detalle se
/// queda en el registro y al cliente le va un mensaje que no lo repite.
fn borrador_de_edicion(cuerpo: &[u8]) -> Result<QuadletDraft, AppError> {
    serde_json::from_slice(cuerpo).map_err(|causa| {
        tracing::debug!(error = %causa, "cuerpo de edición de quadlet rechazado");

        AppError::BadRequest("el cuerpo no es una edición de quadlet válida".to_owned())
    })
}

/// `PATCH /api/v1/quadlets/{name}` — reemplaza una quadlet existente.
///
/// El nombre de la ruta es el **nombre de fichero** (`web.container`) y es la
/// identidad del recurso: el cuerpo sólo describe los campos, y editar nunca
/// renombra. La respuesta es la misma envoltura que la creación, con un `200`
/// porque el recurso ya existía.
pub async fn editar(
    State(state): State<AppState>,
    Path(nombre): Path<String>,
    cuerpo: Bytes,
) -> Result<(StatusCode, ApiResponse<QuadletOutcome>), AppError> {
    let draft = borrador_de_edicion(&cuerpo)?;

    let quadlet = service::editar(
        &state.config.quadlet_dir,
        state.systemd.as_ref(),
        &state.pool,
        &nombre,
        draft,
    )
    .await?;

    Ok((StatusCode::OK, ApiResponse::new(quadlet.into())))
}

/// `GET /api/v1/quadlets/{name}` — lee una quadlet existente.
///
/// El nombre de la ruta es el **nombre de fichero** (`web.container`), igual que
/// en la edición y el borrado. La respuesta es el **mismo objeto plano** que el
/// cuerpo de la creación —`name`, `kind` y los campos—, para que un cliente pueda
/// precargar el formulario de edición sin traducir nada. Los opcionales ausentes no
/// viajan y el contenido del fichero nunca se publica.
pub async fn leer(
    State(state): State<AppState>,
    Path(nombre): Path<String>,
) -> Result<(StatusCode, ApiResponse<QuadletDraft>), AppError> {
    let draft = service::leer(&state.config.quadlet_dir, &nombre).await?;

    Ok((StatusCode::OK, ApiResponse::new(draft)))
}

/// `DELETE /api/v1/quadlets/{name}` — borra una quadlet existente.
///
/// El nombre de la ruta es el mismo **nombre de fichero** que en la edición y
/// también la identidad: no hay cuerpo, porque el recurso ya está en la ruta. Un
/// `200` con la misma envoltura confirma el nombre de fichero borrado.
pub async fn borrar(
    State(state): State<AppState>,
    Path(nombre): Path<String>,
) -> Result<(StatusCode, ApiResponse<QuadletOutcome>), AppError> {
    let quadlet = service::borrar(
        &state.config.quadlet_dir,
        state.systemd.as_ref(),
        &state.pool,
        &nombre,
    )
    .await?;

    Ok((StatusCode::OK, ApiResponse::new(quadlet.into())))
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

    /// El cuerpo de una edición válida de contenedor: mismo contrato que la
    /// creación —campos tipados y `deny_unknown_fields`—, sin contenido libre.
    fn edicion(name: &str) -> serde_json::Value {
        json!({
            "name": name,
            "kind": "container",
            "image": "docker.io/library/nginx:latest",
        })
    }

    /// Prepara el directorio dejando un fichero `web.container` con contenido
    /// reconocible.
    fn con_fichero(dir: &std::path::Path) {
        std::fs::write(dir.join("web.container"), "[Container]\nImage=vieja\n")
            .expect("el fichero de prueba se escribe");
    }

    /// Prepara el directorio dejando un **directorio** llamado `web.container`:
    /// hace que la escritura (o el borrado) del sistema de ficheros falle de forma
    /// determinista, sin depender del uid.
    fn con_directorio(dir: &std::path::Path) {
        std::fs::create_dir(dir.join("web.container")).expect("el directorio de prueba se crea");
    }

    /// Sin preparación.
    fn sin_nada(_dir: &std::path::Path) {}

    /// Escenario «Edición correcta»: `200` con `{data}` y el fichero reescrito.
    #[tokio::test]
    async fn editar_una_quadlet_valida_da_200() {
        let entorno = entorno().await;
        con_fichero(entorno.quadlet_dir());

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::PATCH,
            "/api/v1/quadlets/web.container",
            edicion("web"),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cuerpo["data"]["unit_name"], "web.service");
        assert_eq!(cuerpo["data"]["file_name"], "web.container");
        assert_eq!(cuerpo["data"]["kind"], "container");
        assert!(
            cuerpo["data"].get("path").is_none(),
            "la respuesta no puede publicar la ruta del fichero: {cuerpo}"
        );
        assert_eq!(
            std::fs::read_to_string(entorno.quadlet_dir().join("web.container"))
                .expect("el fichero tiene que estar"),
            "[Container]\nImage=docker.io/library/nginx:latest\n"
        );
    }

    /// Escenario «Borrado correcto»: `200` con `{data}` confirmando el nombre de
    /// fichero borrado, y el fichero desaparece.
    #[tokio::test]
    async fn borrar_una_quadlet_existente_da_200() {
        let entorno = entorno().await;
        con_fichero(entorno.quadlet_dir());

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::DELETE,
            "/api/v1/quadlets/web.container",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cuerpo["data"]["file_name"], "web.container");
        assert_eq!(cuerpo["data"]["unit_name"], "web.service");
        assert!(
            !entorno.quadlet_dir().join("web.container").exists(),
            "el fichero tenía que desaparecer"
        );
    }

    /// Escenario «Lectura correcta»: `200` con `{data}` —el tipo y los campos—,
    /// con la misma forma plana que el cuerpo de la creación para poder precargar
    /// el formulario de edición. El contenido del fichero **no** viaja.
    #[tokio::test]
    async fn leer_una_quadlet_existente_da_200() {
        let entorno = entorno().await;
        std::fs::write(
            entorno.quadlet_dir().join("web.container"),
            "[Container]\nImage=nginx\nContainerName=web\n",
        )
        .expect("el fichero de prueba se escribe");

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::GET,
            "/api/v1/quadlets/web.container",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cuerpo["data"]["name"], "web");
        assert_eq!(cuerpo["data"]["kind"], "container");
        assert_eq!(cuerpo["data"]["image"], "nginx");
        assert_eq!(cuerpo["data"]["container_name"], "web");
        assert!(
            cuerpo["data"].get("content").is_none(),
            "la lectura no puede volcar el fichero: {cuerpo}"
        );
        assert!(
            cuerpo.get("error").is_none(),
            "una lectura correcta no puede traer error: {cuerpo}"
        );
    }

    /// Y un campo opcional ausente **no** viaja: la respuesta es un objeto plano
    /// como el cuerpo de la creación, sin `null` que el formulario no espera.
    #[tokio::test]
    async fn leer_una_quadlet_sin_opcionales_no_los_inventa() {
        let entorno = entorno().await;
        std::fs::write(
            entorno.quadlet_dir().join("web.container"),
            "[Container]\nImage=nginx\n",
        )
        .expect("el fichero de prueba se escribe");

        let (_, cuerpo) = peticion(
            &entorno.app(),
            Method::GET,
            "/api/v1/quadlets/web.container",
        )
        .await;

        assert!(
            cuerpo["data"].get("container_name").is_none(),
            "un opcional ausente no puede viajar como `null`: {cuerpo}"
        );
        assert_eq!(cuerpo["data"]["publish_ports"], json!([]));
    }

    /// Escenario «Recurso desconocido»: leer una quadlet que no existe es `404`
    /// con la envoltura y sin datos.
    #[tokio::test]
    async fn leer_algo_que_no_existe_da_404() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::GET,
            "/api/v1/quadlets/nada.container",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(cuerpo["error"]["code"], "not_found");
        assert!(cuerpo.get("data").is_none());
    }

    /// Y un nombre que no es de una quadlet da `400` con el código que toca.
    #[tokio::test]
    async fn leer_un_nombre_invalido_da_400() {
        let entorno = entorno().await;

        let (status, cuerpo) =
            peticion(&entorno.app(), Method::GET, "/api/v1/quadlets/web.desktop").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(cuerpo["error"]["code"], "invalid_kind");
    }

    /// Escenario «Nombre codificado»: el nombre se decodifica **antes** de
    /// validarse, así que un intento de salir del directorio —`%2e%2e%2fetc` es
    /// `../etc`— llega al mismo validador de siempre y no pasa: la identidad de
    /// una quadlet exige un stem que empiece por letra o número y sin barras. La
    /// respuesta es `400` y el directorio de quadlets **no** se toca.
    #[tokio::test]
    async fn un_nombre_codificado_que_intenta_escapar_da_400_sin_tocar_el_disco() {
        let entorno = entorno().await;
        std::fs::write(
            entorno.quadlet_dir().join("web.container"),
            "[Container]\nImage=secreta\n",
        )
        .expect("el fichero de prueba se escribe");

        let casos = [
            "/api/v1/quadlets/%2e%2e%2fetc",           // ../etc
            "/api/v1/quadlets/%2fetc%2fpasswd",        // /etc/passwd
            "/api/v1/quadlets/..%2f..%2fetc%2fpasswd", // ../../etc/passwd
            "/api/v1/quadlets/%2e%2e%2fweb.container", // ../web.container
            "/api/v1/quadlets/web%2f..%2f..%2fetc",    // web/../../etc
        ];

        for uri in casos {
            let (status, cuerpo) = peticion(&entorno.app(), Method::GET, uri).await;

            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "{uri} no dio un 400: {cuerpo}"
            );
            assert!(
                matches!(
                    cuerpo["error"]["code"].as_str(),
                    Some("invalid_name" | "invalid_kind")
                ),
                "{uri} dio un código inesperado: {cuerpo}"
            );
            assert!(
                cuerpo.get("data").is_none(),
                "{uri} devolvió datos de un fichero de fuera: {cuerpo}"
            );
            assert!(
                !cuerpo.to_string().contains("secreta"),
                "{uri} filtró el contenido de una quadlet: {cuerpo}"
            );
        }

        assert_eq!(
            std::fs::read_dir(entorno.quadlet_dir())
                .expect("el directorio se puede leer")
                .count(),
            1,
            "un nombre codificado no puede crear ni borrar nada"
        );
        assert_eq!(
            std::fs::read_to_string(entorno.quadlet_dir().join("web.container"))
                .expect("el fichero sigue ahí"),
            "[Container]\nImage=secreta\n",
            "el fichero de dentro se tocó"
        );
    }

    /// Y el nombre codificado que **sí** es admisible se decodifica y se trata
    /// como el de siempre: `nada%2econtainer` es `nada.container`, así que si no
    /// existe la respuesta es el `404` normal —la codificación no es una puerta
    /// lateral ni un `400` por el camino—.
    #[tokio::test]
    async fn un_nombre_codificado_admisible_da_404_si_no_existe() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::GET,
            "/api/v1/quadlets/nada%2econtainer",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(cuerpo["error"]["code"], "not_found");
        assert!(cuerpo.get("data").is_none());
    }

    /// Las operaciones registradas, en el orden en que se escribieron.
    async fn operaciones(entorno: &crate::test_support::Entorno) -> Vec<(String, String)> {
        sqlx::query_as::<_, (String, String)>(
            "SELECT operation, file_name FROM quadlet_operations ORDER BY id",
        )
        .fetch_all(&entorno.state().pool)
        .await
        .expect("la tabla de operaciones tiene que existir")
    }

    /// Escenario «Operación registrada» (API): la ruta es la capa fina de siempre
    /// —no decide nada—, así que lo que queda apuntado en la base es lo que apunta
    /// el servicio. Se comprueba de punta a punta: una edición deja una `edit` y un
    /// borrado deja una `delete`, con su nombre de fichero.
    #[tokio::test]
    async fn una_edicion_por_la_api_queda_registrada() {
        let entorno = entorno().await;
        con_fichero(entorno.quadlet_dir());

        let (status, _) = peticion_json(
            &entorno.app(),
            Method::PATCH,
            "/api/v1/quadlets/web.container",
            edicion("web"),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            operaciones(&entorno).await,
            [("edit".to_owned(), "web.container".to_owned())]
        );
    }

    /// La otra mitad: el `DELETE` deja su operación con el tipo que le toca.
    #[tokio::test]
    async fn un_borrado_por_la_api_queda_registrado() {
        let entorno = entorno().await;
        con_fichero(entorno.quadlet_dir());

        let (status, _) = peticion(
            &entorno.app(),
            Method::DELETE,
            "/api/v1/quadlets/web.container",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            operaciones(&entorno).await,
            [("delete".to_owned(), "web.container".to_owned())]
        );
    }
    /// Escenario «Fallo del registro»: la edición se completa en disco, pero si el
    /// registro no se puede escribir la respuesta es un `500` genérico —sin causa,
    /// sin dato y sin deshacer el hecho—. Se fuerza de forma determinista cerrando
    /// el pool: cualquier consulta falla y no depende de los permisos del disco.
    #[tokio::test]
    async fn un_fallo_al_registrar_la_operacion_da_500() {
        let entorno = entorno().await;
        con_fichero(entorno.quadlet_dir());
        entorno.state().pool.close().await;

        let (status, cuerpo) = peticion_json(
            &entorno.app(),
            Method::PATCH,
            "/api/v1/quadlets/web.container",
            edicion("web"),
        )
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(cuerpo["error"]["code"], "internal");
        assert!(cuerpo.get("data").is_none());
        assert_eq!(
            std::fs::read_to_string(entorno.quadlet_dir().join("web.container"))
                .expect("el fichero tiene que estar"),
            "[Container]\nImage=docker.io/library/nginx:latest\n",
            "el hecho ocurrido no se deshace porque falle el registro"
        );
    }

    /// Una fila de la tabla de mapeo: cómo se prepara el directorio, qué petición
    /// se lanza y qué desenlace se espera.
    struct Caso {
        preparar: fn(&std::path::Path),
        metodo: Method,
        uri: &'static str,
        cuerpo: Option<serde_json::Value>,
        status: StatusCode,
        code: &'static str,
    }

    /// Escenario «Mapeo de errores»: la tabla de traducción es estable y coincide
    /// en las tres rutas —lectura, edición y borrado—.
    #[tokio::test]
    async fn el_mapeo_de_errores_es_estable_en_las_rutas() {
        let casos = [
            // GET — lectura.
            Caso {
                preparar: sin_nada,
                metodo: Method::GET,
                uri: "/api/v1/quadlets/nada.container",
                cuerpo: None,
                status: StatusCode::NOT_FOUND,
                code: "not_found",
            },
            Caso {
                preparar: sin_nada,
                metodo: Method::GET,
                uri: "/api/v1/quadlets/web.desktop",
                cuerpo: None,
                status: StatusCode::BAD_REQUEST,
                code: "invalid_kind",
            },
            Caso {
                preparar: sin_nada,
                metodo: Method::GET,
                uri: "/api/v1/quadlets/web",
                cuerpo: None,
                status: StatusCode::BAD_REQUEST,
                code: "invalid_name",
            },
            Caso {
                preparar: con_directorio,
                metodo: Method::GET,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: None,
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "read_failed",
            },
            // PATCH — edición.
            Caso {
                preparar: sin_nada,
                metodo: Method::PATCH,
                uri: "/api/v1/quadlets/nada.container",
                cuerpo: Some(edicion("nada")),
                status: StatusCode::NOT_FOUND,
                code: "not_found",
            },
            Caso {
                preparar: con_fichero,
                metodo: Method::PATCH,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: Some(edicion("otro")),
                status: StatusCode::BAD_REQUEST,
                code: "invalid_name",
            },
            Caso {
                preparar: sin_nada,
                metodo: Method::PATCH,
                uri: "/api/v1/quadlets/web.desktop",
                cuerpo: Some(edicion("web")),
                status: StatusCode::BAD_REQUEST,
                code: "invalid_kind",
            },
            Caso {
                preparar: con_fichero,
                metodo: Method::PATCH,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: Some(json!({
                    "name": "web",
                    "kind": "container",
                    "image": "nginx\nUser=root",
                })),
                status: StatusCode::BAD_REQUEST,
                code: "invalid_value",
            },
            Caso {
                preparar: con_directorio,
                metodo: Method::PATCH,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: Some(edicion("web")),
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "write_failed",
            },
            // DELETE — borrado.
            Caso {
                preparar: sin_nada,
                metodo: Method::DELETE,
                uri: "/api/v1/quadlets/nada.container",
                cuerpo: None,
                status: StatusCode::NOT_FOUND,
                code: "not_found",
            },
            Caso {
                preparar: sin_nada,
                metodo: Method::DELETE,
                uri: "/api/v1/quadlets/web.desktop",
                cuerpo: None,
                status: StatusCode::BAD_REQUEST,
                code: "invalid_kind",
            },
            Caso {
                preparar: con_directorio,
                metodo: Method::DELETE,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: None,
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "write_failed",
            },
        ];

        for caso in casos {
            let entorno = entorno().await;
            (caso.preparar)(entorno.quadlet_dir());

            let (status, cuerpo) = match caso.cuerpo.clone() {
                Some(cuerpo) => {
                    peticion_json(&entorno.app(), caso.metodo.clone(), caso.uri, cuerpo).await
                }
                None => peticion(&entorno.app(), caso.metodo.clone(), caso.uri).await,
            };

            assert_eq!(
                status, caso.status,
                "{} {} dio un estado inesperado: {cuerpo}",
                caso.metodo, caso.uri
            );
            assert_eq!(
                cuerpo["error"]["code"], caso.code,
                "{} {} dio un código inesperado: {cuerpo}",
                caso.metodo, caso.uri
            );
            assert!(
                cuerpo.get("data").is_none(),
                "un error no puede traer datos: {cuerpo}"
            );
        }
    }

    /// Una fila de la tabla de mapeo de los fallos de systemd.
    struct CasoSystemd {
        error: fn() -> SystemdError,
        metodo: Method,
        uri: &'static str,
        cuerpo: Option<serde_json::Value>,
        status: StatusCode,
        code: &'static str,
    }

    fn fallo_de_recarga() -> SystemdError {
        SystemdError::ReloadFailed {
            causa: Box::new(std::io::Error::other("el gestor dijo que no")),
        }
    }

    fn unidad_rechazada() -> SystemdError {
        SystemdError::UnitRejected {
            unit: "web.service".to_owned(),
            causa: Box::new(std::io::Error::other("UnitMasked")),
        }
    }

    fn fallo_de_transporte() -> SystemdError {
        SystemdError::Transport {
            causa: Box::new(std::io::Error::other(
                "did not receive a reply for /org/freedesktop/systemd1",
            )),
        }
    }

    /// Escenario «Error de recarga» y «Error de transporte»: tras editar o
    /// borrar, un fallo del gestor se mapea igual que en la creación.
    #[tokio::test]
    async fn los_fallos_de_systemd_se_mapean_igual_en_ambas_rutas() {
        let casos = [
            CasoSystemd {
                error: fallo_de_recarga,
                metodo: Method::PATCH,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: Some(edicion("web")),
                status: StatusCode::BAD_GATEWAY,
                code: "systemd_reload_failed",
            },
            CasoSystemd {
                error: unidad_rechazada,
                metodo: Method::PATCH,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: Some(edicion("web")),
                status: StatusCode::BAD_GATEWAY,
                code: "systemd_unit_rejected",
            },
            CasoSystemd {
                error: fallo_de_transporte,
                metodo: Method::PATCH,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: Some(edicion("web")),
                status: StatusCode::SERVICE_UNAVAILABLE,
                code: "systemd_unavailable",
            },
            CasoSystemd {
                error: fallo_de_recarga,
                metodo: Method::DELETE,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: None,
                status: StatusCode::BAD_GATEWAY,
                code: "systemd_reload_failed",
            },
            CasoSystemd {
                error: unidad_rechazada,
                metodo: Method::DELETE,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: None,
                status: StatusCode::BAD_GATEWAY,
                code: "systemd_unit_rejected",
            },
            CasoSystemd {
                error: fallo_de_transporte,
                metodo: Method::DELETE,
                uri: "/api/v1/quadlets/web.container",
                cuerpo: None,
                status: StatusCode::SERVICE_UNAVAILABLE,
                code: "systemd_unavailable",
            },
        ];

        for caso in casos {
            let entorno = entorno_con(Arc::new(DobleQueFalla::con(caso.error))).await;
            con_fichero(entorno.quadlet_dir());

            let (status, cuerpo) = match caso.cuerpo.clone() {
                Some(cuerpo) => {
                    peticion_json(&entorno.app(), caso.metodo.clone(), caso.uri, cuerpo).await
                }
                None => peticion(&entorno.app(), caso.metodo.clone(), caso.uri).await,
            };

            assert_eq!(
                status, caso.status,
                "{} {} dio un estado inesperado: {cuerpo}",
                caso.metodo, caso.uri
            );
            assert_eq!(
                cuerpo["error"]["code"], caso.code,
                "{} {} dio un código inesperado: {cuerpo}",
                caso.metodo, caso.uri
            );
        }
    }

    /// Escenario «Fallbacks preservados en las rutas de edición y borrado»: un
    /// método que no esté declarado sobre `/api/v1/quadlets/{name}` da `405` con
    /// la envoltura —incluido el fallback del panel—. `GET`, `PATCH` y `DELETE`
    /// están declarados, así que la prueba usa los que no lo están.
    #[tokio::test]
    async fn un_metodo_no_permitido_sobre_quadlets_name_da_405() {
        let entorno = entorno().await;

        for metodo in [Method::POST, Method::PUT] {
            let (status, cuerpo) = peticion(
                &entorno.app(),
                metodo.clone(),
                "/api/v1/quadlets/web.container",
            )
            .await;

            assert_eq!(
                status,
                StatusCode::METHOD_NOT_ALLOWED,
                "{metodo} sobre la ruta de una quadlet tendría que dar 405"
            );
            assert_eq!(cuerpo["error"]["code"], "method_not_allowed");

            // Y el fallback del panel no se come la ruta nueva.
            let (status, _) =
                peticion_texto(&entorno.app(), metodo, "/api/v1/quadlets/web.container").await;
            assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        }
    }

    /// Y una ruta de la API que no existe sigue dando el `404` de la casa, con su
    /// envoltura, incluso colgando de la ruta de las quadlets.
    #[tokio::test]
    async fn una_ruta_inexistente_bajo_quadlets_da_404() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::GET,
            "/api/v1/quadlets/web.container/extra",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(cuerpo["error"]["code"], "not_found");
    }
}
