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

/// Identidad de la sesión: exactamente `sub`, `email`, `iat`, `exp` y `logout_epoch`.
///
/// La estructura es la que sella la superficie: con esos cinco campos y
/// `deny_unknown_fields`, un token que traiga cualquier claim de más —un rol,
/// por ejemplo— no se puede deserializar y por tanto no se acepta. No hay que
/// acordarse de rechazarlo: no cabe.
///
/// `logout_epoch` indica cuándo se cerró la sesión desde el servidor. Si es
/// `Some(epoch)` y `epoch >= ahora`, ninguna petición posterior con este token
/// se renueva —el *gate* contesta `401` con borrado de cookie. Es el cierre de
/// la carrera logout↔refresh: un logout pone el epoch muy futuro para que todas
/// las peticiones en vuelo lo detecten; si es `None` o `0`, funciona como siempre.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    /// Identificador del usuario en el proveedor de identidad.
    pub sub: String,
    /// Correo del usuario, tomado del ID token ya verificado.
    pub email: String,
    /// Identificador opcional del usuario en el proveedor.
    pub jti: Option<String>,
    /// Momento de emisión, en segundos desde la época.
    pub iat: u64,
    /// Momento de caducidad, en segundos desde la época.
    pub exp: u64,
    /// Cuando la sesión fue revocada por el servidor (`0` si nunca revocada).
    ///
    /// Se escribe en el `logout` con una hora muy futura (día) para que
    /// cualquier petición en vuelo la detecte. Las sesiones emitidas normales
    /// tienen `logout_epoch = 0`: se considera expirado y no bloquea renovación.
    #[serde(default)]
    pub logout_epoch: u64,
}

/// Emite el JWT de sesión firmado con `HS256` y `secret`.
///
/// Devuelve el token, no la cookie: quien lo llame decide si además lo envuelve
/// en una cabecera `Set-Cookie`. La sesión sale **sin `jti`**: es el atajo de los
/// caminos que no asocian un `refresh_token` del proveedor. El callback usa
/// [`issue_con_jti`] para emitir la sesión atada a su `refresh_token`.
pub fn issue(secret: &SecretString, sub: String, email: String) -> Result<String, AppError> {
    issue_con_jti(secret, sub, email, None)
}

/// Emite el JWT de sesión con un `jti` opcional.
///
/// El `jti` ata la sesión al `refresh_token` del proveedor que el callback
/// guarda en memoria: la renovación lo necesita para volver a pedir el ID token
/// y para localizar el token que hay que rotar o descartar. Las sesiones sin
/// `refresh_token` se emiten igualmente con `jti` —el callback siempre pone
/// uno—, de modo que toda sesión de producción tiene con qué identificarse; el
/// `None` queda para la emisión sin refresco y para las pruebas.
pub fn issue_con_jti(
    secret: &SecretString,
    sub: String,
    email: String,
    jti: Option<String>,
) -> Result<String, AppError> {
    let emitido = get_current_timestamp();
    let claims = Claims {
        sub,
        email,
        jti,
        iat: emitido,
        exp: emitido + SESSION_LIFETIME.as_secs(),
        // Nueva sesión: nunca revocada por el servidor.
        logout_epoch: 0,
    };

    firmar(secret, &claims)
}

/// Versión interna de [`issue_con_jti`] que fija el `logout_epoch` a voluntad.
///
/// Se usa por [`renew`] para conservar el epoch cuando el servidor ya marcó la
/// sesión como revocada y no quiere que las peticiones subsiguientes se renueven
/// ignorando ese marcador.
fn issue_con_jti_with_logout(
    secret: &SecretString,
    sub: String,
    email: String,
    jti: Option<String>,
    logout_epoch: u64,
) -> Result<String, AppError> {
    let emitido = get_current_timestamp();
    let claims = Claims {
        sub,
        email,
        jti,
        iat: emitido,
        exp: emitido + SESSION_LIFETIME.as_secs(),
        logout_epoch,
    };

    firmar(secret, &claims)
}

/// Firma unos `claims` de sesión con `HS256` y `secret`.
///
/// Es el único sitio donde la clave de firma sale por `expose`: emitir, renovar
/// y el atajo de las pruebas comparten esta firma en vez de repetir la clave.
fn firmar(secret: &SecretString, claims: &Claims) -> Result<String, AppError> {
    let cabecera = Header::new(Algorithm::HS256);
    let clave = EncodingKey::from_secret(secret.expose().as_bytes());

    encode(&cabecera, claims, &clave).map_err(|error| {
        tracing::error!(%error, "no se pudo firmar el JWT de sesión");
        AppError::Internal("no se pudo emitir la sesión".to_owned())
    })
}

/// Un `jti` nuevo para la sesión: 16 bytes aleatorios en forma de UUID v4.
///
/// Es el identificador con el que el callback asocia el `refresh_token` del
/// proveedor a la sesión emitida. Se construye a mano —versión 4, variante
/// RFC 4122— y no con una caja de `uuid`: `rand` ya está en el árbol de
/// dependencias y el formato es una decena de líneas. Es opaco: solo tiene que
/// ser único y no adivinable, no llevar información.
pub fn nuevo_jti() -> String {
    use rand::Rng as _;

    let mut bytes = [0u8; 16];
    rand::thread_rng().fill(&mut bytes);
    // Los bits de versión (4) y variante (RFC 4122), para que sea un UUID v4
    // bien formado y no solo dieciséis bytes con guiones.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();

    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32],
    )
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
///
/// Si el servidor marcó esta sesión como revocada (`logout_epoch > 0`) y ese
/// momento todavía no llegó, devuelve `false` independientemente de si la
/// ventana lo permitiría: es el cierre de la carrera logout↔refresh. El valor
/// cero significa «sesión emitida, nunca cerrada» y no bloquea nada.
///
/// Nota: el cierre de carrera principal ocurre en el *gate* vía
/// `esta_revocado_por_logout`; `should_renew` es una verificación de defensa
/// en profundidad por si se llama sin pasar por el gate.
pub fn should_renew(claims: &Claims, ahora: u64) -> bool {
    // Bloqueo por revocación del servidor: logout_epoch > 0 marca el cierre
    // y >= ahora indica que todavía está vigente.
    if claims.logout_epoch > 0 && claims.logout_epoch >= ahora {
        return false;
    }

    claims.exp > ahora && claims.exp <= ahora.saturating_add(RENEWAL_WINDOW.as_secs())
}

/// Reemite la sesión con la misma identidad y una vida nueva.
///
/// Conserva `sub`, `email` **y el `jti`** —el `jti` ata la sesión renovada al
/// mismo `refresh_token`, de modo que la renovación siguiente sigue teniendo con
/// qué refrescar— y vuelve a firmar con `issue_con_jti`, así que el token nuevo
/// tiene el `iat` y el `exp` de una emisión recién hecha. No toca la cookie:
/// quien la envuelva decide si además la manda en `Set-Cookie`.
///
/// Preserva también el `logout_epoch`: si el servidor marcó esta sesión como
/// revocada mientras se ejecutaba la renovación, el token nuevo lo mantiene para
/// que las peticiones subsiguientes detecten el cierre antes de renovar sin mirar
/// el epoch (que ahora apunta a un momento futuro pero la petición ya superó ese
/// instante).
pub fn renew(secret: &SecretString, claims: &Claims) -> Result<String, AppError> {
    let logout_epoch = claims.logout_epoch;

    // issue_con_jti fija logout_epoch = 0 por defecto; pasarlo por aquí conserva
    // el valor original cuando el servidor ya marcó la sesión como cerrada.
    issue_con_jti_with_logout(
        secret,
        claims.sub.clone(),
        claims.email.clone(),
        claims.jti.clone(),
        logout_epoch,
    )
}

/// Emite un JWT de sesión con una caducidad dada, para las pruebas.
///
/// `issue` fija la vida de la sesión a [`SESSION_LIFETIME`]; este atajo permite
/// fabricar tokens con el `exp` que cada escenario necesita —recién emitido, a
/// punto de caducar o ya caducado— sin tocar la constante ni depender del reloj,
/// y con el `jti` que la prueba quiera para ejercer la renovación con refresco.
/// Firma con `HS256` y el mismo secreto que `issue`, así que `verify` lo acepta
/// igual que un token de producción.
#[cfg(test)]
pub(crate) fn issue_con_caducidad(
    secret: &SecretString,
    sub: String,
    email: String,
    jti: Option<String>,
    exp: u64,
) -> Result<String, AppError> {
    let claims = Claims {
        sub,
        email,
        jti,
        iat: get_current_timestamp(),
        exp,
        // Las pruebas emiten sin revocación del servidor.
        logout_epoch: 0,
    };

    firmar(secret, &claims)
}

/// La cookie de borrado de la sesión: la misma cookie, con el mismo `Path` y
/// `Max-Age=0`.
///
/// Se construye con `make_removal` en vez de con `CookieJar::remove` a
/// propósito: `remove` solo emite la `Set-Cookie` si la petición traía esa
/// cookie, y ni el `logout` ni el cierre por revocación del proveedor pueden
/// depender de que la cookie siga ahí. El borrado define la cookie en un único
/// sitio —[`SESSION_COOKIE_NAME`] con `Path=/`— para que el `logout` y el `401`
/// por revocación manden exactamente la misma.
pub fn cookie_de_borrado() -> Cookie<'static> {
    let mut cookie = Cookie::build((SESSION_COOKIE_NAME, "")).path("/").build();
    cookie.make_removal();
    cookie
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
            jti: None,
            iat: exp - SESSION_LIFETIME.as_secs(),
            exp,
            logout_epoch: 0,
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

    /// Escenario «Payload exacto»: la sesión lleva exactamente los cinco claims
    /// y ninguno de rol, así que no hay dónde meter claims del proveedor.
    #[test]
    fn el_payload_de_la_sesion_son_exactamente_los_cinco_claims() {
        let token = sesion();

        assert_eq!(
            claves_del_payload(&token),
            ["email", "exp", "iat", "jti", "logout_epoch", "sub"],
            "los claims de la sesión deben ser exactamente los seis: {token}"
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
            "logout_epoch": 0,
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
            jti: None,
            iat: exp.saturating_sub(SESSION_LIFETIME.as_secs()),
            exp,
            logout_epoch: 0,
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

    // ── logout_epoch ────────────────────────────────────────────────────

    /// Helper para construir claims con un `logout_epoch` arbitrario.
    ///
    /// La estructura de producción usa `u64`: cero significa «nunca revocada» y
    /// cualquier valor mayor es el instante en que el servidor cerró la sesión.
    fn claims_con_logout_epoch(exp: u64, logout_epoch: u64) -> Claims {
        Claims {
            sub: SUB.to_owned(),
            email: EMAIL.to_owned(),
            jti: None,
            iat: exp.saturating_sub(SESSION_LIFETIME.as_secs()),
            exp,
            logout_epoch,
        }
    }

    /// Escenario «Logout epoch activo» (`logout_epoch >= now`): no se renueva.
    #[test]
    fn logout_epoch_activo_bloquea_la_renovacion() {
        let ahora = 1_000_000;
        // logout_epoch 1 hora vista: todavía está activo.
        let claims = claims_con_logout_epoch(ahora + RENEWAL_WINDOW.as_secs() + 1, ahora + 3600);

        assert!(
            !should_renew(&claims, ahora),
            "un logout_epoch futuro dentro de la ventana bloquea la renovación"
        );
    }

    /// Escenario «Logout epoch expirado» (`logout_epoch < now`, > 0): sí se renueva
    /// si la sesión sigue dentro de la ventana.
    #[test]
    fn logout_epoch_expirado_permite_la_renovacion() {
        let ahora = 1_000_000;
        // logout_epoch hace 2 horas: ya expiró, pero es > 0.
        let claims = claims_con_logout_epoch(ahora + RENEWAL_WINDOW.as_secs(), ahora - 7200);

        assert!(
            should_renew(&claims, ahora),
            "cuando el logout_epoch expiró y la ventana lo permite, se renueva"
        );
    }

    /// Escenario «Sin logout_epoch» (`logout_epoch == 0`): se renueva dentro de la
    /// ventana como siempre.
    #[test]
    fn sin_logout_epoch_se_renuvia_normalmente() {
        let ahora = 1_000_000;
        let claims = claims_con_logout_epoch(ahora + RENEWAL_WINDOW.as_secs(), 0);

        assert!(
            should_renew(&claims, ahora),
            "con logout_epoch == 0, funciona como siempre"
        );
    }

    /// Escenario «Logout epoch exactamente ahora»: `logout_epoch >= now`, así que
    /// no se renueva (borde).
    #[test]
    fn logout_epoch_exactamente_now_bloquea() {
        let ahora = 1_000_000;
        let claims = claims_con_logout_epoch(ahora + RENEWAL_WINDOW.as_secs(), ahora);

        assert!(
            !should_renew(&claims, ahora),
            "logout_epoch == ahora también bloquea (borde)"
        );
    }

    /// Escenario «Renovación conserva `logout_epoch`»: cuando la sesión emitida lleva
    /// `logout_epoch > 0` (revocada por el servidor), la renovación mantiene ese
    /// valor para que las peticiones subsiguientes sigan detectando el cierre.
    #[test]
    fn la_renovacion_conserva_logout_epoch_revocado() {
        let ahora = jsonwebtoken::get_current_timestamp();
        let logout_at = ahora + 86400; // revocada mañana.
        let claims = claims_con_logout_epoch(ahora + 60, logout_at);

        let token = renew(&secret(), &claims).expect("la renovación no puede fallar");
        let renovada = verify(&secret(), &token).expect("el JWT renovado debe verificarse");

        assert_eq!(
            renovada.logout_epoch, logout_at,
            "la renovación conserva el logout_epoch revocado"
        );
    }

    /// Escenario «Renovación sin revocación» (`logout_epoch == 0`): permanece `0`.
    #[test]
    fn la_renovacion_mantiene_logout_epoch_cero() {
        let ahora = jsonwebtoken::get_current_timestamp();
        let claims = claims_con_logout_epoch(ahora + 60, 0);

        let token = renew(&secret(), &claims).expect("la renovación no puede fallar");
        let renovada = verify(&secret(), &token).expect("el JWT renovado debe verificarse");

        assert_eq!(
            renovada.logout_epoch, 0,
            "si no había revocación, la renovación mantiene logout_epoch en 0"
        );
    }
}
