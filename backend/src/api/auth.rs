//! Las rutas del flujo de entrada: `/api/v1/auth/*`.
//!
//! Aquí sólo hay HTTP: la verificación de la sesión vive en `src/auth/`, y el
//! flujo OIDC también —el descubrimiento, la URL de autorización, el canje del
//! código y la verificación del ID token, todo en [`crate::auth::oidc`], y la
//! sesión en [`crate::auth::session`]—. Este módulo traduce la petición en
//! llamadas a ese módulo y compone la respuesta: un `302` en los dos extremos del
//! flujo —`login` hacia el proveedor, `callback` de vuelta al panel— y la cookie
//! de sesión en el callback.

use axum::body::Body;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Extension, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::{Deserialize, Serialize};

use crate::api::response::ApiResponse;
use crate::auth::middleware::SessionIdentity;
use crate::auth::{oidc, session};
use crate::error::AppError;
use crate::state::AppState;

pub use session::SESSION_COOKIE_NAME;

/// Mensaje único con el que se rechaza un intento de entrada que no cuadra.
///
/// Los tres modos de fallo del `state` —desconocido, caducado y reutilizado— se
/// distinguen en el código y en el registro, pero comparten este mensaje: quien
/// no ha entrado no aprende cuál de los tres es el suyo.
const INTENTO_INVALIDO: &str = "el intento de entrada no es válido";

/// Mensaje con el que se rechaza un callback al que le faltan los parámetros del
/// flujo.
const CALLBACK_INCOMPLETO: &str = "el callback no trae los parámetros del flujo";

/// Destino de la vuelta tras el callback: la raíz del panel.
///
/// La raíz es lo natural —es donde se sirve el `index.html` de la SPA— y no
/// arrastra ningún parámetro del flujo: la `Location` no puede llevar el código
/// de autorización ni el `state`.
const DESTINO_PANEL: &str = "/";

/// `GET /api/v1/auth/login` — pública.
///
/// Redirige al `authorization_endpoint` del proveedor con los seis parámetros
/// del flujo, guarda el `state`/`nonce` emitidos y **no** emite sesión: la cookie
/// llega con el callback.
pub async fn login(State(state): State<AppState>) -> Response {
    // La ruta sólo se monta en modo `Oidc` (el router), así que este camino es
    // inalcanzable por construcción. No se confía en ello: sin estado de
    // autenticación se contesta el `5xx` genérico en vez de un `panic`.
    let Some(auth) = state.auth.as_ref() else {
        return AppError::Internal("el login se atendió sin estado de autenticación".to_owned())
            .into_response();
    };

    // El descubrimiento cacheado o su reintento. Si falla, el error ya es un
    // `BadGateway`, que traduce a `502` con la envoltura y el mensaje genérico.
    let discovery = match oidc::discovery_cached(auth).await {
        Ok(discovery) => discovery,
        Err(error) => return error.into_response(),
    };

    let solicitud = oidc::build_authorization_url(auth.settings.as_ref(), &discovery);

    // El `state` se guarda con su marca de tiempo y junto al `nonce` emitido: el
    // callback comparará ese `nonce` con el del ID token al verificar. La
    // respuesta no emite cookie: la sesión llega con el callback.
    auth.registrar_login(&solicitud.state, &solicitud.nonce)
        .await;

    redireccion_302(&solicitud.url)
}

/// Respuesta `302` con la cabecera `Location`.
///
/// Se construye a mano porque `axum::response::Redirect` no ofrece `302` —`to` es
/// `303`, y el flujo de entrada exige `302` literal—. La respuesta lleva solo
/// `Location`, sin `Set-Cookie`: el login no establece sesión.
fn redireccion_302(destino: &str) -> Response {
    match HeaderValue::from_str(destino) {
        Ok(location) => {
            let mut respuesta = Response::new(Body::empty());
            *respuesta.status_mut() = StatusCode::FOUND;
            respuesta.headers_mut().insert(header::LOCATION, location);

            respuesta
        }
        // Una URL que no cabe en una cabecera no es un destino a medias: es un
        // fallo al construirla.
        Err(error) => AppError::Internal(format!(
            "la URL de autorización no es una cabecera válida: {error}"
        ))
        .into_response(),
    }
}

/// `GET /api/v1/auth/callback` — pública.
///
/// Retorno del proveedor: consume el `state`, canjea el código, verifica el ID
/// token y, si todo cuadra, emite la cookie de sesión y vuelve al panel. Todo
/// rechazo contesta la envoltura y **no** emite cookie.
///
/// La consulta se extrae **tipada** —`Result<Query<_>, QueryRejection>` y no
/// `Query<_>` a secas— para poder traducir el rechazo del extractor: servir el
/// que trae `Query` por defecto rompe las dos reglas de la envoltura. Sale con
/// `text/plain` —no con el JSON de la casa— y con el detalle de serde, que
/// publica la estructura interna del struct («duplicate field `code`»). Y no es
/// un borde teórico: un `code` duplicado en la URL lo provoca cualquiera.
pub async fn callback(
    State(state): State<AppState>,
    params: Result<Query<CallbackParams>, QueryRejection>,
) -> Response {
    // La ruta sólo se monta en modo `Oidc` (el router), así que este camino es
    // inalcanzable por construcción. No se confía en ello.
    let Some(auth) = state.auth.as_ref() else {
        return AppError::Internal("el callback se atendió sin estado de autenticación".to_owned())
            .into_response();
    };

    // Una consulta que el extractor no sabe leer es un callback mal formado, y se
    // contesta como tal: `400` con la envoltura y el mensaje genérico del flujo.
    // El detalle del rechazo —el de serde— no se traduce ni al cuerpo ni al
    // registro: nombra campos internos del struct y al cliente no le sirve.
    let Query(params) = match params {
        Ok(params) => params,
        Err(_) => {
            tracing::debug!("la consulta del callback no pasa el extractor");
            return AppError::BadRequest(CALLBACK_INCOMPLETO.to_owned()).into_response();
        }
    };

    // Un `error` en la consulta es el proveedor diciendo que no: se rechaza sin
    // canjear el código —el `token_endpoint` no se toca—.
    if params.error.is_some() {
        return AppError::Unauthorized(INTENTO_INVALIDO.to_owned()).into_response();
    }

    let Some(state_recibido) = params.state.as_deref() else {
        return AppError::BadRequest(CALLBACK_INCOMPLETO.to_owned()).into_response();
    };
    let Some(code) = params.code.as_deref() else {
        return AppError::BadRequest(CALLBACK_INCOMPLETO.to_owned()).into_response();
    };

    // El `state` se consume: extraerlo del mapa y descartarlo. Un `state`
    // desconocido, caducado o reutilizado se distingue en el código —y en el
    // registro— pero contesta el mismo mensaje al cliente.
    let pendiente = match auth.consumir_login(state_recibido).await {
        Ok(pendiente) => pendiente,
        Err(rechazo) => {
            tracing::debug!(?rechazo, "callback con un state no utilizable");
            return AppError::Unauthorized(INTENTO_INVALIDO.to_owned()).into_response();
        }
    };

    let discovery = match oidc::discovery_cached(auth).await {
        Ok(discovery) => discovery,
        Err(error) => return error.into_response(),
    };

    let canje = match oidc::canjear_codigo(auth.settings.as_ref(), &discovery, code).await {
        Ok(canje) => canje,
        Err(error) => return error.into_response(),
    };

    let identidad = match oidc::verificar_id_token(auth, &canje.id_token, &pendiente.nonce).await {
        Ok(identidad) => identidad,
        Err(error) => return error.into_response(),
    };

    // Toda sesión emitida lleva `jti`: es el identificador con el que se ata el
    // `refresh_token` del proveedor —si lo emite— a la sesión. Sin `refresh_token`
    // el `jti` viaja igualmente, así que toda sesión tiene con qué identificarse y
    // la renovación no depende de que exista el refresco.
    let jti = session::nuevo_jti();

    if let Some(refresh_token) = canje.refresh_token.as_deref() {
        auth.guardar_refresh_token(&jti, refresh_token).await;
    }

    let token =
        match session::issue_con_jti(&auth.secret, identidad.sub, identidad.email, Some(jti)) {
            Ok(token) => token,
            Err(error) => return error.into_response(),
        };

    let cookie = match session::session_cookie(token, &auth.settings.redirect_url) {
        Ok(cookie) => cookie,
        Err(error) => return error.into_response(),
    };

    redireccion_302_al_panel(cookie)
}

/// `302` al panel con la cookie de sesión en `Set-Cookie`.
fn redireccion_302_al_panel(cookie: Cookie<'static>) -> Response {
    let mut respuesta = redireccion_302(DESTINO_PANEL);

    match HeaderValue::from_str(&cookie.to_string()) {
        Ok(valor) => {
            respuesta.headers_mut().insert(header::SET_COOKIE, valor);
            respuesta
        }
        // Una cookie que no cabe en una cabecera no es una sesión a medias: es
        // un fallo al construirla.
        Err(error) => AppError::Internal(format!(
            "la cookie de sesión no es una cabecera válida: {error}"
        ))
        .into_response(),
    }
}

/// Parámetros de la consulta del callback.
///
/// Es `pub` porque el manejador que lo extrae también lo es: un parámetro de
/// tipo privado en una firma pública no compila.
#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    /// Código de autorización que devuelve el proveedor.
    code: Option<String>,
    /// `state` emitido en el login: la clave con la que se recupera el `nonce`.
    state: Option<String>,
    /// Error declarado por el proveedor (`access_denied`, por ejemplo).
    error: Option<String>,
}

/// El acuse del cierre de sesión.
#[derive(Debug, Serialize)]
pub struct AcuseDeCierre {
    /// Estado del proceso tras el cierre: siempre `ok`.
    pub status: &'static str,
}

/// La identidad que contesta la ruta de sesión.
#[derive(Debug, Serialize)]
pub struct IdentidadDeSesion {
    /// Identificador del usuario en el proveedor.
    pub sub: String,
    /// Correo del usuario.
    pub email: String,
}

/// `POST /api/v1/auth/logout` — pública. El logout es idempotente y borra la cookie aunque la sesión no verifique o falte.
///
/// Borra la cookie de sesión —la **misma** cookie, con el mismo `Path` y con
/// `Max-Age=0`— y marca la sesión como cerrada en la caché de revocación. La
/// marca cierra la carrera logout↔refresh: cualquier petición ya en vuelo detecta
/// el cierre al leer el *gate* la caché y contesta `401`.
///
/// Se declara como `POST` y no como `GET` porque cambia estado: un `GET` lo
/// dispararían un enlace, un prefetch o una imagen.
pub async fn logout(
    jar: CookieJar,
    State(state): State<AppState>,
) -> (CookieJar, ApiResponse<AcuseDeCierre>) {
    // Si hay una cookie, extraer su jti para marcarlo como cerrado por logout.
    // El logout no se detiene si la cookie no verifica ni está ausente: sigue
    // borrando la cookie del navegador. Pero sin jti válido no hay nada que
    // cerrar en la caché.
    if let Some(auth) = state.auth.as_ref()
        && let Some(cookie) = jar.get(SESSION_COOKIE_NAME)
        && let Ok(claims) = session::verify(&auth.secret, cookie.value())
        && let Some(jti) = claims.jti.as_deref()
    {
        auth.cerrar_sesion_por_logout(jti).await;
    }

    // La `Set-Cookie` de borrado viaja por el propio frasco de la petición: es lo
    // que hace que la respuesta lleve la cabecera y la envoltura a la vez. La
    // cookie la define [`session::cookie_de_borrado`], la misma que manda el `401`
    // por revocación del proveedor: un solo borrado para los dos casos.
    let jar = jar.add(session::cookie_de_borrado());

    (jar, ApiResponse::new(AcuseDeCierre { status: "ok" }))
}

/// `GET /api/v1/auth/session` — protegida.
///
/// Contesta la identidad de la sesión —su `sub` y su `email`— sin devolver el
/// JWT. Sin cookie válida, el `401` del *gate* **es** la respuesta a «¿hay
/// sesión?».
pub async fn session(
    Extension(identidad): Extension<SessionIdentity>,
) -> ApiResponse<IdentidadDeSesion> {
    // La identidad entra por las *extensions* que dejó el *gate*: aquí no se lee
    // la cookie ni se verifica el JWT otra vez, y el token no se devuelve.
    ApiResponse::new(IdentidadDeSesion {
        sub: identidad.sub,
        email: identidad.email,
    })
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, StatusCode, header};

    use crate::auth::session::SESSION_COOKIE_NAME;
    use crate::test_support::proveedor_falso::{IdToken, ProveedorFalso};
    use crate::test_support::{
        OIDC_CLIENT_ID, cookie_de_sesion, entorno_oidc_con_emisor, header_texto, peticion,
        peticion_con_cookie, peticion_cruda, valor_de_cookie,
    };

    /// Extrae el `state` y el `nonce` de la `Location` de un login.
    fn state_y_nonce(location: &str) -> (String, String) {
        let consulta = location
            .split_once('?')
            .map(|(_, consulta)| consulta)
            .expect("el login debe traer cadena de consulta");

        let mut state = None;
        let mut nonce = None;

        for par in consulta.split('&') {
            if let Some((clave, valor)) = par.split_once('=') {
                match clave {
                    "state" => state = Some(valor.to_owned()),
                    "nonce" => nonce = Some(valor.to_owned()),
                    _ => {}
                }
            }
        }

        (
            state.expect("la Location debe traer state"),
            nonce.expect("la Location debe traer nonce"),
        )
    }

    /// Escenario «Redirección al proveedor»: el login contesta `302` con el
    /// `authorization_endpoint` del proveedor y los parámetros del flujo.
    #[tokio::test]
    async fn el_login_redirige_al_proveedor_con_state_y_nonce() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/auth/login").await;

        assert_eq!(respuesta.status(), StatusCode::FOUND);

        let location = header_texto(&respuesta, header::LOCATION).expect("el 302 trae Location");

        assert!(
            location.starts_with(&format!("{}/authorize?", proveedor.issuer)),
            "la redirección debe apuntar al authorization_endpoint: {location}"
        );

        let (state, nonce) = state_y_nonce(&location);

        assert_eq!(state.len(), 32, "el state debe tener 32 caracteres");
        assert_eq!(nonce.len(), 32, "el nonce debe tener 32 caracteres");
        assert_ne!(state, nonce, "el state y el nonce deben ser distintos");
    }

    /// Escenario «Callback correcto»: con un código válido y el `state` esperado
    /// se emite la cookie de sesión y se vuelve al panel.
    #[tokio::test]
    async fn el_callback_correcto_emite_la_cookie_y_vuelve_al_panel() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let login = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/auth/login").await;
        let location = header_texto(&login, header::LOCATION).expect("el 302 trae Location");
        let (state, nonce) = state_y_nonce(&location);

        proveedor
            .montar_token_endpoint(&proveedor.firmar(&IdToken::valido(
                &proveedor.issuer,
                OIDC_CLIENT_ID,
                &nonce,
            )))
            .await;

        let respuesta = peticion_cruda(
            &entorno.app(),
            Method::GET,
            &format!("/api/v1/auth/callback?code=un-codigo&state={state}"),
        )
        .await;

        assert_eq!(respuesta.status(), StatusCode::FOUND);
        assert_eq!(
            header_texto(&respuesta, header::LOCATION).as_deref(),
            Some("/"),
            "el callback debe volver al panel"
        );

        let cookie = header_texto(&respuesta, header::SET_COOKIE)
            .expect("el callback debe emitir la cookie de sesión");

        assert!(
            cookie.starts_with(&format!("{SESSION_COOKIE_NAME}=")),
            "la Set-Cookie debe ser la de sesión: {cookie}"
        );
    }

    /// Escenario «State reutilizado»: el mismo `state` no vale dos veces.
    #[tokio::test]
    async fn un_state_reutilizado_se_rechaza_sin_emitir_sesion() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let login = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/auth/login").await;
        let location = header_texto(&login, header::LOCATION).expect("el 302 trae Location");
        let (state, nonce) = state_y_nonce(&location);

        proveedor
            .montar_token_endpoint(&proveedor.firmar(&IdToken::valido(
                &proveedor.issuer,
                OIDC_CLIENT_ID,
                &nonce,
            )))
            .await;

        let url = format!("/api/v1/auth/callback?code=un-codigo&state={state}");

        let primero = peticion_cruda(&entorno.app(), Method::GET, &url).await;
        assert_eq!(primero.status(), StatusCode::FOUND, "el primero debe valer");

        let segundo = peticion_cruda(&entorno.app(), Method::GET, &url).await;
        assert_eq!(
            segundo.status(),
            StatusCode::UNAUTHORIZED,
            "el segundo consumo del mismo state no puede valer"
        );
        assert!(
            header_texto(&segundo, header::SET_COOKIE).is_none(),
            "un rechazo no puede emitir cookie de sesión"
        );
    }

    /// Escenario «State desconocido»: un `state` que nadie emitió se rechaza.
    #[tokio::test]
    async fn un_state_desconocido_se_rechaza_sin_emitir_sesion() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::GET,
            "/api/v1/auth/callback?code=un-codigo&state=inventado",
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(cuerpo["error"]["code"], "unauthorized");
    }

    /// Escenario «ID token inválido»: un ID token que no verifica su firma se
    /// rechaza sin emitir sesión.
    #[tokio::test]
    async fn un_id_token_invalido_se_rechaza_sin_emitir_sesion() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let login = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/auth/login").await;
        let location = header_texto(&login, header::LOCATION).expect("el 302 trae Location");
        let (state, nonce) = state_y_nonce(&location);

        proveedor
            .montar_token_endpoint(&proveedor.firmar_con_clave_ajena(&IdToken::valido(
                &proveedor.issuer,
                OIDC_CLIENT_ID,
                &nonce,
            )))
            .await;

        let respuesta = peticion_cruda(
            &entorno.app(),
            Method::GET,
            &format!("/api/v1/auth/callback?code=un-codigo&state={state}"),
        )
        .await;

        assert_eq!(respuesta.status(), StatusCode::UNAUTHORIZED);
        assert!(
            header_texto(&respuesta, header::SET_COOKIE).is_none(),
            "un ID token inválido no puede emitir cookie"
        );
    }

    /// Escenario «Identidad de la sesión»: con sesión válida, la ruta devuelve la
    /// identidad sin el JWT.
    #[tokio::test]
    async fn la_ruta_de_sesion_devuelve_la_identidad() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let (status, cuerpo) = peticion_con_cookie(
            &entorno.app(),
            Method::GET,
            "/api/v1/auth/session",
            &cookie_de_sesion(),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cuerpo["data"]["sub"], crate::test_support::SUB);
        assert_eq!(cuerpo["data"]["email"], crate::test_support::EMAIL);
    }

    /// Escenario «Cierre de sesión»: el `logout` borra la cookie.
    #[tokio::test]
    async fn el_cierre_borra_la_cookie() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let respuesta = peticion_con_cookie_cruda(
            &entorno.app(),
            Method::POST,
            "/api/v1/auth/logout",
            &cookie_de_sesion(),
        )
        .await;

        assert_eq!(respuesta.status(), StatusCode::OK);

        let cabecera = header_texto(&respuesta, header::SET_COOKIE)
            .expect("el logout debe traer una Set-Cookie de borrado");

        assert!(
            cabecera.starts_with(&format!("{SESSION_COOKIE_NAME}=;")),
            "la cookie de borrado debe dejar el valor vacío: {cabecera}"
        );
        assert!(
            cabecera.contains("Max-Age=0"),
            "la cookie de borrado debe llevar Max-Age=0: {cabecera}"
        );
        assert!(
            cabecera.contains("Path=/"),
            "la cookie de borrado debe llevar el mismo Path que la de sesión: {cabecera}"
        );
    }

    /// Escenario «Cierre sin cookie»: el `logout` se atiende igual —contesta
    /// `200` y la cookie de borrado— aunque la petición no traiga sesión.
    #[tokio::test]
    async fn el_cierre_atiende_sin_cookie() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let (status, _) = peticion(&entorno.app(), Method::POST, "/api/v1/auth/logout").await;
        assert_eq!(
            status,
            StatusCode::OK,
            "el cierre no puede exigir sesión: tiene que borrar la cookie igual"
        );

        let respuesta = peticion_cruda(&entorno.app(), Method::POST, "/api/v1/auth/logout").await;
        assert_eq!(respuesta.status(), StatusCode::OK);

        let cabecera = header_texto(&respuesta, header::SET_COOKIE)
            .expect("el logout sin cookie también debe traer la Set-Cookie de borrado");

        assert!(
            cabecera.starts_with(&format!("{SESSION_COOKIE_NAME}=;")),
            "la cookie de borrado debe dejar el valor vacío: {cabecera}"
        );
        assert!(
            cabecera.contains("Max-Age=0"),
            "la cookie de borrado debe llevar Max-Age=0: {cabecera}"
        );
        assert!(
            cabecera.contains("Path=/"),
            "la cookie de borrado debe llevar el mismo Path que la de sesión: {cabecera}"
        );
    }

    /// Escenario «Cierre con cookie inválida»: una cookie que no verifica no
    /// impide el cierre; se atiende `200` y se borra la cookie.
    #[tokio::test]
    async fn el_cierre_atiende_con_cookie_invalida() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let respuesta = peticion_con_cookie_cruda(
            &entorno.app(),
            Method::POST,
            "/api/v1/auth/logout",
            &format!("{SESSION_COOKIE_NAME}=no-es-un-jwt"),
        )
        .await;

        assert_eq!(
            respuesta.status(),
            StatusCode::OK,
            "una cookie que no verifica no puede bloquear el cierre"
        );

        let cabecera = header_texto(&respuesta, header::SET_COOKIE)
            .expect("el logout con cookie inválida también debe traer la Set-Cookie de borrado");

        assert!(
            cabecera.starts_with(&format!("{SESSION_COOKIE_NAME}=;")),
            "la cookie de borrado debe dejar el valor vacío: {cabecera}"
        );
        assert!(
            cabecera.contains("Max-Age=0"),
            "la cookie de borrado debe llevar Max-Age=0: {cabecera}"
        );
        assert!(
            cabecera.contains("Path=/"),
            "la cookie de borrado debe llevar el mismo Path que la de sesión: {cabecera}"
        );
    }

    /// El `logout` no se declara como `GET`: un `GET` lo dispararían un enlace o
    /// un prefetch.
    #[tokio::test]
    async fn el_cierre_no_se_declara_como_get() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        let (status, cuerpo) = peticion_con_cookie(
            &entorno.app(),
            Method::GET,
            "/api/v1/auth/logout",
            &cookie_de_sesion(),
        )
        .await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// Escenario «Racing logout↔refresh»: tras el cierre, una petición en vuelo con
    /// la cookie antigua no se renueva —el *gate* detecta que el logout cerró la
    /// sesión (`esta_revocado_por_logout`) y contesta `401`. Es el cierre de la
    /// carrera entre el logout del usuario y la renovación automática que
    /// intentaría salvar la sesión.
    #[tokio::test]
    async fn el_logout_bloquea_la_renovacionautomatica_de_cookies_antiguas() {
        let proveedor = ProveedorFalso::servir().await;
        let entorno = entorno_oidc_con_emisor(&proveedor.issuer).await;

        // Paso 1: login para obtener cookie válida.
        let login = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/auth/login").await;
        let location =
            header_texto(&login, header::LOCATION).expect("el login devuelve 302 con Location");
        let (state, nonce) = state_y_nonce(&location);

        proveedor
            .montar_token_endpoint(&proveedor.firmar(&IdToken::valido(
                &proveedor.issuer,
                OIDC_CLIENT_ID,
                &nonce,
            )))
            .await;

        let callback_resp = peticion_cruda(
            &entorno.app(),
            Method::GET,
            &format!("/api/v1/auth/callback?code=un-codigo&state={state}"),
        )
        .await;
        assert_eq!(callback_resp.status(), StatusCode::FOUND);

        // Extraer la cookie de sesión emitida por el callback.
        let cookie_nueva =
            header_texto(&callback_resp, header::SET_COOKIE).expect("el callback emite Set-Cookie");
        let nombre_token = valor_de_cookie(&cookie_nueva, SESSION_COOKIE_NAME)
            .expect("la Set-Cookie trae el token");
        let token_sesion = format!("{SESSION_COOKIE_NAME}={nombre_token}");

        // Paso 2: verificar que la ruta protegida responde 200 con la cookie.
        let (status, _) =
            peticion_con_cookie(&entorno.app(), Method::GET, "/api/v1/units", &token_sesion).await;
        assert_eq!(status, StatusCode::OK, "una sesión válida accede a /units");

        // Paso 3: cerrar la sesión (logout).
        let logout = peticion_con_cookie_cruda(
            &entorno.app(),
            Method::POST,
            "/api/v1/auth/logout",
            &token_sesion,
        )
        .await;
        assert_eq!(logout.status(), StatusCode::OK);

        // Paso 4: reutilizar la misma cookie de sesión (sin renovar) —debe fallar.
        // Esto reproduce una petición ya en vuelo cuando el logout acabó de responder.
        let (status_revocada, cuerpo_revocada) =
            peticion_con_cookie(&entorno.app(), Method::GET, "/api/v1/units", &token_sesion).await;

        assert_eq!(
            status_revocada,
            StatusCode::UNAUTHORIZED,
            "tras logout, la cookie antigua se rechaza con 401 (no se renueva)"
        );
        assert_eq!(
            cuerpo_revocada["error"]["code"], "unauthorized",
            "el rechazo lleva la envoltura JSON estándar"
        );
    }

    /// Lanza una petición con cookie y devuelve la respuesta sin leer el cuerpo.
    ///
    /// Es una ayuda local del módulo: el cuerpo de la respuesta de `logout` no es
    /// JSON —lleva una cabecera que hay que mirar— y el de `session` sí, así que
    /// conviven las dos formas.
    async fn peticion_con_cookie_cruda(
        app: &axum::Router,
        method: Method,
        uri: &str,
        cookie: &str,
    ) -> axum::response::Response {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        app.clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .expect("la petición de prueba se construye siempre"),
            )
            .await
            .expect("el router no puede fallar al atender")
    }
}
