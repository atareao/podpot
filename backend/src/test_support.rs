//! Ayudas compartidas por las pruebas de la casa.
//!
//! Vive detrás de `#[cfg(test)]`: no forma parte del contrato del crate y no
//! compila en el binario de producción.

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::config::Config;
use crate::db;
use crate::state::AppState;
use crate::systemd::fake::FakeSystemd;
use crate::systemd::{SystemdClient, SystemdError};

/// Lo que una prueba necesita para ejercer la aplicación.
pub struct Entorno {
    state: AppState,
    /// El directorio de la base temporal.
    ///
    /// Se guarda —y no se suelta al terminar de componer— porque el directorio se
    /// borra cuando el valor se suelta: sin esta referencia, la base desaparecería
    /// en mitad de la prueba.
    _dir: tempfile::TempDir,
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

/// Un entorno con la configuración mínima que arranca y el doble de muestra.
pub async fn entorno() -> Entorno {
    entorno_con(Arc::new(FakeSystemd::demo())).await
}

/// Un entorno con el cliente de systemd que se le inyecte.
///
/// Es la puerta que permite medir desenlaces que el doble de muestra no alcanza
/// —un job que falla, un gestor que no contesta— **a través del manejador**, que
/// es donde se decide el código HTTP: con un cliente cableado dentro del
/// manejador, esos caminos sólo se podrían medir en la traducción suelta.
pub async fn entorno_con(systemd: Arc<dyn SystemdClient>) -> Entorno {
    let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
    let dsn = format!("sqlite://{}/podpot.db", dir.path().display());

    // La base es un fichero y no `sqlite::memory:` a propósito: con el pool, cada
    // conexión a una base en memoria es una base **distinta**, así que el esquema
    // que migrara una conexión no lo verían las demás.
    let pool = db::preparar(&dsn)
        .await
        .expect("la base temporal de la prueba tiene que abrirse y migrarse");

    Entorno {
        state: AppState::new(config_de_prueba(&dsn), pool, systemd),
        _dir: dir,
    }
}

/// La configuración que usan las pruebas: loopback en el puerto por defecto, la
/// base temporal de la prueba y el doble de systemd, para no depender del bus.
pub fn config_de_prueba(dsn: &str) -> Config {
    Config::from_source(|clave| match clave {
        "BIND_ADDRESS" => Some("127.0.0.1:8080".to_owned()),
        "DATABASE_URL" => Some(dsn.to_owned()),
        "PODPOT_SYSTEMD" => Some("fake".to_owned()),
        _ => None,
    })
    .expect("la configuración de prueba tiene que ser válida")
}

/// Un cliente que falla siempre, con el error que diga su fábrica.
///
/// El error se fabrica en cada llamada —y no se guarda ya hecho— porque
/// [`SystemdError`] no es clonable: lleva dentro la causa de una biblioteca, y
/// fingir que se puede copiar sería fingir que todas las llamadas fallan por el
/// mismo objeto y no por el mismo motivo.
pub struct DobleQueFalla {
    fabrica: Box<dyn Fn() -> SystemdError + Send + Sync>,
}

impl DobleQueFalla {
    /// Un doble que falla con lo que devuelva la fábrica.
    pub fn con(fabrica: impl Fn() -> SystemdError + Send + Sync + 'static) -> Self {
        Self {
            fabrica: Box::new(fabrica),
        }
    }

    /// El error de esta llamada.
    fn error(&self) -> SystemdError {
        (self.fabrica)()
    }
}

impl std::fmt::Debug for DobleQueFalla {
    /// El trait exige `Debug`; la fábrica no lo es, así que se nombra el doble y
    /// no se finge publicar un error que todavía no existe.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DobleQueFalla")
    }
}

#[async_trait]
impl SystemdClient for DobleQueFalla {
    async fn quadlets(&self) -> Result<Vec<crate::systemd::UnitStatus>, SystemdError> {
        Err(self.error())
    }

    async fn status(
        &self,
        _unit: &crate::systemd::UnitName,
    ) -> Result<crate::systemd::UnitStatus, SystemdError> {
        Err(self.error())
    }

    async fn action(
        &self,
        _unit: &crate::systemd::UnitName,
        _action: crate::systemd::UnitAction,
    ) -> Result<crate::systemd::UnitStatus, SystemdError> {
        Err(self.error())
    }
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
