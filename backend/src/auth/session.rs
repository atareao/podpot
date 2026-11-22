//! La sesión del operador: el JWT que viaja en la cookie y los atributos de esta.
//!
//! La sesión no tiene estado en el servidor: es un JWT firmado con
//! `PODPOT_SECRET` que sobrevive a un reinicio y no depende de que exista la
//! base de datos. El *gate* lo verifica en cada petición —firma y vigencia—, así
//! que no hay ninguna tabla ni store que consultar y ninguna sesión que revocar
//! en el servidor: la revocación es la ausencia de una cookie válida.

use axum_extra::extract::cookie::{Cookie, SameSite};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode, get_current_timestamp,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::secret::SecretString;

/// Nombre de la cookie de sesión.
///
/// Es una constante y no un literal repetido: el `logout` tiene que borrar
/// **exactamente** esta cookie, y el *gate* tiene que leer exactamente esta.
pub const SESSION_COOKIE_NAME: &str = "podpot_session";

/// Vida de la sesión: 8 horas.
///
/// Es una constante del módulo, no una variable de entorno: el panel es de un
/// solo operador y una sesión que caduque en mitad de una operación obliga a
/// rehacer el flujo de entrada. Ocho horas cubren una jornada de trabajo.
pub const SESSION_LIFETIME: std::time::Duration = std::time::Duration::from_secs(8 * 60 * 60);

/// Ventana de renovación deslizante: 2 horas.
///
/// Cuando a la sesión le quedan dos horas o menos de sus ocho de vida, el *gate*
/// la reemite con una vida nueva. Es un margen amplio a propósito: cubre una
/// jornada de trabajo sin que el operador vuelva a entrar, y deja tiempo de sobra
/// para que la renovación llegue antes de que el `exp` caduque.
pub const RENEWAL_WINDOW: std::time::Duration = std::time::Duration::from_secs(2 * 60 * 60);

/// Identidad de la sesión: exactamente `sub`, `email`, `iat` y `exp`.
///
/// La estructura es la que sella la superficie: con esos cuatro campos y
/// `deny_unknown_fields`, un token que traiga cualquier claim de más —un rol,
/// por ejemplo— no se puede deserializar y por tanto no se acepta. No hay que
/// acordarse de rechazarlo: no cabe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    /// Identificador del usuario en el proveedor de identidad.
    pub sub: String,
    /// Correo del usuario, tomado del ID token ya verificado.
    pub email: String,
    /// Momento de emisión, en segundos desde la época.
    pub iat: u64,
    /// Momento de caducidad, en segundos desde la época.
    pub exp: u64,
}

/// Emite el JWT de sesión firmado con `HS256` y `secret`.
///
/// Devuelve el token, no la cookie: quien lo llame decide si además lo envuelve
/// en una cabecera `Set-Cookie`.
pub fn issue(secret: &SecretString, sub: String, email: String) -> Result<String, AppError> {
    let emitido = get_current_timestamp();
    let claims = Claims {
        sub,
        email,
        iat: emitido,
        exp: emitido + SESSION_LIFETIME.as_secs(),
    };

    let cabecera = Header::new(Algorithm::HS256);
    // La clave entra por el tipo que redacta y solo sale por `expose`, en el
    // único sitio donde hace falta: firmar.
    let clave = EncodingKey::from_secret(secret.expose().as_bytes());

    encode(&cabecera, &claims, &clave).map_err(|error| {
        tracing::error!(%error, "no se pudo firmar el JWT de sesión");
        AppError::Internal("no se pudo emitir la sesión".to_owned())
    })
}

/// ¿Toca renovar la sesión?
///
/// Solo tiene sentido con `claims` **válidos**: un token ya caducado no se
/// renueva, porque no hay sesión que alargar —el *gate* lo rechaza antes con
/// `401`— y reemitirlo sería resucitar una sesión muerta. Devuelve `true`
/// cuando a `claims` le quedan [`RENEWAL_WINDOW`] o menos de vida y todavía no
/// ha llegado su `exp`. Se compara con `<=` para que el borde —exactamente dos
/// horas— también renueve: llegar justo al umbral no es un caso que convenga
/// dejar fuera.
pub fn should_renew(claims: &Claims, ahora: u64) -> bool {
    claims.exp > ahora && claims.exp <= ahora.saturating_add(RENEWAL_WINDOW.as_secs())
}

/// Reemite la sesión con la misma identidad y una vida nueva.
///
/// Conserva `sub` y `email` —la renovación no cambia quién es el operador— y
/// vuelve a firmar con `issue`, así que el token nuevo tiene el `iat` y el `exp`
/// de una emisión recién hecha. No toca la cookie: quien la envuelva decide si
/// además la manda en `Set-Cookie`.
pub fn renew(secret: &SecretString, claims: &Claims) -> Result<String, AppError> {
    issue(secret, claims.sub.clone(), claims.email.clone())
}

/// Emite un JWT de sesión con una caducidad dada, para las pruebas.
///
/// `issue` fija la vida de la sesión a [`SESSION_LIFETIME`]; este atajo permite
/// fabricar tokens con el `exp` que cada escenario necesita —recién emitido, a
/// punto de caducar o ya caducado— sin tocar la constante ni depender del reloj.
/// Firma con `HS256` y el mismo secreto que `issue`, así que `verify` lo acepta
/// igual que un token de producción.
#[cfg(test)]
pub(crate) fn issue_con_caducidad(
    secret: &SecretString,
    sub: String,
    email: String,
    exp: u64,
) -> Result<String, AppError> {
    let claims = Claims {
        sub,
        email,
        iat: get_current_timestamp(),
        exp,
    };

    let cabecera = Header::new(Algorithm::HS256);
    let clave = EncodingKey::from_secret(secret.expose().as_bytes());

    encode(&cabecera, &claims, &clave).map_err(|error| {
        tracing::error!(%error, "no se pudo firmar el JWT de sesión");
        AppError::Internal("no se pudo emitir la sesión".to_owned())
    })
}

/// Verifica la firma, el algoritmo y la vigencia de un JWT de sesión.
///
/// Rechaza con [`AppError::Unauthorized`] todo token mal firmado, firmado con
/// otra clave, caducado o manipulado. La cookie **ausente** no llega hasta aquí:
/// la rechaza el *gate* de la superficie protegida antes de llamar a esta
/// función, que trabaja sobre un token ya extraído.
pub fn verify(secret: &SecretString, token: &str) -> Result<Claims, AppError> {
    // El algoritmo se fija aquí y no se lee de la cabecera del token: aceptar el
    // que traiga el propio token es la confusión de algoritmo de manual.
    let mut validacion = Validation::new(Algorithm::HS256);
    // La sesión no declara `aud` —el panel es su único destinatario— y el
    // defecto de `Validation` tampoco lo rechazaría por ausencia: sin `aud` en la
    // validación, la rama que lo comprueba no se dispara. Se desactiva para que
    // el día que la sesión lleve un `aud` no quede a medio comprobar en silencio:
    // o se valida de verdad, con su valor esperado, o no se valida.
    validacion.validate_aud = false;
    // Sin margen de cortesía sobre `exp`: «`exp` en vigor» es literal. El
    // `leeway` que trae `Validation` por defecto —60 segundos— no cubre ningún
    // desfase entre relojes, porque no hay dos: el navegador no valida el JWT,
    // solo guarda la cookie, y el único reloj que interviene es el de este
    // proceso, que firma y verifica con el mismo. Lo único que haría ese minuto
    // es **alargar la vida de una sesión ya caducada**.
    validacion.leeway = 0;

    let clave = DecodingKey::from_secret(secret.expose().as_bytes());

    decode::<Claims>(token, &clave, &validacion)
        .map(|datos| datos.claims)
        .map_err(|error| {
            // El detalle se queda en el registro, que es donde el operador lo
            // necesita; el cliente recibe el 401 con su motivo.
            tracing::debug!(%error, "cookie de sesión rechazada");
            AppError::Unauthorized("la cookie de sesión no es válida".to_owned())
        })
}

/// Construye la cookie de sesión con el JWT dado.
///
/// `redirect_url` decide el atributo `Secure`: se añade solo cuando su esquema
/// es `https`, porque en desarrollo se entra por `http://127.0.0.1` y una cookie
/// `Secure` no viajaría, así que la sesión no se establecería nunca.
pub fn session_cookie(token: String, redirect_url: &str) -> Result<Cookie<'static>, AppError> {
    // El crate `cookie` espera una duración del crate `time`, que es una
    // dependencia transitiva y no se puede nombrar desde aquí. La conversión
    // desde `std::time::Duration` deja que el compilador infiera el tipo del
    // parámetro de `max_age`; con 8 horas no puede fallar, pero el error se
    // propaga tipado en vez de recurrir a un `unwrap`.
    let max_age = SESSION_LIFETIME.try_into().map_err(|_| {
        AppError::Internal("la vida de la sesión no cabe en el Max-Age de la cookie".to_owned())
    })?;

    Ok(Cookie::build((SESSION_COOKIE_NAME, token))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .secure(requires_secure(redirect_url))
        .max_age(max_age)
        .build())
}

/// ¿El esquema de la URL de retorno obliga a marcar la cookie como `Secure`?
///
/// Una URL que no se pueda leer como `esquema://…` no se considera segura: el
/// defecto útil es el del desarrollo, donde el retorno es `http` sobre loopback.
fn requires_secure(redirect_url: &str) -> bool {
    redirect_url
        .split_once("://")
        .is_some_and(|(esquema, _)| esquema.eq_ignore_ascii_case("https"))
}

#[cfg(test)]
mod tests {
    use axum_extra::extract::cookie::SameSite;
    use jsonwebtoken::{Algorithm, EncodingKey, Header};

    use super::{
        Claims, RENEWAL_WINDOW, SESSION_COOKIE_NAME, SESSION_LIFETIME, issue, renew,
        session_cookie, should_renew, verify,
    };
    use crate::error::AppError;
    use crate::secret::SecretString;

    /// Clave de firma de la sesión, con la longitud de sobra.
    const SECRET: &str = "clave-de-firma-distinguible-000000000000000000000000000000000000";
    /// Otra clave de firma, distinguible de la anterior: es la que no vale.
    const OTRO_SECRET: &str = "otra-clave-de-firma-000000000000000000000000000000000000000000";
    /// URL de retorno con esquema `http`: el caso del desarrollo.
    const REDIRECT_HTTP: &str = "http://127.0.0.1:8080/api/v1/auth/callback";
    /// URL de retorno con esquema `https`: el caso del despliegue.
    const REDIRECT_HTTPS: &str = "https://podpot.example.test/api/v1/auth/callback";
    /// Usuario distinguible de la sesión.
    const SUB: &str = "usuario-distinguible-1";
    /// Correo distinguible de la sesión.
    const EMAIL: &str = "operador-distinguible@example.test";
    /// ID token del proveedor, con forma de JWT ajeno: no puede pasar por sesión.
    const ID_TOKEN_DEL_PROVEEDOR: &str =
        "eyJhbGciOiJSUzI1NiJ9.ID-TOKEN-DEL-PROVEEDOR.firma-del-proveedor";

    /// Clave de firma, envuelta en el tipo que redacta.
    fn secret() -> SecretString {
        SecretString::new(SECRET.to_owned())
    }

    /// Emite una sesión con los valores distinguibles de estas pruebas.
    fn sesion() -> String {
        issue(&secret(), SUB.to_owned(), EMAIL.to_owned()).expect("la emisión no puede fallar")
    }

    /// Cancela la firma de un JWT alterando un carácter de su contenido.
    ///
    /// Es una manipulación real: el token sigue teniendo tres partes y sigue
    /// siendo base64url, pero ya no lo respalda la firma original.
    fn manipular(token: &str) -> String {
        let mut caracteres: Vec<char> = token.chars().collect();
        let posicion = caracteres.len() / 2;

        if let Some(caracter) = caracteres.get_mut(posicion) {
            *caracter = if *caracter == 'a' { 'b' } else { 'a' };
        }

        caracteres.into_iter().collect()
    }

    /// Firma un JWT de sesión con el `exp` indicado, saltándose `issue`.
    ///
    /// Es lo que permite fabricar un token caducado: la emisión real siempre
    /// pone `exp` a ocho horas vista.
    fn firmar_con_caducidad(exp: u64) -> String {
        let claims = Claims {
            sub: SUB.to_owned(),
            email: EMAIL.to_owned(),
            iat: exp - SESSION_LIFETIME.as_secs(),
            exp,
        };

        jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .expect("el token de prueba debe poder firmarse")
    }

    /// Claves del payload de un JWT, ordenadas y sin verificar la firma.
    fn claves_del_payload(token: &str) -> Vec<String> {
        let claims: serde_json::Value = jsonwebtoken::dangerous::insecure_decode_claims(token)
            .expect("el payload debe poder leerse como JSON");

        let mut claves: Vec<String> = claims
            .as_object()
            .expect("los claims deben ser un objeto JSON")
            .keys()
            .cloned()
            .collect();
        claves.sort();

        claves
    }

    /// Escenario «Sesión emitida y verificada»: el JWT que sale de `issue`
    /// vuelve a entrar por `verify` con la misma identidad.
    #[test]
    fn una_sesion_emitida_se_verifica_y_devuelve_su_identidad() {
        let claims = verify(&secret(), &sesion()).expect("el JWT emitido debe verificarse");

        assert_eq!(claims.sub, SUB);
        assert_eq!(claims.email, EMAIL);
    }

    /// Escenario «Atributos de la cookie»: `HttpOnly`, `SameSite=Strict`,
    /// `Path=/` y `Max-Age`, que es lo que viaja de verdad en la cabecera.
    #[test]
    fn los_atributos_de_la_cookie() {
        let cookie = session_cookie(sesion(), REDIRECT_HTTP).expect("la cookie se construye");

        assert_eq!(
            cookie.name(),
            SESSION_COOKIE_NAME,
            "el logout tendrá que borrar exactamente esta cookie"
        );
        assert_eq!(
            cookie.http_only(),
            Some(true),
            "la cookie debe ser HttpOnly"
        );
        assert_eq!(
            cookie.same_site(),
            Some(SameSite::Strict),
            "la cookie debe ser SameSite=Strict"
        );
        assert_eq!(
            cookie.path(),
            Some("/"),
            "la cookie debe valer para todo el panel"
        );
        assert!(
            cookie.max_age().is_some(),
            "la cookie debe llevar Max-Age, o el navegador la trataría de sesión"
        );

        let cabecera = cookie.to_string();

        for atributo in ["HttpOnly", "SameSite=Strict", "Path=/", "Max-Age="] {
            assert!(
                cabecera.contains(atributo),
                "la Set-Cookie no trae {atributo}: {cabecera}"
            );
        }
    }

    /// Escenario «Secure según el esquema»: con retorno `http` la cookie no lleva
    /// `Secure` —o no viajaría en desarrollo— y con retorno `https` sí.
    #[test]
    fn el_secure_depende_del_esquema_de_retorno() {
        let con_http = session_cookie(sesion(), REDIRECT_HTTP).expect("la cookie se construye");

        assert!(
            !con_http.to_string().contains("Secure"),
            "con retorno http la cookie no puede ir Secure, o no viajaría: {con_http}"
        );

        let con_https = session_cookie(sesion(), REDIRECT_HTTPS).expect("la cookie se construye");

        assert_eq!(
            con_https.secure(),
            Some(true),
            "con retorno https la cookie debe ir Secure"
        );
    }

    /// Escenario «Payload exacto»: la sesión lleva exactamente los cuatro claims
    /// y ninguno de rol, así que no hay dónde meter claims del proveedor.
    #[test]
    fn el_payload_de_la_sesion_son_exactamente_los_cuatro_claims() {
        let token = sesion();

        assert_eq!(
            claves_del_payload(&token),
            ["email", "exp", "iat", "sub"],
            "los claims de la sesión deben ser exactamente los cuatro: {token}"
        );

        assert!(
            verify(&secret(), ID_TOKEN_DEL_PROVEEDOR).is_err(),
            "un token del proveedor no puede pasar por sesión"
        );
    }

    /// Escenario «Payload exacto» (el filo de dentro): un token **bien firmado**
    /// con `PODPOT_SECRET` que lleva un claim de más no se puede deserializar, así
    /// que `verify` lo rechaza.
    ///
    /// Es lo que canda `deny_unknown_fields`: sin él, un token con un `role` —o con
    /// cualquier claim que un manejador futuro pudiera llegar a leer— entraría como
    /// sesión válida, y la superficie dejaría de estar sellada por la forma del
    /// struct. La firma es la legítima a propósito: lo único que separa a este
    /// token de una sesión es el claim de más.
    #[test]
    fn una_sesion_con_un_claim_de_mas_no_se_acepta() {
        let ahora = jsonwebtoken::get_current_timestamp();
        let claims = serde_json::json!({
            "sub": SUB,
            "email": EMAIL,
            "iat": ahora,
            "exp": ahora + SESSION_LIFETIME.as_secs(),
            "role": "admin",
        });

        let token = jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .expect("el token de prueba debe poder firmarse");

        let error = verify(&secret(), &token)
            .expect_err("un claim de más no puede pasar por sesión de podpot");

        assert!(
            matches!(error, AppError::Unauthorized(_)),
            "el rechazo debe ser un 401 tipado: {error}"
        );
    }

    /// La vida de la sesión es una constante de ocho horas, y el `exp` del JWT y
    /// el `Max-Age` de la cookie coinciden con ella.
    #[test]
    fn la_vida_de_la_sesion_es_una_constante_de_ocho_horas() {
        assert_eq!(
            SESSION_LIFETIME.as_secs(),
            8 * 60 * 60,
            "la vida de la sesión debe ser de 8 horas"
        );

        let token = sesion();
        let claims = verify(&secret(), &token).expect("el JWT emitido debe verificarse");
        let cookie = session_cookie(token, REDIRECT_HTTP).expect("la cookie se construye");

        let max_age = cookie
            .max_age()
            .map(|duracion| duracion.whole_seconds())
            .expect("la cookie debe llevar Max-Age");

        assert_eq!(
            max_age,
            SESSION_LIFETIME.as_secs() as i64,
            "el Max-Age de la cookie y la constante de vida deben coincidir"
        );
        assert_eq!(
            claims.exp - claims.iat,
            SESSION_LIFETIME.as_secs(),
            "el exp del JWT y la constante de vida deben coincidir"
        );
    }

    /// Escenario «Firma con otra clave»: una cookie firmada con una clave
    /// distinta de `PODPOT_SECRET` no se acepta.
    #[test]
    fn una_cookie_firmada_con_otra_clave_no_se_acepta() {
        let error = verify(&SecretString::new(OTRO_SECRET.to_owned()), &sesion())
            .expect_err("una clave distinta de PODPOT_SECRET no puede validar la sesión");

        assert!(
            matches!(error, AppError::Unauthorized(_)),
            "el rechazo debe ser un 401 tipado: {error}"
        );
    }

    /// Escenario «Sesión caducada»: un `exp` en el pasado no se acepta.
    #[test]
    fn una_cookie_caducada_no_se_acepta() {
        let error = verify(
            &secret(),
            &firmar_con_caducidad(jsonwebtoken::get_current_timestamp() - 60 * 60),
        )
        .expect_err("una sesión caducada no se acepta");

        assert!(
            matches!(error, AppError::Unauthorized(_)),
            "el rechazo debe ser un 401 tipado: {error}"
        );
    }

    /// El candado de `leeway = 0`: un `exp` treinta segundos atrás lo rechaza el
    /// margen cero y lo **aceptaría** el de cortesía que trae `Validation` por
    /// defecto —sesenta segundos—, de modo que revertir esa decisión pone esta
    /// prueba en rojo.
    #[test]
    fn una_cookie_con_el_exp_apenas_pasado_no_se_acepta() {
        let error = verify(
            &secret(),
            &firmar_con_caducidad(jsonwebtoken::get_current_timestamp() - 30),
        )
        .expect_err("una sesión caducada hace 30 segundos no se acepta");

        assert!(
            matches!(error, AppError::Unauthorized(_)),
            "el rechazo debe ser un 401 tipado: {error}"
        );
    }

    /// Escenario «Sesión manipulada»: alterar el contenido sin volver a firmarlo
    /// no puede colar.
    #[test]
    fn una_cookie_manipulada_no_se_acepta() {
        let error = verify(&secret(), &manipular(&sesion()))
            .expect_err("alterar el contenido sin volver a firmarlo no puede colar");

        assert!(
            matches!(error, AppError::Unauthorized(_)),
            "el rechazo debe ser un 401 tipado: {error}"
        );
    }

    /// Claims con la caducidad indicada, para probar la ventana de renovación.
    fn claims_con_caducidad(exp: u64) -> Claims {
        Claims {
            sub: SUB.to_owned(),
            email: EMAIL.to_owned(),
            iat: exp.saturating_sub(SESSION_LIFETIME.as_secs()),
            exp,
        }
    }

    /// Escenario «Fuera de la ventana»: con más de dos horas de vida no se renueva.
    #[test]
    fn fuera_de_la_ventana_no_se_renueva() {
        let ahora = 1_000_000;
        let claims = claims_con_caducidad(ahora + RENEWAL_WINDOW.as_secs() + 1);

        assert!(
            !should_renew(&claims, ahora),
            "una sesión con más de dos horas de vida no se renueva"
        );
    }

    /// Escenario «Borde de la ventana»: exactamente dos horas de vida sí renueva.
    #[test]
    fn en_el_borde_de_la_ventana_se_renueva() {
        let ahora = 1_000_000;
        let claims = claims_con_caducidad(ahora + RENEWAL_WINDOW.as_secs());

        assert!(
            should_renew(&claims, ahora),
            "el borde de la ventana también renueva"
        );
    }

    /// Escenario «Dentro de la ventana»: con menos de dos horas de vida se renueva.
    #[test]
    fn dentro_de_la_ventana_se_renueva() {
        let ahora = 1_000_000;
        let claims = claims_con_caducidad(ahora + RENEWAL_WINDOW.as_secs() - 1);

        assert!(
            should_renew(&claims, ahora),
            "una sesión a menos de dos horas de caducar se renueva"
        );
    }

    /// Escenario «Token caducado»: un `exp` ya pasado no se renueva, aunque caiga
    /// dentro de la ventana aritmética.
    #[test]
    fn un_token_caducado_no_se_renueva() {
        let ahora = 1_000_000;
        let claims = claims_con_caducidad(ahora - 1);

        assert!(
            !should_renew(&claims, ahora),
            "un token caducado no se renueva: no hay sesión que alargar"
        );
    }

    /// Escenario «Cookie renovada sobre `https`»: el token reemitido se envuelve
    /// con el esquema del retorno y la cookie resultante viaja `Secure`.
    #[test]
    fn la_cookie_renovada_respeta_el_esquema_https() {
        let ahora = 1_000_000;
        let token = renew(&secret(), &claims_con_caducidad(ahora + 60))
            .expect("la renovación no puede fallar");

        let cookie = session_cookie(token, REDIRECT_HTTPS).expect("la cookie se construye");

        assert_eq!(
            cookie.secure(),
            Some(true),
            "con retorno https la cookie renovada debe ir Secure"
        );
    }

    /// Escenario «Renovación»: conserva `sub` y `email` y alarga el `exp`.
    #[test]
    fn la_renovacion_conserva_la_identidad_y_alarga_la_vida() {
        let ahora = jsonwebtoken::get_current_timestamp();
        let original = claims_con_caducidad(ahora + 60);

        let token = renew(&secret(), &original).expect("la renovación no puede fallar");
        let renovada = verify(&secret(), &token).expect("el JWT renovado debe verificarse");

        assert_eq!(renovada.sub, original.sub, "la renovación conserva el sub");
        assert_eq!(
            renovada.email, original.email,
            "la renovación conserva el email"
        );
        assert!(
            renovada.exp > original.exp,
            "la renovación tiene que alargar la vida: {} no es mayor que {}",
            renovada.exp,
            original.exp
        );
    }
}
