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
use tokio::sync::{broadcast, watch};
use tower::ServiceExt;

use crate::config::Config;
use crate::db;
use crate::state::{AppState, CAPACIDAD_DE_EVENTOS};
use crate::systemd::fake::FakeSystemd;
use crate::systemd::{SystemdClient, SystemdError, UnitWatcher};

/// Lo que una prueba necesita para ejercer la aplicación.
pub struct Entorno {
    state: AppState,
    /// El disparador del cierre.
    ///
    /// Vive aquí —y no dentro del estado— porque el estado sólo necesita poder
    /// **mirar** si el proceso se está apagando: quien lo dispara es quien arranca
    /// el servidor, y en una prueba eso es la propia prueba.
    cierre: watch::Sender<bool>,
    /// El directorio de la base temporal.
    ///
    /// Se guarda —y no se suelta al terminar de componer— porque el directorio se
    /// borra cuando el valor se suelta: sin esta referencia, la base desaparecería
    /// en mitad de la prueba.
    _dir: tempfile::TempDir,
    /// El directorio de quadlets de la prueba.
    ///
    /// Es temporal y **propio de cada entorno**, para que una prueba no escriba
    /// donde otra mira. Vive aquí por la misma razón que el de la base: soltarlo
    /// lo borraría antes de que la prueba termine.
    _quadlets: tempfile::TempDir,
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

    /// El directorio en el que la creación escribe las quadlets.
    pub fn quadlet_dir(&self) -> &std::path::Path {
        &self.state.config.quadlet_dir
    }

    /// Dispara el cierre, como haría la señal de terminación del proceso.
    ///
    /// Es lo que permite medir el apagado ordenado sin mandar una señal de verdad:
    /// el fan-out termina y los flujos SSE acaban porque los dos miran este mismo
    /// canal.
    pub fn cerrar(&self) {
        // Sin receptores el envío falla, y no es un error: nadie mira.
        let _ = self.cierre.send(true);
    }
}

/// Un entorno con la configuración mínima que arranca y el doble de muestra.
pub async fn entorno() -> Entorno {
    entorno_con(Arc::new(FakeSystemd::demo())).await
}

/// Un entorno con el doble de muestra y un panel estático servido desde `panel`.
///
/// Es la puerta de las pruebas del requisito «Servicio del panel»: la misma
/// aplicación, con la única diferencia de que lo que no es de la API se sirve del
/// directorio en vez de contestar el `404` de la casa.
pub async fn entorno_con_panel(panel: &std::path::Path) -> Entorno {
    montar(Arc::new(FakeSystemd::demo()), Some(panel)).await
}

/// Un entorno con el cliente de systemd que se le inyecte.
///
/// Es la puerta que permite medir desenlaces que el doble de muestra no alcanza
/// —un job que falla, un gestor que no contesta— **a través del manejador**, que
/// es donde se decide el código HTTP: con un cliente cableado dentro del
/// manejador, esos caminos sólo se podrían medir en la traducción suelta.
pub async fn entorno_con(systemd: Arc<dyn SystemdClient>) -> Entorno {
    montar(systemd, None).await
}

/// Un entorno con el doble de systemd, devolviendo además su trazo de vigilante.
///
/// Es la puerta del canal en vivo: el fan-out necesita las **dos** caras del mismo
/// objeto —el listado que recalcula y los avisos que lo disparan—, y el doble las
/// tiene. Se devuelven juntas porque tienen que ser el mismo doble: con dos
/// distintos, un aviso no tendría nada que ver con lo que se lista.
pub async fn entorno_con_vigilante(doble: Arc<FakeSystemd>) -> (Entorno, Arc<dyn UnitWatcher>) {
    let vigilante: Arc<dyn UnitWatcher> = doble.clone();

    (entorno_con(doble).await, vigilante)
}

/// Compone el entorno: base temporal, configuración, canales y estado.
async fn montar(systemd: Arc<dyn SystemdClient>, panel: Option<&std::path::Path>) -> Entorno {
    let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
    let dsn = format!("sqlite://{}/podpot.db", dir.path().display());

    // La base es un fichero y no `sqlite::memory:` a propósito: con el pool, cada
    // conexión a una base en memoria es una base **distinta**, así que el esquema
    // que migrara una conexión no lo verían las demás.
    let pool = db::preparar(&dsn)
        .await
        .expect("la base temporal de la prueba tiene que abrirse y migrarse");

    // El directorio de quadlets también es temporal: la creación **escribe** en
    // él, así que no puede apuntar al del usuario que compila.
    let quadlets = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");

    // Los dos canales del canal en vivo. El extremo «de escritura» de cada uno se
    // queda aquí: el de los eventos no hace falta —el fan-out publica con el que
    // guarda el estado—, pero el del cierre sí, y es lo que `cerrar()` dispara.
    let (eventos, _) = broadcast::channel(CAPACIDAD_DE_EVENTOS);
    let (cierre, apagado) = watch::channel(false);

    Entorno {
        state: AppState::new(
            config_de_prueba_con(&dsn, panel, quadlets.path()),
            pool,
            systemd,
            eventos,
            apagado,
        ),
        cierre,
        _dir: dir,
        _quadlets: quadlets,
    }
}

/// La configuración que usan las pruebas: loopback en el puerto por defecto, la
/// base temporal de la prueba, un directorio de quadlets temporal y el doble de
/// systemd, para no depender del bus. Con panel estático si se indica un
/// directorio.
pub fn config_de_prueba_con(
    dsn: &str,
    panel: Option<&std::path::Path>,
    quadlets: &std::path::Path,
) -> Config {
    let panel = panel.map(|ruta| {
        ruta.to_str()
            .expect("la ruta del panel de prueba es UTF-8")
            .to_owned()
    });
    let quadlets = quadlets
        .to_str()
        .expect("la ruta de quadlets de prueba es UTF-8")
        .to_owned();

    Config::from_source(|clave| match clave {
        "BIND_ADDRESS" => Some("127.0.0.1:8080".to_owned()),
        "DATABASE_URL" => Some(dsn.to_owned()),
        "PODPOT_SYSTEMD" => Some("fake".to_owned()),
        "PODPOT_STATIC_DIR" => panel.clone(),
        "PODPOT_QUADLET_DIR" => Some(quadlets.clone()),
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

    async fn reload(&self) -> Result<(), SystemdError> {
        Err(self.error())
    }
}

/// Un doble que se queda **dentro** del listado hasta que la prueba lo deja seguir.
///
/// Es la puerta de las pruebas de orden: con el listado retenido, la prueba sabe
/// cuándo el manejador está dentro de él —y, por tanto, cuándo ya pasó por todo lo
/// que va antes— y puede publicar un cambio justo en ese hueco. Sin poder retener
/// el listado, la carrera entre suscribirse y listar no se puede medir: sólo se
/// podría mirar el resultado, que es idéntico pase lo que pase.
pub struct DobleRetenido {
    /// Lo que devuelve el listado cuando se le deja seguir.
    unidades: Vec<crate::systemd::UnitStatus>,
    /// Se completa al entrar en `quadlets()`.
    dentro: Arc<tokio::sync::Notify>,
    /// Se espera, ya dentro, para dejar terminar el listado.
    seguir: Arc<tokio::sync::Notify>,
}

impl DobleRetenido {
    /// Un doble que devuelve `unidades` en cuanto se le deja seguir.
    pub fn con(unidades: Vec<crate::systemd::UnitStatus>) -> Self {
        Self {
            unidades,
            dentro: Arc::new(tokio::sync::Notify::new()),
            seguir: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Se completa cuando el doble entra en el listado.
    pub fn dentro(&self) -> Arc<tokio::sync::Notify> {
        self.dentro.clone()
    }

    /// Deja que el listado termine.
    pub fn seguir(&self) -> Arc<tokio::sync::Notify> {
        self.seguir.clone()
    }
}

impl std::fmt::Debug for DobleRetenido {
    /// El trait exige `Debug`; se nombra el doble y no se publica el listado
    /// retenido, que no aporta nada al diagnóstico.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DobleRetenido")
    }
}

#[async_trait]
impl SystemdClient for DobleRetenido {
    /// El listado retenido: avisa de que está dentro y espera a que la prueba lo
    /// deje seguir.
    async fn quadlets(&self) -> Result<Vec<crate::systemd::UnitStatus>, SystemdError> {
        self.dentro.notify_one();
        self.seguir.notified().await;

        Ok(self.unidades.clone())
    }

    async fn status(
        &self,
        _unit: &crate::systemd::UnitName,
    ) -> Result<crate::systemd::UnitStatus, SystemdError> {
        Err(no_se_usa())
    }

    async fn action(
        &self,
        _unit: &crate::systemd::UnitName,
        _action: crate::systemd::UnitAction,
    ) -> Result<crate::systemd::UnitStatus, SystemdError> {
        Err(no_se_usa())
    }

    async fn reload(&self) -> Result<(), SystemdError> {
        Err(no_se_usa())
    }
}

/// El error de las llamadas que un doble de prueba no ejerce.
///
/// No es una ausencia fingida: si alguna prueba llegara aquí por error, lo que
/// quiere saber es que llamó donde no debía, y eso lo dice el texto.
fn no_se_usa() -> SystemdError {
    SystemdError::Transport {
        causa: Box::new(std::io::Error::other(
            "el doble retenido sólo ejerce el listado",
        )),
    }
}

/// Lanza una petición contra el router y devuelve la respuesta **sin leer** el
/// cuerpo.
///
/// Es lo que necesitan las pruebas del canal en vivo: un flujo `text/event-stream`
/// no termina —es justo su razón de ser—, así que leerlo entero colgaría la
/// prueba. Con la respuesta en la mano se mira la cabecera y se leen los trozos que
/// van llegando.
pub async fn peticion_cruda(app: &Router, method: Method, uri: &str) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .expect("la petición de prueba se construye siempre"),
        )
        .await
        .expect("el router no puede fallar al atender")
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

/// Lanza una petición con un cuerpo JSON contra el router.
///
/// Es la mitad que [`peticion`] no cubre: las rutas que leen un cuerpo —la de
/// creación de quadlets— necesitan que el JSON llegue de verdad, y con el
/// `Content-Type` puesto, o axum no lo mira.
pub async fn peticion_json(
    app: &Router,
    method: Method,
    uri: &str,
    cuerpo: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let peticion = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(cuerpo.to_string()))
        .expect("la petición de prueba se construye siempre");

    let respuesta = app
        .clone()
        .oneshot(peticion)
        .await
        .expect("el router no puede fallar al atender");

    let status = respuesta.status();

    (status, body_json(respuesta).await)
}

/// Lanza una petición contra el router y devuelve el código y el cuerpo como
/// texto.
///
/// Es lo que necesitan las pruebas del panel: su respuesta es el `index.html` o
/// el fichero del build, no la envoltura JSON de la API.
pub async fn peticion_texto(app: &Router, method: Method, uri: &str) -> (StatusCode, String) {
    let respuesta = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .expect("la petición de prueba se construye siempre"),
        )
        .await
        .expect("el router no puede fallar al atender");

    let status = respuesta.status();
    let bytes = respuesta
        .into_body()
        .collect()
        .await
        .expect("el cuerpo de la respuesta se puede leer")
        .to_bytes();

    (status, String::from_utf8_lossy(&bytes).into_owned())
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
