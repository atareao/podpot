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

use crate::auth::oidc;
use crate::auth::session::{self, Claims, SESSION_COOKIE_NAME};
use crate::auth::state::AuthState;
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

/// Motivo con el que se registra la sesión que el proveedor ha revocado.
///
/// No viaja al cuerpo: al cliente le llega el [`SIN_SESION`] genérico —quien ya
/// no tiene sesión no gana nada sabiendo si fue por caducidad, por firma o por
/// revocación del proveedor—. El motivo concreto se queda en el registro.
const SESION_REVOCADA: &str = "el proveedor ha revocado la sesión";

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
/// sesión actual, que sigue siendo válida. La única excepción es que el proveedor
/// **rechace** el `refresh_token`: entonces la sesión ya no vale y se contesta
/// `401` con la cookie borrada.
pub async fn gate(
    State(estado): State<AppState>,
    jar: CookieJar,
    mut request: Request,
    next: Next,
) -> Response {
    let (identidad, renovada) = match con_cookie(&estado, &jar).await {
        Ok(ResolucionDeSesion::Valida(identidad, renovada)) => (identidad, renovada),
        Ok(ResolucionDeSesion::Revocada) => return sesion_revocada(),
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

/// El desenlace de resolver la sesión de una petición en el *gate*.
///
/// Existe para poder señalar el tercer caso —la sesión que el proveedor ha
/// revocado— sin cambiar el contrato externo: el *gate* traduce `Valida` a pasar
/// o a pasar con cookie nueva, y `Revocada` a un `401` con la cookie borrada.
///
/// La cookie viaja boxeada porque una `Cookie` es mucho más grande que el
/// discriminante de la variante y, sin el `Box`, cada `Valida` de la sesión
/// vigente acarrearía ese tamaño aunque no haya renovación —que es el caso
/// común—.
enum ResolucionDeSesion {
    /// Sesión válida, con la identidad y, si tocaba renovar, la cookie nueva.
    Valida(SessionIdentity, Option<Box<Cookie<'static>>>),
    /// El proveedor rechazó el `refresh_token`: la sesión ya no es válida.
    Revocada,
}

/// El `401` de la sesión revocada por el proveedor: la envoltura **y** la cookie
/// borrada.
///
/// Es la única renovación que cambia la respuesta: el proveedor ha dicho que el
/// `refresh_token` no vale, así que la sesión local deja de considerarse válida y
/// se manda la cookie de borrado para que el navegador no la reintente. Reutiliza
/// la misma cookie de borrado que el `logout` —mismo nombre, mismo `Path`— para
/// que el cliente no vea dos borrados distintos. El **cuerpo** es el `401`
/// genérico [`SIN_SESION`] y no [`SESION_REVOCADA`]: quien ya no tiene sesión no
/// gana nada sabiendo que fue el proveedor, y el motivo concreto se queda en el
/// registro.
fn sesion_revocada() -> Response {
    tracing::debug!(motivo = SESION_REVOCADA, "sesión revocada por el proveedor");

    let mut respuesta = AppError::Unauthorized(SIN_SESION.to_owned()).into_response();

    match HeaderValue::from_str(&session::cookie_de_borrado().to_string()) {
        Ok(valor) => {
            respuesta.headers_mut().append(header::SET_COOKIE, valor);
        }
        Err(error) => {
            tracing::error!(%error, "la Set-Cookie de borrado no es una cabecera válida");
        }
    }

    respuesta
}

/// Resuelve la identidad por el camino de la cookie de sesión.
///
/// Devuelve también la cookie renovada, si toca. Un fallo al renovar no invalida
/// la sesión: se registra y se devuelve `None`, porque la petición puede seguir
/// con el token actual hasta que caduque. Solo un rechazo explícito del
/// `refresh_token` por el proveedor devuelve [`ResolucionDeSesion::Revocada`].
///
/// La revocación se consulta **antes** de intentar renovar: una sesión cuyo `jti`
/// ya está marcado como revocado no se renueva —ni contra el proveedor ni
/// localmente—, porque su `refresh_token` ya se descartó y la renovación local la
/// resucitaría.
async fn con_cookie(estado: &AppState, jar: &CookieJar) -> Result<ResolucionDeSesion, AppError> {
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

    // La revocación es **pegajosa**: si el proveedor ya rechazó el `refresh_token`
    // de esta sesión, el `jti` está marcado y ninguna petición posterior con la
    // misma cookie se renueva. Se consulta antes de renovar porque, con el token
    // ya borrado, la petición siguiente caería en la renovación local y
    // resucitaría la sesión.
    if let Some(jti) = claims.jti.as_deref()
        && auth.esta_revocado(jti).await
    {
        return Ok(ResolucionDeSesion::Revocada);
    }

    if !session::should_renew(&claims, get_current_timestamp()) {
        return Ok(ResolucionDeSesion::Valida(identidad_de(&claims), None));
    }

    // Dentro de la ventana. Si la sesión tiene `jti` y hay un `refresh_token`
    // asociado, se refresca el ID token contra el proveedor antes de reemitir; sin
    // él, la renovación local de siempre.
    if let Some(jti) = claims.jti.as_deref()
        && let Some(refresh_token) = auth.obtener_refresh_token_por_jti(jti).await
    {
        return renovar_con_proveedor(auth, jti, &refresh_token, &claims).await;
    }

    Ok(ResolucionDeSesion::Valida(
        identidad_de(&claims),
        renovacion_local(auth, &claims),
    ))
}

/// Renueva la sesión pidiendo un ID token nuevo al proveedor con el
/// `refresh_token` asociado.
///
/// Es el camino de la sesión que sí tiene refresco. Distingue los tres desenlaces
/// de [`oidc::RefreshProveedor`]: un ID token nuevo —que se verifica **sin
/// `nonce`** antes de reemitir y, si el proveedor rota el token, sustituye al
/// anterior—, un rechazo del token —que cierra la sesión— y un fallo del
/// proveedor —que mantiene la sesión local—.
///
/// El refresh va **serializado por `jti`**: solo una petición de la misma sesión
/// lo pide a la vez. Con un proveedor que **rota** el token, las que esperan
/// encuentran el token ya cambiado y reutilizan el resultado sin volver a llamar
/// al proveedor; con uno que **no** rota, el token sigue igual y pueden llegar a
/// pedir el refresh dos veces —inofensivo, porque el mismo token sigue valiendo—.
/// Sin esta serialización, con un proveedor de uso único dos peticiones
/// concurrentes usarían el mismo token, una lo rotaría y la otra recibiría un
/// `invalid_grant` que no es revocación, cerrando la sesión sin motivo.
async fn renovar_con_proveedor(
    auth: &AuthState,
    jti: &str,
    refresh_token: &str,
    claims: &Claims,
) -> Result<ResolucionDeSesion, AppError> {
    let cerrojo = auth.cerrojo_de_refresh(jti).await;
    // El guardia vive hasta el final de la función: es lo que serializa el
    // refresh y lo que hace que las peticiones siguientes de la misma sesión
    // esperen a que este termine.
    let _guardia = cerrojo.lock().await;

    // Otra petición pudo revocar la sesión mientras esta esperaba el candado: la
    // marca es pegajosa y se comprueba también aquí, porque la comprobación de
    // `con_cookie` ocurrió antes de que la revocación se escribiera. Sin esto, una
    // petición ya en vuelo se llevaría un `200` sobre una sesión revocada.
    if auth.esta_revocado(jti).await {
        tracing::debug!("la sesión se revocó mientras se esperaba el turno de refresh");

        return Ok(ResolucionDeSesion::Revocada);
    }

    // Ya en el turno, se relee el token: si cambió desde que esta petición lo
    // leyó, otra se adelantó y ya renovó. No se vuelve a pedir nada al proveedor
    // —reutiliza el resultado con una renovación local—.
    if auth.obtener_refresh_token_por_jti(jti).await.as_deref() != Some(refresh_token) {
        tracing::debug!("otra petición ya renovó la sesión; se reutiliza el resultado");

        return Ok(ResolucionDeSesion::Valida(
            identidad_de(claims),
            renovacion_local(auth, claims),
        ));
    }

    let discovery = match oidc::discovery_cached(auth).await {
        Ok(discovery) => discovery,
        Err(error) => {
            tracing::error!(
                %error,
                "no se pudo descubrir el proveedor al renovar; se mantiene la sesión local"
            );
            return Ok(ResolucionDeSesion::Valida(
                identidad_de(claims),
                renovacion_local(auth, claims),
            ));
        }
    };

    match oidc::refrescar_id_token(auth.settings.as_ref(), &discovery, refresh_token).await {
        oidc::RefreshProveedor::Nuevo {
            id_token,
            refresh_token: nuevo,
        } => match oidc::verificar_id_token_renovado(auth, &id_token).await {
            // La identidad del refresh tiene que ser la de la sesión: un ID token
            // de otro usuario no puede reetiquetar la cookie. Se trata como un
            // fallo del proveedor —se mantiene la sesión local—, no como una
            // identidad nueva.
            Ok(identidad) if identidad.sub != claims.sub => {
                // El token nuevo, si lo hay, no se pierde: se guarda antes de
                // caer al camino local, para no forzar un `invalid_grant` en la
                // siguiente renovación.
                if let Some(nuevo) = nuevo {
                    auth.rotar_refresh_token(jti, &nuevo).await;
                }

                tracing::error!(
                    sub_refresh = %identidad.sub,
                    sub_sesion = %claims.sub,
                    "el ID token del refresh es de otro usuario; se mantiene la sesión local"
                );

                Ok(ResolucionDeSesion::Valida(
                    identidad_de(claims),
                    renovacion_local(auth, claims),
                ))
            }
            Ok(identidad) => {
                // La rotación es real: si el proveedor emite un `refresh_token`
                // nuevo, sustituye al anterior bajo el mismo `jti`, de modo que la
                // próxima renovación use el vigente.
                if let Some(nuevo) = nuevo {
                    auth.rotar_refresh_token(jti, &nuevo).await;
                }

                let identidad = SessionIdentity {
                    sub: identidad.sub,
                    email: identidad.email,
                };
                let cookie = emitir_cookie(auth, &identidad, Some(jti.to_owned()));

                Ok(ResolucionDeSesion::Valida(identidad, cookie))
            }
            Err(error) => {
                // El ID token del refresh no verifica, pero el `refresh_token`
                // nuevo —si el proveedor lo rotó— sí vale: se guarda para que la
                // siguiente renovación no se encuentre con el viejo y un
                // `invalid_grant`.
                if let Some(nuevo) = nuevo {
                    auth.rotar_refresh_token(jti, &nuevo).await;
                }

                tracing::error!(
                    %error,
                    "el ID token del refresh no verifica; se mantiene la sesión local"
                );
                Ok(ResolucionDeSesion::Valida(
                    identidad_de(claims),
                    renovacion_local(auth, claims),
                ))
            }
        },
        oidc::RefreshProveedor::InvalidGrant => {
            // Antes de dar la sesión por revocada, se relee el token: si cambió
            // desde el que se usó, no es revocación —es la rotación de otra
            // petición— y no se cierra la sesión.
            if auth.obtener_refresh_token_por_jti(jti).await.as_deref() != Some(refresh_token) {
                tracing::debug!("el token cambió desde que se usó; no es revocación");

                return Ok(ResolucionDeSesion::Valida(
                    identidad_de(claims),
                    renovacion_local(auth, claims),
                ));
            }

            // El proveedor ha revocado el token: se descarta, se marca el `jti`
            // —para que la revocación no se quede en esta petición— y la sesión se
            // considera sin sesión.
            tracing::debug!("el proveedor revocó el refresh token; se cierra la sesión");
            auth.borrar_refresh_token(jti).await;
            auth.revocar_jti(jti).await;

            Ok(ResolucionDeSesion::Revocada)
        }
        oidc::RefreshProveedor::Fallo(error) => {
            tracing::error!(
                %error,
                "el refresh del proveedor falló; se mantiene la sesión local"
            );
            Ok(ResolucionDeSesion::Valida(
                identidad_de(claims),
                renovacion_local(auth, claims),
            ))
        }
    }
}

/// La identidad pública de unos `claims` ya verificados.
///
/// Deja fuera el `jti` y los `iat`/`exp`: el manejador solo necesita saber quién
/// es el operador.
fn identidad_de(claims: &Claims) -> SessionIdentity {
    SessionIdentity {
        sub: claims.sub.clone(),
        email: claims.email.clone(),
    }
}

/// Renueva la sesión con su propia firma, sin tocar el proveedor.
///
/// Es la renovación de siempre —conserva la identidad y el `jti` y alarga la
/// vida— y también el camino de respaldo cuando el proveedor no está disponible o
/// el ID token del refresh no verifica: la sesión local sigue valiendo.
fn renovacion_local(auth: &AuthState, claims: &Claims) -> Option<Box<Cookie<'static>>> {
    emitir_cookie(auth, &identidad_de(claims), claims.jti.clone())
}

/// Emite la cookie de sesión con la identidad y el `jti` dados.
///
/// Un fallo al firmar o al construir la cookie no invalida la sesión: se registra
/// y se devuelve `None`, porque la petición puede seguir con el token actual
/// hasta que caduque.
fn emitir_cookie(
    auth: &AuthState,
    identidad: &SessionIdentity,
    jti: Option<String>,
) -> Option<Box<Cookie<'static>>> {
    match session::issue_con_jti(
        &auth.secret,
        identidad.sub.clone(),
        identidad.email.clone(),
        jti,
    )
    .and_then(|token| session::session_cookie(token, &auth.settings.redirect_url))
    {
        Ok(cookie) => Some(Box::new(cookie)),
        Err(error) => {
            tracing::error!(%error, "no se pudo renovar la sesión; se mantiene la actual");
            None
        }
    }
}
