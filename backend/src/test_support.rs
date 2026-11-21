//! Ayudas compartidas por las pruebas de la casa.
//!
//! Vive detrás de `#[cfg(test)]`: no forma parte del contrato del crate y no
//! compila en el binario de producción.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::http::{HeaderName, Method, Request, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use tokio::sync::{broadcast, watch};
use tower::ServiceExt;

use crate::config::Config;
use crate::db;
use crate::registros::fake::FakeRegistros;
use crate::secret::SecretString;
use crate::state::{AppState, CAPACIDAD_DE_EVENTOS};
use crate::systemd::fake::FakeSystemd;
use crate::systemd::{SystemdClient, SystemdError, UnitWatcher};

/// Usuario distinguible de la sesión de pruebas.
pub const SUB: &str = "usuario-distinguible-1";
/// Correo distinguible de la sesión de pruebas.
pub const EMAIL: &str = "operador-distinguible@example.test";
/// Identificador público del cliente OIDC de pruebas.
pub const OIDC_CLIENT_ID: &str = "podpot-panel";
/// Secreto del cliente OIDC de pruebas: solo tiene que estar presente.
pub const OIDC_CLIENT_SECRET: &str = "secreto-de-cliente-oidc-distinguible";
/// Secreto de firma de la sesión de las pruebas, con longitud de sobra.
pub const SECRETO: &str = "secreto-de-firma-distinguible-000000000000000000000000000000";
/// Un emisor donde nadie escucha: cualquier login que una prueba dispare por
/// accidente falla al instante contra loopback, sin resolver un DNS ni salir a
/// la red.
pub const EMISOR_CAIDO: &str = "http://127.0.0.1:1";

/// Lo que una prueba necesita para ejercer la aplicación.
pub struct Entorno {
    state: AppState,
    /// El doble del progreso de los trabajos.
    ///
    /// Es un doble **aparte** del cliente de systemd —y no el mismo objeto— porque
    /// el entorno admite cualquier `Arc<dyn SystemdClient>` y de ahí no se puede
    /// recuperar el puerto. No hace falta que sean el mismo: el progreso sólo empuja
    /// eventos y nadie los consulta, así que un doble propio para los jobs basta
    /// para medir el canal y su endpoint.
    trabajos: Arc<FakeSystemd>,
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

    /// El doble del progreso de los trabajos, para que la prueba emita eventos.
    ///
    /// Es la puerta del canal de trabajos: sin bus no hay señales de
    /// `JobNew`/`JobRemoved` que reenviar, así que los eventos de un job los emite
    /// la prueba con [`FakeSystemd::emitir`] y el endpoint los lee del otro lado.
    pub fn trabajos(&self) -> &Arc<FakeSystemd> {
        &self.trabajos
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

/// Un entorno con la fuente de registros que se le inyecte.
///
/// Es la puerta del canal de logs: el doble se construye **antes** de montar
/// —con su cola inicial o fingiendo una unidad sin contenedor— y la prueba lo
/// sigue manejando después para emitir líneas con la conexión ya abierta.
pub async fn entorno_con_registros(registros: Arc<FakeRegistros>) -> Entorno {
    montar_con_vars(Arc::new(FakeSystemd::demo()), None, &[], registros).await
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

/// Un entorno en modo `oidc` apuntando a un emisor donde nadie escucha.
///
/// Es la puerta de las pruebas que no necesitan un descubrimiento que funcione:
/// la capa de sesión está montada, pero cualquier login que se dispare por
/// accidente falla al instante contra loopback, sin salir a la red. Las pruebas
/// que quieren un descubrimiento de verdad usan [`entorno_oidc_con_emisor`] con
/// el proveedor falso.
pub async fn entorno_oidc() -> Entorno {
    entorno_oidc_con_emisor(EMISOR_CAIDO).await
}

/// Un entorno en modo `oidc` apuntando a un emisor dado.
///
/// Es lo que permite que una prueba hable con el proveedor falso local: el emisor
/// entra por parámetro en vez de fijarse en el entorno del proceso.
pub async fn entorno_oidc_con_emisor(emisor: &str) -> Entorno {
    montar_con_vars(
        Arc::new(FakeSystemd::demo()),
        None,
        &[
            ("PODPOT_OIDC_ISSUER_URL", emisor),
            ("PODPOT_OIDC_CLIENT_ID", OIDC_CLIENT_ID),
            ("PODPOT_OIDC_CLIENT_SECRET", OIDC_CLIENT_SECRET),
            ("PODPOT_SECRET", SECRETO),
        ],
        Arc::new(FakeRegistros::new()),
    )
    .await
}

/// Un entorno en modo `oidc` con la fuente de registros que se le inyecte.
///
/// Es la puerta del canal de logs bajo el *gate*: con sesión, el flujo se abre y
/// las líneas del doble tienen que llegar por él.
pub async fn entorno_oidc_con_registros(registros: Arc<FakeRegistros>) -> Entorno {
    montar_con_vars(
        Arc::new(FakeSystemd::demo()),
        None,
        &[
            ("PODPOT_OIDC_ISSUER_URL", EMISOR_CAIDO),
            ("PODPOT_OIDC_CLIENT_ID", OIDC_CLIENT_ID),
            ("PODPOT_OIDC_CLIENT_SECRET", OIDC_CLIENT_SECRET),
            ("PODPOT_SECRET", SECRETO),
        ],
        registros,
    )
    .await
}

/// Compone el entorno: base temporal, configuración, canales y estado.
async fn montar(systemd: Arc<dyn SystemdClient>, panel: Option<&std::path::Path>) -> Entorno {
    montar_con_vars(systemd, panel, &[], Arc::new(FakeRegistros::new())).await
}

/// Compone el entorno con las variables de entorno extra que pida la prueba.
async fn montar_con_vars(
    systemd: Arc<dyn SystemdClient>,
    panel: Option<&std::path::Path>,
    extra: &[(&str, &str)],
    registros: Arc<FakeRegistros>,
) -> Entorno {
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

    // El doble del progreso de los trabajos: propio del entorno, para que una prueba
    // pueda emitir eventos de job sin tocar un bus.
    let trabajos = Arc::new(FakeSystemd::new());

    Entorno {
        state: AppState::new(
            config_de_prueba_con_vars(&dsn, panel, quadlets.path(), extra),
            pool,
            systemd,
            trabajos.clone(),
            registros.clone(),
            eventos,
            apagado,
        ),
        trabajos,
        cierre,
        _dir: dir,
        _quadlets: quadlets,
    }
}

/// La configuración de prueba con variables extra, para los modos que necesitan
/// alguna —el `oidc`, por ejemplo—.
pub fn config_de_prueba_con_vars(
    dsn: &str,
    panel: Option<&std::path::Path>,
    quadlets: &std::path::Path,
    extra: &[(&str, &str)],
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
        // El socket se declara y no se deja al defecto: una prueba no puede
        // depender de que la máquina que compila tenga `XDG_RUNTIME_DIR`. Nada se
        // conecta a él —la fuente de registros es el doble—, así que la ruta sólo
        // tiene que ser válida.
        "PODPOT_PODMAN_SOCKET" => Some("/run/podpot-de-prueba/podman.sock".to_owned()),
        otra => extra
            .iter()
            .find(|(nombre, _)| *nombre == otra)
            .map(|(_, valor)| (*valor).to_owned()),
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

/// El valor de una cabecera de la respuesta, como texto.
///
/// Es lo que necesitan las pruebas del flujo de entrada: la `Location` de un
/// `302` y la `Set-Cookie` de la sesión son cabeceras, no cuerpo.
pub fn header_texto(respuesta: &Response, nombre: HeaderName) -> Option<String> {
    respuesta
        .headers()
        .get(nombre)
        .and_then(|valor| valor.to_str().ok())
        .map(str::to_owned)
}

/// Una cookie de sesión válida, firmada con el secreto de las pruebas.
///
/// Es lo que permite entrar a la superficie de negocio sin recorrer el flujo
/// OIDC entero: el *gate* sólo mira la firma y la vigencia, así que una cookie
/// emitida por la misma función que la emite de verdad vale.
pub fn cookie_de_sesion() -> String {
    let token = crate::auth::session::issue(
        &SecretString::new(SECRETO.to_owned()),
        SUB.to_owned(),
        EMAIL.to_owned(),
    )
    .expect("la sesión de prueba tiene que poder emitirse");

    format!("{}={}", crate::auth::session::SESSION_COOKIE_NAME, token)
}

/// Lanza una petición con una cookie contra el router y devuelve el código y el
/// cuerpo JSON.
pub async fn peticion_con_cookie(
    app: &Router,
    method: Method,
    uri: &str,
    cookie: &str,
) -> (StatusCode, serde_json::Value) {
    let respuesta = peticion_cruda_con_cookie(app, method, uri, cookie).await;
    let status = respuesta.status();

    (status, body_json(respuesta).await)
}

/// Lanza una petición con una cookie y devuelve la respuesta **sin leer** el
/// cuerpo.
///
/// Es lo que necesitan las pruebas del canal en vivo y de las cabeceras: un flujo
/// `text/event-stream` no termina, así que leerlo entero colgaría la prueba.
pub async fn peticion_cruda_con_cookie(
    app: &Router,
    method: Method,
    uri: &str,
    cookie: &str,
) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .expect("la petición de prueba se construye siempre"),
        )
        .await
        .expect("el router no puede fallar al atender")
}

/// Un proveedor OIDC falso local para las pruebas del flujo de entrada.
///
/// **Por qué un servidor de verdad y no dobles de las funciones de red.** Un
/// doble de `reqwest` —o de la propia capa de descubrimiento— devolvería lo que
/// la prueba le pidiera, y con eso el flujo pasaría sin tocar la red. Eso dejaría
/// sin probar precisamente lo central: que el ID token se **verifica** —firma
/// `RS256` contra el `jwks_uri`, `iss`, `aud`, `exp`/`iat` y `nonce`—. Un doble
/// de la red haría que la verificación de la firma nunca se ejercitara: se
/// probaría que el código llama a las cosas, no que las cosas funcionan.
///
/// Por eso el arnés es `wiremock` sirviendo un proveedor de verdad: un par RSA
/// **real** (con `rsa`), el ID token firmado **`RS256` de verdad** con ese par, y
/// la clave pública publicada como JWK en el `jwks_uri`. La firma que se publica
/// y la que se verifica son la misma matemática que en producción.
///
/// **Ninguna prueba sale a internet**: `wiremock` levanta un servidor HTTP real
/// atado a `127.0.0.1` con un puerto efímero, y todas las peticiones se quedan en
/// la máquina.
pub mod proveedor_falso {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use jsonwebtoken::jwk::{Jwk, JwkSet};
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use rsa::RsaPrivateKey;
    use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
    use serde::{Deserialize, Serialize};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    use super::{EMAIL, SUB};

    /// Ruta del documento de descubrimiento.
    const RUTA_DESCUBRIMIENTO: &str = "/.well-known/openid-configuration";
    /// Ruta del canje del código.
    const RUTA_TOKEN: &str = "/token";
    /// Ruta del conjunto de claves públicas.
    const RUTA_JWKS: &str = "/jwks";

    /// `kid` con el que el proveedor falso etiqueta su clave pública y sus tokens.
    ///
    /// Es coherente entre la cabecera del JWT y el JWK a propósito: la
    /// verificación tiene que encontrar la clave por su `kid` y comprobar la firma
    /// contra ella, no tropezar antes con una clave ausente.
    pub const KID: &str = "podpot-clave-de-prueba";

    /// `kid` que el `jwks_uri` **no** publica.
    ///
    /// Sirve para fabricar un token cuya firma cuadra con la clave publicada pero
    /// cuya cabecera apunta a una clave que el conjunto no tiene: el rechazo sale
    /// de la búsqueda por `kid`, que es anterior a la firma.
    pub const KID_AJENO: &str = "podpot-clave-que-no-se-publica";

    /// `kid` de la clave que el proveedor publica **después** de rotar.
    ///
    /// Es el complemento de [`KID`] para el escenario de rotación: el conjunto
    /// cacheado publica [`KID`] y el `jwks_uri` ya publica éste, así que un token
    /// firmado con la clave nueva solo entra si el flujo **refresca** el conjunto.
    pub const KID_ROTADO: &str = "podpot-clave-rotada";

    /// Claims del ID token que el proveedor falso firma.
    ///
    /// Los valores por defecto de [`IdToken::valido`] son distinguibles: `sub` y
    /// `email` son los de la sesión de pruebas, y `nonce` viaja tal cual, de modo
    /// que las pruebas localizan la identidad literalmente.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct IdToken {
        /// Emisor del token (`iss`).
        pub iss: String,
        /// Audiencia (`aud`): el `client_id` con el que se pidió la sesión.
        pub aud: String,
        /// Usuario (`sub`).
        pub sub: String,
        /// Correo (`email`).
        pub email: String,
        /// `nonce` devuelto: se compara con el emitido en el login.
        pub nonce: String,
        /// Caducidad, en segundos desde la época (`exp`).
        pub exp: u64,
        /// Emisión, en segundos desde la época (`iat`).
        pub iat: u64,
    }

    impl IdToken {
        /// Un ID token válido para el emisor, el `client_id` y el `nonce` dados.
        ///
        /// Caduca a diez minutos vista y se emite ahora: dentro de la ventana de
        /// vigencia y sin adelantarse al reloj. Las pruebas que quieren un token
        /// que falle **un** campo parten de aquí y lo tuercen.
        pub fn valido(issuer: &str, client_id: &str, nonce: &str) -> Self {
            let ahora = jsonwebtoken::get_current_timestamp();

            Self {
                iss: issuer.to_owned(),
                aud: client_id.to_owned(),
                sub: SUB.to_owned(),
                email: EMAIL.to_owned(),
                nonce: nonce.to_owned(),
                exp: ahora + 600,
                iat: ahora,
            }
        }
    }

    /// La clave con la que el proveedor firma **después** de rotar.
    ///
    /// Es opaca a propósito: la prueba la recibe de [`ProveedorFalso::rotar_clave`] y
    /// la devuelve en [`ProveedorFalso::firmar_con_rotada`], sin poder mirarla ni
    /// confundirla con la vigente.
    pub struct ClaveRotada {
        /// Clave privada nueva, con la que se firman los tokens posteriores.
        clave: EncodingKey,
    }

    /// Proveedor OIDC falso: un servidor `wiremock` que sirve el descubrimiento,
    /// el `token_endpoint` y el `jwks_uri`, con un par RSA real.
    pub struct ProveedorFalso {
        /// El servidor real, vivo mientras lo esté el proveedor.
        server: MockServer,
        /// Emisor: la base contra la que se construyen los endpoints.
        pub issuer: String,
        /// Clave con la que se firman los ID tokens **válidos**.
        clave: EncodingKey,
        /// Clave que el `jwks_uri` **no** publica: firma los tokens que deben
        /// rechazarse por una firma que no cuadra.
        clave_no_publicada: EncodingKey,
        /// Clave `HMAC` cuyo secreto es la **clave pública** del proveedor: firma
        /// el token `HS256` de la confusión de algoritmo.
        ///
        /// El material es público —lo publica el propio `jwks_uri`—, así que
        /// cualquiera puede fabricar este token. Es exactamente lo que lo hace el
        /// ataque: lo único que lo para es que la verificación fije `RS256` en vez
        /// de leer el algoritmo de la cabecera.
        clave_confusa: EncodingKey,
        /// Lo que el `jwks_uri` publica **en este momento**.
        ///
        /// Es un catálogo vivo y no un mock por rotación: el `jwks_uri` se monta una
        /// sola vez y lo que cambia es lo que publica. Montar un mock nuevo en cada
        /// rotación dejaría el enrutado en manos de la prioridad de `wiremock` y el
        /// conteo de lecturas repartido entre varios mocks.
        publicadas: Arc<Mutex<Vec<Jwk>>>,
        /// Si el `jwks_uri` está caído: desde aquí, sus lecturas contestan `500`.
        ///
        /// Es el escenario del proveedor que no responde al refrescar el conjunto,
        /// y se enciende a voluntad para que la prueba decida **cuándo** se cae.
        caido: Arc<AtomicBool>,
        /// Lecturas del `jwks_uri` que el proveedor ha atendido.
        ///
        /// Lo cuenta el propio doble —y no `received_requests`— porque lo que las
        /// pruebas de la caché necesitan medir es **cuántas veces se ha ido a por el
        /// conjunto**, incluidas las lecturas que el proveedor ha contestado con un
        /// error.
        peticiones_al_jwks: Arc<AtomicUsize>,
    }

    impl ProveedorFalso {
        /// Levanta el proveedor y monta el descubrimiento y el JWKS.
        ///
        /// El documento de descubrimiento **no** publica `userinfo_endpoint`, y
        /// eso es deliberado: el flujo no puede depender de `userinfo`.
        pub async fn servir() -> Self {
            let server = MockServer::start().await;
            let issuer = server.uri();

            // El par de claves es real: la firma que el callback verifica es la
            // misma matemática que en producción. La segunda clave nunca se publica.
            let (clave, publica) = generar_clave();
            let (clave_no_publicada, _) = generar_clave();

            let documento = serde_json::json!({
                "authorization_endpoint": format!("{issuer}/authorize"),
                "token_endpoint": format!("{issuer}{RUTA_TOKEN}"),
                "jwks_uri": format!("{issuer}{RUTA_JWKS}"),
            });

            Mock::given(method("GET"))
                .and(path(RUTA_DESCUBRIMIENTO))
                .respond_with(ResponseTemplate::new(200).set_body_json(documento))
                .mount(&server)
                .await;

            // El catálogo arranca con la clave vigente y el contador a cero: el
            // `jwks_uri` que lo sirve es el mismo de toda la vida del proveedor.
            let publicadas = Arc::new(Mutex::new(vec![jwk_publicada(&clave, KID)]));
            let caido = Arc::new(AtomicBool::new(false));
            let peticiones_al_jwks = Arc::new(AtomicUsize::new(0));

            Mock::given(method("GET"))
                .and(path(RUTA_JWKS))
                .respond_with(lector_del_jwks(
                    Arc::clone(&publicadas),
                    Arc::clone(&caido),
                    Arc::clone(&peticiones_al_jwks),
                ))
                .mount(&server)
                .await;

            Self {
                server,
                issuer,
                clave,
                clave_no_publicada,
                // El secreto `HMAC` es el DER de la clave pública: el mismo
                // material que el `jwks_uri` publica. `EncodingKey::from_secret`
                // no distingue familias, así que el token se firma de verdad.
                clave_confusa: EncodingKey::from_secret(&publica),
                publicadas,
                caido,
                peticiones_al_jwks,
            }
        }

        /// Firma un ID token `RS256` con la clave que el JWKS **sí** publica.
        pub fn firmar(&self, claims: &IdToken) -> String {
            firmar_con(&self.clave, claims)
        }

        /// Firma un ID token `RS256` a partir de claims **crudos**.
        ///
        /// Es lo que permite fabricar tokens que **omiten** un claim obligatorio:
        /// [`IdToken`] declara `exp` e `iat` y no sabe representar su ausencia, y
        /// los candados necesitan un token sin `exp` y otro sin `iat` para probar
        /// que se rechazan.
        pub fn firmar_claims(&self, claims: &serde_json::Value) -> String {
            let mut cabecera = Header::new(Algorithm::RS256);
            cabecera.kid = Some(KID.to_owned());

            encode(&cabecera, claims, &self.clave).expect("el ID token de prueba debe firmarse")
        }

        /// Firma un ID token `RS256` con la clave que el JWKS **no** publica.
        ///
        /// La cabecera lleva el mismo `kid` que el JWK publicado: así la
        /// verificación encuentra la clave y el rechazo sale de que la firma no
        /// cuadra, que es lo que la prueba quiere cazar. Con un `kid` ajeno, el
        /// token se rechazaría por no encontrar la clave y la comprobación
        /// criptográfica nunca se ejercitaría.
        pub fn firmar_con_clave_ajena(&self, claims: &IdToken) -> String {
            firmar_con(&self.clave_no_publicada, claims)
        }

        /// Firma un ID token `RS256` **sin `kid`** en la cabecera.
        ///
        /// La firma es legítima —la hace la clave que el JWKS publica—, pero la
        /// cabecera no dice con cuál, así que no hay clave que elegir. Un «si el
        /// conjunto trae una sola clave, úsala» aceptaría este token.
        pub fn firmar_sin_kid(&self, claims: &serde_json::Value) -> String {
            let cabecera = Header::new(Algorithm::RS256);

            encode(&cabecera, claims, &self.clave).expect("el ID token de prueba debe firmarse")
        }

        /// Firma un ID token `RS256` con la clave publicada pero con el `kid` de
        /// [`KID_AJENO`], que el JWKS no publica.
        ///
        /// La firma cuadra con la única clave del conjunto; lo que falla es la
        /// búsqueda por `kid`. Es el complemento de
        /// [`Self::firmar_con_clave_ajena`]: allí el `kid` resuelve a una clave y
        /// la firma no cuadra; aquí la firma cuadraría y es el `kid` el que no
        /// resuelve.
        pub fn firmar_con_kid_ajeno(&self, claims: &serde_json::Value) -> String {
            let mut cabecera = Header::new(Algorithm::RS256);
            cabecera.kid = Some(KID_AJENO.to_owned());

            encode(&cabecera, claims, &self.clave).expect("el ID token de prueba debe firmarse")
        }

        /// Firma un ID token `HS256` usando como secreto `HMAC` la **clave
        /// pública** del proveedor: la confusión de algoritmo clásica.
        ///
        /// El `kid` es el legítimo, así que la resolución de la clave no falla: lo
        /// único que separa a este token de la sesión es que la verificación fija
        /// `RS256` en vez de leer el algoritmo de la cabecera.
        pub fn firmar_confundiendo_algoritmo(&self, claims: &serde_json::Value) -> String {
            let mut cabecera = Header::new(Algorithm::HS256);
            cabecera.kid = Some(KID.to_owned());

            encode(&cabecera, claims, &self.clave_confusa)
                .expect("el ID token de prueba debe firmarse")
        }

        /// Rota la clave publicada: el `jwks_uri` deja de publicar la vigente y
        /// pasa a publicar una nueva, con [`KID_ROTADO`].
        ///
        /// Devuelve la clave con la que hay que firmar los tokens posteriores, que
        /// los firma [`Self::firmar_con_rotada`]. Es lo que permite reproducir el
        /// caso que el refresco del conjunto existe para cubrir: el `kid` del token
        /// **no** está en el conjunto que el flujo tenía cacheado.
        pub fn rotar_clave(&self) -> ClaveRotada {
            let (clave, _) = generar_clave();

            *self
                .publicadas
                .lock()
                .expect("el catálogo del JWKS de prueba no puede envenenarse") =
                vec![jwk_publicada(&clave, KID_ROTADO)];

            ClaveRotada { clave }
        }

        /// Firma un ID token `RS256` con una clave **rotada**, con su `kid`.
        pub fn firmar_con_rotada(&self, rotada: &ClaveRotada, claims: &IdToken) -> String {
            let mut cabecera = Header::new(Algorithm::RS256);
            cabecera.kid = Some(KID_ROTADO.to_owned());

            encode(&cabecera, claims, &rotada.clave).expect("el ID token de prueba debe firmarse")
        }

        /// Tumba el `jwks_uri`: desde aquí, sus lecturas contestan `500`.
        ///
        /// Es el escenario del proveedor caído **al refrescar**: el conjunto que el
        /// flujo tenía cacheado sigue valiendo, y lo que no llega es el nuevo.
        pub fn tumbar_el_jwks(&self) {
            self.caido.store(true, Ordering::SeqCst);
        }

        /// El conjunto que el `jwks_uri` publica **ahora mismo**.
        ///
        /// Es lo que permite **sembrar** la caché de una prueba con el conjunto
        /// vigente sin gastar una lectura, que es como se fabrica una caché
        /// caducada: se espera diez minutos o se le pone a mano una marca de lectura
        /// vieja, y la segunda es la que cabe en una prueba.
        pub fn jwks_publicado(&self) -> JwkSet {
            JwkSet {
                keys: self
                    .publicadas
                    .lock()
                    .expect("el catálogo del JWKS de prueba no puede envenenarse")
                    .clone(),
            }
        }

        /// Cuántas veces se ha leído el `jwks_uri`.
        ///
        /// Es lo que hace creíble la caché: contar las lecturas es la única forma de
        /// probar que un segundo callback **no** vuelve a pedir el conjunto. El
        /// contador es del doble y no de `wiremock` para que cuente también las
        /// lecturas que el proveedor contesta con un error.
        pub fn peticiones_al_jwks(&self) -> usize {
            self.peticiones_al_jwks.load(Ordering::SeqCst)
        }

        /// Monta el `token_endpoint` para que devuelva el ID token dado.
        ///
        /// Devuelve también un `access_token` distinguible: el flujo no lo usa
        /// —con el ID token verificado sobra— y así se comprueba que no acaba en
        /// la cookie.
        pub async fn montar_token_endpoint(&self, id_token: &str) {
            let cuerpo = serde_json::json!({
                "id_token": id_token,
                "access_token": "access-token-del-proveedor-distinguible",
                "token_type": "Bearer",
                "expires_in": 600,
            });

            Mock::given(method("POST"))
                .and(path(RUTA_TOKEN))
                .respond_with(ResponseTemplate::new(200).set_body_json(cuerpo))
                .mount(&self.server)
                .await;
        }

        /// Cuántas peticiones de descubrimiento han llegado al proveedor.
        ///
        /// Es lo que hace creíble la caché: contar las peticiones es la única
        /// forma de probar que un segundo login **no** vuelve a pedir el documento.
        pub async fn peticiones_de_descubrimiento(&self) -> usize {
            self.peticiones_a(RUTA_DESCUBRIMIENTO).await
        }

        /// Cuántas veces se ha llamado al `token_endpoint`.
        pub async fn peticiones_al_token(&self) -> usize {
            self.peticiones_a(RUTA_TOKEN).await
        }

        /// Cuenta las peticiones recibidas cuya ruta coincide con `ruta`.
        async fn peticiones_a(&self, ruta: &str) -> usize {
            self.server
                .received_requests()
                .await
                .unwrap_or_default()
                .iter()
                .filter(|peticion| peticion.url.path() == ruta)
                .count()
        }
    }

    /// Genera un par RSA real y devuelve la clave privada como clave de firma.
    ///
    /// A 2048 bits, que es el mínimo que la verificación `RS256` de `jsonwebtoken`
    /// espera. La clave se exporta en PKCS#1 DER porque es lo que acepta
    /// `EncodingKey::from_rsa_der` y lo que `Jwk::from_encoding_key` sabe leer.
    /// Devuelve también la **clave pública** en PKCS#1 DER.
    fn generar_clave() -> (EncodingKey, Vec<u8>) {
        let mut rng = rand::thread_rng();

        let privada = RsaPrivateKey::new(&mut rng, 2048)
            .expect("la generación de un par RSA de prueba no puede fallar");
        let der = privada
            .to_pkcs1_der()
            .expect("la clave privada de prueba debe codificarse en PKCS#1")
            .as_bytes()
            .to_vec();
        let publica = privada
            .to_public_key()
            .to_pkcs1_der()
            .expect("la clave pública de prueba debe codificarse en PKCS#1")
            .as_bytes()
            .to_vec();

        (EncodingKey::from_rsa_der(&der), publica)
    }

    /// Publica la clave pública de `clave` como JWK, con el `kid` dado.
    ///
    /// Se usa `Jwk::from_encoding_key`, que extrae el módulo y el exponente y los
    /// codifica en base64url sin relleno —como exige el RFC 7518—, de modo que la
    /// misma biblioteca que firma y verifica construye la clave que publica.
    fn jwk_publicada(clave: &EncodingKey, kid: &str) -> Jwk {
        let mut jwk = Jwk::from_encoding_key(clave, Algorithm::RS256)
            .expect("la clave pública de prueba debe poder publicarse como JWK");
        jwk.common.key_id = Some(kid.to_owned());

        jwk
    }

    /// El lector del `jwks_uri`: cuenta la lectura y publica lo que toque.
    ///
    /// Es la única pieza que decide qué contesta el `jwks_uri`, y por eso es donde
    /// vive el contador: una lectura cuenta siempre —también la que termina en
    /// error—, porque lo que las pruebas de la caché miden es cuántas veces se ha
    /// ido a por el conjunto.
    fn lector_del_jwks(
        publicadas: Arc<Mutex<Vec<Jwk>>>,
        caido: Arc<AtomicBool>,
        peticiones: Arc<AtomicUsize>,
    ) -> impl Fn(&Request) -> ResponseTemplate + Send + Sync + 'static {
        move |_peticion| {
            peticiones.fetch_add(1, Ordering::SeqCst);

            if caido.load(Ordering::SeqCst) {
                return ResponseTemplate::new(500);
            }

            let conjunto = JwkSet {
                keys: publicadas
                    .lock()
                    .expect("el catálogo del JWKS de prueba no puede envenenarse")
                    .clone(),
            };

            ResponseTemplate::new(200)
                .set_body_json(serde_json::to_value(conjunto).expect("el JWKS debe serializarse"))
        }
    }

    /// Firma `claims` con `clave` y el `kid` del proveedor.
    fn firmar_con(clave: &EncodingKey, claims: &IdToken) -> String {
        let mut cabecera = Header::new(Algorithm::RS256);
        cabecera.kid = Some(KID.to_owned());

        encode(&cabecera, claims, clave).expect("el ID token de prueba debe firmarse")
    }

    /// JWT con la cabecera `{"alg":"none","kid":…}` y **sin firma**.
    ///
    /// Es la confusión de algoritmo más burda: el token declara que no va firmado
    /// y los claims son los legítimos. No se construye con `jsonwebtoken` porque
    /// su `Algorithm` no tiene la variante `none` —es justo la razón de que este
    /// token se rechace al leer la cabecera—, así que las tres partes se codifican
    /// a mano en base64url sin relleno, como manda el RFC 7515. La tercera va
    /// vacía: eso es exactamente lo que significa `alg: none`.
    pub fn token_con_alg_none(claims: &serde_json::Value) -> String {
        use base64::Engine as _;

        let cabecera = serde_json::json!({ "alg": "none", "kid": KID });

        let codificar = |valor: &serde_json::Value| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(valor.to_string())
        };

        format!("{}.{}.", codificar(&cabecera), codificar(claims))
    }
}

// ── Captura del registro ─────────────────────────────────────────────────────
//
// Los candados de las comprobaciones del ID token no se conforman con el `401`:
// un token rechazado por la firma, por el `kid`, por `iss`, por `aud` o por el
// algoritmo da **el mismo** `401`, así que sin mirar la causa registrada el
// candado no fija de qué comprobación viene el rechazo que anuncia. Eso obliga a
// leer lo que se registra, y de ahí este arnés.
//
// El arnés instala **un suscriptor global**, una sola vez por proceso, y elige a
// qué buffer escribe cada evento por **hilo**: un `thread_local` guarda el buffer
// del test que está capturando en ese hilo, y el evento que sale de un hilo sin
// captura se descarta en silencio. Así la suite sigue corriendo en paralelo, sin
// un `Mutex` global compartido y sin serializar tests.
//
// **No reintroducir `set_default` ni `with_default` en este arnés**: con un
// suscriptor **de hilo**, reconstruir el interés de un callsite puede preguntarle
// a un hilo que no tiene ninguno y dejarlo cacheado como `never` para **todo el
// proceso** —`tracing-core` lo hace así cuando solo hay un despachante vivo—, y
// desde entonces ese registro desaparece, en cualquier hilo, aunque el suscriptor
// esté instalado. El síntoma es «un registro que falta», no un test rojo: el test
// cae por la aserción que comprueba que la captura está en pie. Si vuelve a
// fallar así, se arregla aquí, no aflojando la aserción.

thread_local! {
    /// Buffer de captura del hilo actual, si ese hilo está capturando.
    static BUFFER_DEL_HILO: RefCell<Option<BufferDeLog>> = const { RefCell::new(None) };
}

/// Marca de que el suscriptor global del arnés ya está instalado.
///
/// Un `OnceLock` y no un `bool`: la instalación tiene que ser **idempotente** y
/// segura ante dos tests que la pidan a la vez, porque `set_global_default` solo
/// se puede llamar una vez por proceso.
static SUSCRIPTOR_GLOBAL: OnceLock<()> = OnceLock::new();

/// Buffer de registro en memoria, compartido por todas las copias que se clonen.
///
/// Es el destino al que el suscriptor de [`capturar_log`] escribe: lo que se
/// registre en el hilo pasa a ser texto que la prueba puede inspeccionar, en vez
/// de salir por la salida estándar.
#[derive(Clone, Default)]
pub struct BufferDeLog(Arc<Mutex<Vec<u8>>>);

impl BufferDeLog {
    /// Texto acumulado hasta este instante.
    pub fn texto(&self) -> String {
        let bytes = self
            .0
            .lock()
            .expect("el buffer de log no puede envenenarse");

        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl std::io::Write for BufferDeLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("el buffer de log no puede envenenarse")
            .extend_from_slice(buf);

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufferDeLog {
    type Writer = BufferDeLog;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Escritor del suscriptor global: escribe en el buffer del hilo, si lo hay.
///
/// Fuera del alcance de una captura no hay buffer y el registro se descarta **en
/// silencio**, como un `/dev/null`: así un hilo que no captura no contamina a
/// nadie.
struct EscritorDeCaptura(Option<BufferDeLog>);

impl std::io::Write for EscritorDeCaptura {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match &mut self.0 {
            Some(buffer) => buffer.write(buf),
            None => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// `MakeWriter` del suscriptor global: elige el buffer **por hilo**.
struct CapturaGlobal;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturaGlobal {
    type Writer = EscritorDeCaptura;

    fn make_writer(&'a self) -> Self::Writer {
        EscritorDeCaptura(BUFFER_DEL_HILO.with(|celda| celda.borrow().clone()))
    }
}

/// Instala el suscriptor global de las pruebas, una sola vez por proceso.
///
/// El filtro es por **nivel**, no por `target` ni por `EnvFilter`: `LevelFilter`
/// contesta `Interest::always` a todo lo que pasa el nivel y no cachea interés por
/// callsite, así que le quita una capa de caché al fallo que este arnés existe
/// para no tener. El nivel es `TRACE` a propósito: un candado de «esto no se
/// registra» solo vale si el registro estaba en su nivel más hablador cuando se
/// comprobó.
///
/// **No cambiar esto por `init()` ni `try_init()`**: ésos instalan además el
/// puente `log` → `tracing`, y `wiremock` instrumenta con `log` —con el cuerpo de
/// la petición, y el del canje lleva el `client_secret`—, así que el puente
/// metería credenciales en el buffer de captura.
fn instalar_suscriptor_global() {
    SUSCRIPTOR_GLOBAL.get_or_init(|| {
        let suscriptor = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .without_time()
            .with_writer(CapturaGlobal)
            .finish();

        if let Err(error) = tracing::subscriber::set_global_default(suscriptor) {
            panic!(
                "el arnés de captura no pudo instalar su suscriptor global: {error}. \
                 Este arnés es el único que instala un suscriptor en el binario de \
                 pruebas: quita el que lo pisa —el único del crate está en \
                 `src/main.rs:178`, y usa `try_init()`—, o la captura de registro \
                 deja de ser fiable."
            );
        }

        // Aquí hace falta, y no es ceremonia: `set_global_default` ya reconstruye
        // el interés de lo **ya registrado**, pero lo hace **antes** de marcar el
        // global como inicializado. En esa ventana, un callsite estrenado por un
        // test que no captura puede quedar cacheado como `never`. Esta llamada, ya
        // con el global puesto, repara esos callsites.
        tracing::callsite::rebuild_interest_cache();
    });
}

/// Guardián de una captura: al soltarse, devuelve al hilo el buffer que tenía.
///
/// Guarda el valor **anterior** del `thread_local`, no un `None` a secas: si una
/// captura se anidara dentro de otra, al cerrar la de dentro hay que volver a la
/// de fuera. Mantenerlo vivo mientras se leen los eventos es obligatorio: al
/// soltarlo, el hilo deja de capturar.
/// No la muevas de hilo: su `Drop` escribe en el `thread_local` del hilo que la
/// suelta.
#[must_use = "soltar la guardia deja de capturar: manténla viva mientras leas el buffer"]
pub struct GuardiaDeCaptura(Option<BufferDeLog>);

impl Drop for GuardiaDeCaptura {
    fn drop(&mut self) {
        BUFFER_DEL_HILO.with(|celda| *celda.borrow_mut() = self.0.take());
    }
}

/// Captura el registro del hilo actual en memoria y devuelve el buffer.
///
/// El suscriptor es **global** y se instala una sola vez; lo que cada llamada fija
/// es el **buffer del hilo**. Se usa así:
///
/// ```ignore
/// let (buffer, _guardia) = capturar_log();
/// // …lo que registre…
/// assert!(buffer.texto().contains("…"));
/// ```
pub fn capturar_log() -> (BufferDeLog, GuardiaDeCaptura) {
    instalar_suscriptor_global();

    let buffer = BufferDeLog::default();
    let anterior = BUFFER_DEL_HILO.with(|celda| celda.borrow_mut().replace(buffer.clone()));

    (buffer, GuardiaDeCaptura(anterior))
}
