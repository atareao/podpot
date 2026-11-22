//! El *gate* de la superficie de negocio: sin sesión válida no se pasa.
//!
//! Es una API, así que el rechazo es un **`401` con la envoltura** `{error}`,
//! nunca una redirección ni HTML: el panel necesita distinguir «sin sesión» de
//! cualquier otro fallo para decidir si vuelve a la portada, y una redirección a
//! un `fetch` no le sirve.
//!
//! La credencial es **sólo** la cookie de sesión. Podpot no tiene tokens de API
//! —no hay nada que una máquina tenga que accionar sin un navegador—, así que el
//! camino es uno: leer la cookie, verificar el JWT y dejar la identidad en las
//! *extensions* de la petición.
//!
//! La sesión se renueva de forma **deslizante**: cuando al token válido le quedan
//! dos horas o menos de vida, el *gate* lo reemite y lo añade a la respuesta en
//! `Set-Cookie`. Así el operador no vuelve a entrar en mitad de una jornada. La
//! renovación no cambia el cuerpo ni el código de la respuesta: sólo añade la
//! cabecera. No hay tope absoluto de vida —cada renovación vuelve a empezar las
//! ocho horas— y varias peticiones simultáneas pueden emitir varias cookies: la
//! última en llegar gana en el navegador.

use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use jsonwebtoken::get_current_timestamp;

use crate::auth::session::{self, SESSION_COOKIE_NAME};
use crate::error::AppError;
use crate::state::AppState;

/// Identidad de la sesión ya verificada, tal y como la ve un manejador.
///
/// Viaja en las *extensions* de la petición —el camino natural de axum para
/// pasar datos de una capa al manejador— y no vuelve a leerse de la cookie: el
/// manejador no tiene por qué saber que la identidad viaja dentro de un JWT, ni
/// tiene por qué verificarlo otra vez.
///
/// Lleva **sólo** `sub` y `email`, no los `iat`/`exp` de
/// [`crate::auth::session::Claims`]: la vigencia ya la comprobó el *gate*, y
/// exponerla invitaría a reimplementar esa comprobación en cada manejador.
#[derive(Debug, Clone)]
pub struct SessionIdentity {
    /// Identificador del usuario en el proveedor de identidad.
    pub sub: String,
    /// Correo del usuario.
    pub email: String,
}

/// Motivo con el que se rechaza una petición que no trae una cookie de sesión
/// válida.
///
/// No distingue «no hay cookie» de «la cookie no vale»: decir cuál de las dos es
/// contaría a quien no ha entrado si su sesión caducó o si la firma no cuadra.
/// El detalle concreto se queda en el registro, dentro de
/// [`crate::auth::session::verify`].
const SIN_SESION: &str = "hace falta una sesión válida para esta ruta";

/// *Gate* de la superficie protegida.
///
/// Lee la cookie de sesión, verifica el JWT y, si vale, deja la identidad en las
/// *extensions* y pasa al manejador. Sin cookie o con una que no verifique,
/// contesta `401` con la envoltura `{error}` —nunca con el cuerpo vacío, nunca
/// con HTML y nunca con una redirección—.
///
/// Si la sesión está dentro de la ventana de renovación, añade la cookie nueva a
/// la respuesta del manejador con `Set-Cookie`. El código y el cuerpo no cambian:
/// la renovación es invisible para el cliente salvo por la cabecera. Un fallo al
/// renovar no convierte la petición en un `500`: se registra y se sigue con la
/// sesión actual, que sigue siendo válida.
pub async fn gate(
    State(estado): State<AppState>,
    jar: CookieJar,
    mut request: Request,
    next: Next,
) -> Response {
    let (identidad, renovada) = match con_cookie(&estado, &jar) {
        Ok(par) => par,
        Err(rechazo) => return rechazo.into_response(),
    };

    // La identidad viaja en las *extensions*: el manejador la lee de ahí y no
    // vuelve a tocar la cookie.
    request.extensions_mut().insert(identidad);

    let mut respuesta = next.run(request).await;

    if let Some(cookie) = renovada {
        match HeaderValue::from_str(&cookie.to_string()) {
            Ok(valor) => {
                respuesta.headers_mut().append(header::SET_COOKIE, valor);
            }
            Err(error) => {
                tracing::error!(%error, "la Set-Cookie de la renovación no es una cabecera válida");
            }
        }
    }

    respuesta
}

/// Resuelve la identidad por el camino de la cookie de sesión.
///
/// Devuelve también la cookie renovada, si toca. Un fallo al renovar no invalida
/// la sesión: se registra y se devuelve `None`, porque la petición puede seguir
/// con el token actual hasta que caduque.
fn con_cookie(
    estado: &AppState,
    jar: &CookieJar,
) -> Result<(SessionIdentity, Option<Cookie<'static>>), AppError> {
    // Por construcción el *gate* sólo se monta donde hay proveedor, así que un
    // `None` aquí es un fallo de invariante: se rechaza la petición en vez de
    // abrirla.
    let auth = estado
        .auth
        .as_ref()
        .ok_or_else(|| AppError::Unauthorized(SIN_SESION.to_owned()))?;

    let cookie = jar
        .get(SESSION_COOKIE_NAME)
        .ok_or_else(|| AppError::Unauthorized(SIN_SESION.to_owned()))?;

    let claims = session::verify(&auth.secret, cookie.value())?;

    let renovada = if session::should_renew(&claims, get_current_timestamp()) {
        match session::renew(&auth.secret, &claims)
            .and_then(|token| session::session_cookie(token, &auth.settings.redirect_url))
        {
            Ok(cookie) => Some(cookie),
            Err(error) => {
                tracing::error!(%error, "no se pudo renovar la sesión; se mantiene la actual");
                None
            }
        }
    } else {
        None
    };

    Ok((
        SessionIdentity {
            sub: claims.sub,
            email: claims.email,
        },
        renovada,
    ))
}
