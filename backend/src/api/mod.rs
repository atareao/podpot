//! El servidor HTTP de podpot.
//!
//! Todo lo que cuelga de `/api/v1/` responde con la misma envoltura: los datos
//! dentro de `data` y los fallos dentro de `error`.
//!
//! Fuera de `/api/v1/` está el panel: si la configuración apunta a un build
//! estático, se sirve desde ahí y **cualquier** ruta que no sea de la API cae al
//! `index.html` —la reserva que necesita una SPA, que resuelve sus rutas en el
//! cliente—. Sin panel, lo que no es de la API es un `404` con la envoltura.

pub mod auth;
pub mod health;
pub mod jobs;
pub mod logs;
pub mod quadlets;
pub mod response;
pub mod stream;
pub mod units;

use std::path::Path;

use axum::Router;
use axum::routing::{get, post};
use tower_http::services::{ServeDir, ServeFile};

use crate::auth::middleware;
use crate::error::AppError;
use crate::state::AppState;

/// El prefijo bajo el que vive toda la API.
///
/// Es lo que separa las dos mitades del router: por debajo, envoltura JSON; por
/// encima, el panel. Está aquí y no repetido en cada ruta porque es una sola
/// decisión.
const API_PREFIX: &str = "/api/v1";

/// El fichero al que se reserva una ruta de la SPA que no es un fichero.
const PANEL_INDEX: &str = "index.html";

/// Compone el router de la aplicación con su estado.
///
/// La superficie de `/api/v1/` se parte en dos mitades por **lista blanca**: una
/// pública —la salud y, en modo `Oidc`, las dos rutas del flujo de entrada— y
/// una de negocio con el resto, a la que el *gate* de sesión se cuelga con
/// `route_layer` para que no alcance a las públicas. La lista blanca y no una
/// lista negra: así una ruta nueva queda del lado protegido por construcción, sin
/// que nadie tenga que acordarse de colgarle el *gate*.
///
/// Las dos trampas que el router cierra a mano son las de la envoltura única: una
/// ruta que no existe y un método que no está declarado contestan por defecto con
/// un cuerpo vacío, y el contrato exige la envoltura **también ahí**, así que las
/// dos se sustituyen por manejadores propios —y por eso la API va anidada: así su
/// `404` no se confunde con la reserva del panel—.
///
/// La acción de una unidad va en la ruta —`…/{name}/{action}`— porque es lo que
/// la identifica: `start` sobre una unidad y `stop` sobre la misma unidad son dos
/// recursos distintos, y con la acción en el cuerpo cada petición tendría que
/// llevar uno para decir qué quiere.
pub fn router(state: AppState) -> Router {
    // La salud es lo único público en cualquier modo.
    let public = Router::new().route("/health", get(health::health));

    // La superficie de negocio: las unidades, sus acciones, la creación y
    // edición de quadlets y el canal en vivo. Cuelga de aquí, y no de la mitad
    // pública, por construcción: el *gate* la alcanza sin que nadie tenga que
    // acordarse.
    let negocio = Router::new()
        .route("/units", get(units::listar))
        // La ruta del canal en vivo va **antes** que la de la acción —dos
        // segmentos frente a tres— para que `stream` no se confunda con un nombre
        // de unidad: el router resuelve por forma, no por orden, pero leerlo así
        // deja claro que son dos recursos distintos. El SSE queda protegido por el
        // mismo *gate* que el resto: no hay ninguna excepción en el router.
        .route("/units/stream", get(stream::stream))
        // El canal de progreso de los jobs: dos segmentos, como el de unidades, y
        // por eso no se confunde con ninguna acción. Es su propio recurso —los jobs
        // de systemd, no las unidades— y va bajo el mismo *gate* que el resto.
        .route("/jobs/stream", get(jobs::stream))
        // El canal de logs de una unidad: cuatro segmentos, así que no se confunde
        // con la acción —tres—. Cuelga de la mitad de negocio y por eso el *gate*
        // lo cubre sin ninguna excepción, igual que el canal en vivo.
        .route("/units/{name}/logs/stream", get(logs::stream))
        .route("/units/{name}/{action}", post(units::accionar))
        .route("/quadlets", post(quadlets::crear))
        .route(
            "/quadlets/{name}",
            get(quadlets::leer)
                .patch(quadlets::editar)
                .delete(quadlets::borrar),
        );

    // Con proveedor, el flujo de entrada es público y las rutas de sesión, de
    // negocio; sin él, ni unas ni otras se declaran —contestan el `404` de la
    // casa— y no se monta ninguna capa.
    let (public, negocio) = match &state.auth {
        Some(_) => (
            public
                .route("/auth/login", get(auth::login))
                .route("/auth/callback", get(auth::callback)),
            negocio
                .route("/auth/logout", post(auth::logout))
                .route("/auth/session", get(auth::session)),
        ),
        None => (public, negocio),
    };

    // El *gate* se cuelga **sólo** de la mitad de negocio. `route_layer` y no
    // `layer`: así no alcanza a los *fallbacks* de ruta —un `404` de una ruta
    // inexistente no puede volverse un `401`—, que es justo lo que la API
    // necesita.
    let negocio = match &state.auth {
        Some(_) => negocio.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::gate,
        )),
        None => negocio,
    };

    // El orden de estas dos líneas del `method_not_allowed_fallback` respecto al
    // `route_layer` de arriba es **semántico**, no cosmético: `method_not_allowed_fallback`
    // sustituye el *fallback* por defecto del `MethodRouter`, y sólo lo sustituye si
    // sigue siendo el de por defecto.
    //
    // Declarado aquí —*después* de colgar la capa—, sustituye un *fallback* que ya
    // estaba envuelto por otro que nunca vio el *gate* —el envoltorio conserva la
    // variante «por defecto», y por eso la sustitución posterior lo pisa—: un método
    // no declarado sobre una ruta de negocio contesta `405` **sin** haber exigido
    // sesión. Es lo que pide el diseño de este change —«un método no permitido da
    // `405`, no `401`»— y lo que preserva el contrato de la casa: la partición no
    // cambia el comportamiento del `405`.
    //
    // Declarado *antes* —sobre `negocio`, antes del `route_layer`— la capa lo
    // envolvería y el *gate* decidiría antes que el enrutado por método, con lo que
    // un método no declarado contestaría `401`. Es la elección de kesto, que prefiere
    // no revelar a quien no ha entrado qué métodos existen; aquí se descarta a
    // propósito porque el contrato de `api-shell` fija el `405` con su envoltura.
    //
    // Medido, no supuesto: invertir las dos líneas pone en rojo
    // `el_405_se_preserva_sin_sesion` —`left: 401, right: 405`—, que es el candado
    // de esta decisión.
    let api = public
        .merge(negocio)
        .fallback(ruta_inexistente)
        .method_not_allowed_fallback(metodo_no_permitido);

    let app = Router::new().nest(API_PREFIX, api);

    // El fallback del router es lo que atiende todo lo que no es de la API: con
    // panel, sus ficheros (y el `index.html` para lo que no sea fichero); sin
    // panel, el `404` de la casa.
    match state.config.static_dir.as_deref() {
        Some(dir) => app.fallback_service(panel(dir)),
        None => app.fallback(ruta_inexistente),
    }
    .with_state(state)
}

/// El servicio del panel: el build estático, con reserva a `index.html`.
///
/// `ServeDir` sirve el fichero que corresponde a la ruta —y el `index.html` del
/// directorio cuando la ruta es un directorio—; lo que **no** encuentra lo
/// contesta su servicio de reserva, que es el `index.html` de la raíz. Ésa es la
/// reserva SPA: una ruta como `/unidades/1` no es un fichero, y el panel la
/// resuelve en el cliente.
///
/// Se usa `fallback` y no `not_found_service` porque la reserva SPA es una
/// **respuesta buena** (el `index.html` con su `200`), no un `404` con cuerpo de
/// otro: `not_found_service` forzaría el `404` sobre la reserva y el navegador
/// trataría de cargar el `index.html` como si faltase.
fn panel(dir: &Path) -> ServeDir<ServeFile> {
    ServeDir::new(dir).fallback(ServeFile::new(dir.join(PANEL_INDEX)))
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
    use axum::http::{Method, StatusCode, header};

    use crate::test_support::proveedor_falso::ProveedorFalso;
    use crate::test_support::{
        body_json, cookie_de_sesion, entorno, entorno_con_panel, entorno_oidc,
        entorno_oidc_con_emisor, header_texto, peticion, peticion_con_cookie, peticion_cruda,
        peticion_cruda_con_cookie, peticion_texto,
    };

    /// Un directorio de panel con un `index.html` reconocible, para las pruebas
    /// que necesitan uno. Se devuelve el `TempDir` para que viva hasta el final:
    /// soltarlo borraría el directorio que se está sirviendo.
    fn panel_de_prueba() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");

        std::fs::write(
            dir.path().join("index.html"),
            "<!doctype html><title>podpot</title>",
        )
        .expect("el index del panel de prueba se tiene que poder escribir");

        dir
    }

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

    /// Escenario «Panel en la raíz»: con panel configurado, `/` sirve el
    /// `index.html` del build.
    #[tokio::test]
    async fn la_raiz_sirve_el_index_del_panel() {
        let panel = panel_de_prueba();
        let entorno = entorno_con_panel(panel.path()).await;

        let (status, cuerpo) = peticion_texto(&entorno.app(), Method::GET, "/").await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            cuerpo.contains("podpot"),
            "la raíz tiene que servir el index del panel: {cuerpo}"
        );
    }

    /// Escenario «Ruta de la SPA»: una ruta que no es un fichero del build cae al
    /// `index.html`, que es lo que necesita una SPA que resuelve en el cliente.
    #[tokio::test]
    async fn una_ruta_de_la_spa_cae_al_index() {
        let panel = panel_de_prueba();
        let entorno = entorno_con_panel(panel.path()).await;

        let (status, cuerpo) = peticion_texto(&entorno.app(), Method::GET, "/unidades/1").await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            cuerpo.contains("podpot"),
            "una ruta desconocida tiene que caer al index del panel: {cuerpo}"
        );
    }

    /// El panel no se come a la API: con panel configurado, un recurso de la API
    /// que no existe sigue dando el `404` de la casa con su envoltura.
    #[tokio::test]
    async fn el_panel_no_se_come_el_404_de_la_api() {
        let panel = panel_de_prueba();
        let entorno = entorno_con_panel(panel.path()).await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/no-existe").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(cuerpo["error"]["code"], "not_found");
    }

    /// Y tampoco su `405`: el método no declarado de una ruta de la API sigue
    /// saliendo con la envoltura, no con el `index.html`.
    #[tokio::test]
    async fn el_panel_no_se_come_el_405_de_la_api() {
        let panel = panel_de_prueba();
        let entorno = entorno_con_panel(panel.path()).await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::POST, "/api/v1/health").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// Escenario «Sin panel»: sin `PODPOT_STATIC_DIR` la aplicación arranca sólo
    /// como API, y lo que no es de la API es el `404` de la casa.
    #[tokio::test]
    async fn sin_panel_lo_que_no_es_de_la_api_da_404() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(cuerpo["error"]["code"], "not_found");
    }

    // --- La partición de la superficie: el *gate* de sesión -------------------

    /// Escenario «Petición sin sesión»: en modo `oidc`, una ruta de negocio sin
    /// cookie contesta `401` con la envoltura, **sin** redirigir y sin HTML.
    #[tokio::test]
    async fn sin_sesion_una_ruta_de_negocio_da_401_envuelto() {
        let entorno = entorno_oidc().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/units").await;

        assert_eq!(respuesta.status(), StatusCode::UNAUTHORIZED);
        assert!(
            header_texto(&respuesta, header::LOCATION).is_none(),
            "el rechazo no puede ser una redirección: es una API"
        );
        assert!(
            header_texto(&respuesta, header::CONTENT_TYPE)
                .is_some_and(|tipo| tipo.starts_with("application/json")),
            "el rechazo tiene que ser JSON, no HTML"
        );

        let cuerpo = body_json(respuesta).await;
        assert_eq!(cuerpo["error"]["code"], "unauthorized");
        assert!(
            cuerpo["error"]["message"].as_str().is_some(),
            "el 401 tiene que explicar el motivo: {cuerpo}"
        );
    }

    /// Escenario «Rutas públicas»: la salud, el login y el callback se atienden
    /// sin exigir sesión.
    #[tokio::test]
    async fn las_rutas_publicas_se_atienden_sin_sesion() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let (salud, _) = peticion(&entorno.app(), Method::GET, "/api/v1/health").await;
        assert_eq!(salud, StatusCode::OK, "la salud es pública");

        let login = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/auth/login").await;
        assert_eq!(
            login.status(),
            StatusCode::FOUND,
            "el login redirige al proveedor sin exigir sesión"
        );

        let (callback, _) = peticion(&entorno.app(), Method::GET, "/api/v1/auth/callback").await;
        assert_eq!(
            callback,
            StatusCode::BAD_REQUEST,
            "el callback sin parámetros se atiende —y se queja del flujo—, no del gate"
        );
    }

    /// Escenario «Con sesión»: con cookie válida, la superficie de negocio se
    /// atiende con normalidad.
    #[tokio::test]
    async fn con_sesion_la_superficie_de_negocio_se_atiende() {
        let entorno = entorno_oidc().await;

        let (status, cuerpo) = peticion_con_cookie(
            &entorno.app(),
            Method::GET,
            "/api/v1/units",
            &cookie_de_sesion(),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            cuerpo["data"].is_array(),
            "el listado de unidades va dentro de data: {cuerpo}"
        );
    }

    /// El `405` de un método no declarado se **preserva**: la partición no lo
    /// convierte en `401`. Es la decisión de este repositorio —el contrato de la
    /// casa manda `405` con la envoltura— y diverge a propósito de kesto, donde
    /// el `401` sale antes que el enrutado por método.
    #[tokio::test]
    async fn el_405_se_preserva_sin_sesion() {
        let entorno = entorno_oidc().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::POST, "/api/v1/units").await;

        assert_eq!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "un método no declarado no puede volverse un 401"
        );
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// Y una ruta inexistente sigue siendo `404`, no `401`: `route_layer` no
    /// envuelve el *fallback* que resuelve las rutas no declaradas.
    #[tokio::test]
    async fn el_404_de_una_ruta_inexistente_no_se_vuelve_401() {
        let entorno = entorno_oidc().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/no-existe").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(cuerpo["error"]["code"], "not_found");
    }

    /// Escenario «Modo deshabilitado no protege»: sin proveedor no se monta el
    /// *gate* y la superficie sigue accesible sin sesión.
    #[tokio::test]
    async fn en_disabled_la_superficie_sigue_accesible() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/units").await;

        assert_eq!(status, StatusCode::OK);
        assert!(cuerpo["data"].is_array());
    }

    /// Y en `disabled` las rutas de sesión **no se declaran**: contestan el `404`
    /// de la casa, que es la forma de decir que ahí no hay autenticación.
    #[tokio::test]
    async fn en_disabled_las_rutas_de_sesion_no_existen() {
        let entorno = entorno().await;

        for ruta in ["/api/v1/auth/login", "/api/v1/auth/callback"] {
            let (status, cuerpo) = peticion(&entorno.app(), Method::GET, ruta).await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{ruta}");
            assert_eq!(cuerpo["error"]["code"], "not_found");
        }

        let (status, _) = peticion(&entorno.app(), Method::POST, "/api/v1/auth/logout").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    // --- La salud anuncia el modo -------------------------------------------

    /// Escenario «Modo OIDC anunciado»: la salud publica `auth_mode`.
    #[tokio::test]
    async fn la_salud_publica_el_modo_oidc() {
        let entorno = entorno_oidc().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/health").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cuerpo["data"]["auth_mode"], "oidc");
    }

    /// Escenario «Modo deshabilitado por defecto»: sin variables, la salud lo
    /// dice.
    #[tokio::test]
    async fn la_salud_publica_el_modo_deshabilitado() {
        let entorno = entorno().await;

        let (_, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/health").await;

        assert_eq!(cuerpo["data"]["auth_mode"], "disabled");
    }

    /// La salud trae la versión del paquete: el panel la muestra y el contrato de
    /// la interfaz la espera.
    #[tokio::test]
    async fn la_salud_publica_la_version() {
        let entorno = entorno().await;

        let (_, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/health").await;

        assert_eq!(cuerpo["data"]["version"], env!("CARGO_PKG_VERSION"));
    }

    // --- El canal en vivo, bajo el mismo *gate* ------------------------------

    /// Escenario «Stream sin sesión»: `401` con la envoltura y **sin** abrir el
    /// flujo de eventos.
    #[tokio::test]
    async fn el_stream_sin_sesion_da_401_y_no_abre_el_flujo() {
        let entorno = entorno_oidc().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/units/stream").await;

        assert_eq!(respuesta.status(), StatusCode::UNAUTHORIZED);

        let tipo = header_texto(&respuesta, header::CONTENT_TYPE).unwrap_or_default();
        assert!(
            !tipo.starts_with("text/event-stream"),
            "sin sesión no puede abrirse el flujo: {tipo}"
        );

        let cuerpo = body_json(respuesta).await;
        assert_eq!(cuerpo["error"]["code"], "unauthorized");
    }

    /// Escenario «Stream con sesión»: con cookie válida el flujo se abre.
    #[tokio::test]
    async fn el_stream_con_sesion_abre_el_flujo() {
        let entorno = entorno_oidc().await;

        let respuesta = peticion_cruda_con_cookie(
            &entorno.app(),
            Method::GET,
            "/api/v1/units/stream",
            &cookie_de_sesion(),
        )
        .await;

        assert_eq!(respuesta.status(), StatusCode::OK);
        assert!(
            header_texto(&respuesta, header::CONTENT_TYPE)
                .is_some_and(|tipo| tipo.starts_with("text/event-stream")),
            "con sesión el flujo debe abrirse"
        );
    }

    /// Y en `disabled` el flujo sigue abierto sin sesión: no hay capa que lo
    /// cierre.
    #[tokio::test]
    async fn en_disabled_el_stream_sigue_abierto_sin_sesion() {
        let entorno = entorno().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/units/stream").await;

        assert_eq!(respuesta.status(), StatusCode::OK);
        assert!(
            header_texto(&respuesta, header::CONTENT_TYPE)
                .is_some_and(|tipo| tipo.starts_with("text/event-stream"))
        );
    }
}
