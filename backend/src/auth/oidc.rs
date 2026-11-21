//! El flujo OIDC: descubrimiento del proveedor, URL de autorización,
//! `state`/`nonce` y, ya en el retorno, el canje del código y la verificación
//! del ID token.
//!
//! Aquí vive todo el flujo salvo el HTTP: el descubrimiento que el `login`
//! necesita para redirigir al proveedor, y lo que el `callback` necesita para
//! convertir el retorno en una identidad verificada. El flujo se escribe a mano
//! —patrón de kesto— y no con `openidconnect`: la caja no está en la caché de
//! Cargo de esta máquina y el repositorio tiene que poder compilarse sin conexión.
//!
//! El módulo cachea las **dos** respuestas del proveedor que no cambian de un
//! login a otro: el documento de descubrimiento ([`discovery_cached`]) y el
//! conjunto de claves ([`jwks_cached`]). El segundo **caduca** —la entrada guarda
//! su instante de lectura y se reutiliza a lo sumo [`JWKS_TTL`]— y además se
//! refresca de forma condicional: si el ID token viene firmado con un `kid` que el
//! conjunto cacheado no publica, [`verificar_id_token`] pide un refresco forzado
//! antes de rendirse. Es un ahorro —un `jwks_uri` por TTL en vez de uno por
//! callback— con un precio acotado y explícito: el rato del TTL es la ventana en
//! la que una clave retirada seguiría sirviendo aquí.

use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode_header};
use serde::Deserialize;

use crate::auth::state::AuthState;
use crate::config::OidcSettings;
use crate::error::AppError;

/// Ruta del documento de descubrimiento, relativa al emisor.
const RUTA_DESCUBRIMIENTO: &str = "/.well-known/openid-configuration";

/// Tiempo máximo que se espera al proveedor en el descubrimiento.
///
/// Un proveedor que acepta la conexión y no contesta no puede colgar el login
/// para siempre: el primer login tiene que fallar en un tiempo acotado en vez de
/// quedarse esperando.
pub const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Scopes que se piden al proveedor.
///
/// Incluye `openid` —sin él no hay ID token— y `email`, porque el `email` de la
/// sesión es un claim del ID token y la mayoría de los proveedores solo lo
/// publican con ese scope.
pub const SCOPE: &str = "openid email";

/// Longitud del `state` y del `nonce`: 32 caracteres aleatorios.
pub const TOKEN_LENGTH: usize = 32;

/// Margen de cortesía, en segundos, para el `iat` del ID token.
///
/// El `exp` de la sesión se valida con margen cero porque los dos extremos son
/// podpot, y el `exp` del ID token por otro motivo: va una hora visto, así que un
/// desfase de segundos no lo alcanza. El `iat` del ID token es el caso distinto:
/// lo firma el reloj del proveedor y lo compara el de podpot —dos relojes—, y va
/// redondeado al segundo, de modo que sin margen un desfase de un segundo tumba
/// un login por lo demás en regla con un `401` genérico. Sesenta segundos es la
/// tolerancia habitual de un JWT y no alarga la vida de nada: solo tolera el
/// futuro, nunca estira la vigencia.
pub const MARGEN_DE_RELOJ: u64 = 60;

/// Documento de descubrimiento, con lo que de verdad se usa.
///
/// `userinfo_endpoint` **no** se declara a propósito: el ID token verificado
/// sobra y no se llama a `userinfo`, así que exigirlo rompería proveedores que
/// no lo publican. Serde ignora las claves que no estén aquí, de modo que el
/// resto del documento no molesta.
#[derive(Debug, Clone, Deserialize)]
pub struct Discovery {
    /// Endpoint al que se redirige al usuario para autorizar.
    pub authorization_endpoint: String,
    /// Endpoint del canje del código por los tokens.
    pub token_endpoint: String,
    /// Endpoint del JWKS contra el que se verifica el ID token.
    pub jwks_uri: String,
}

/// URL del documento de descubrimiento del emisor dado.
///
/// El `/` final del emisor se recorta para no construir `//.well-known/...`.
pub fn discovery_url(issuer: &str) -> String {
    format!("{}{RUTA_DESCUBRIMIENTO}", issuer.trim_end_matches('/'))
}

/// Pide el documento de descubrimiento al proveedor.
///
/// Cualquier desenlace que no sea un documento válido —no contestar, contestar
/// un error o contestar algo que no es el documento— es un
/// [`AppError::BadGateway`]: el fallo es de un servicio de terceros, no de
/// podpot. La causa viaja en el error para que acabe en el registro, nunca en el
/// cuerpo de la respuesta.
pub async fn fetch_discovery(settings: &OidcSettings) -> Result<Discovery, AppError> {
    let url = discovery_url(&settings.issuer);

    // El cliente se construye aquí y no se guarda en el estado: el
    // descubrimiento queda cacheado, así que esto ocurre una vez. El `timeout`
    // acota la espera para que un proveedor que acepta la conexión y no contesta
    // no cuelgue el login.
    let cliente = reqwest::Client::builder()
        .timeout(DISCOVERY_TIMEOUT)
        .build()
        .map_err(|error| {
            AppError::BadGateway(format!(
                "no se pudo preparar el cliente del descubrimiento: {error}"
            ))
        })?;

    let respuesta = cliente.get(&url).send().await.map_err(|error| {
        AppError::BadGateway(format!(
            "el proveedor de identidad no respondió al descubrimiento: {error}"
        ))
    })?;

    if !respuesta.status().is_success() {
        return Err(AppError::BadGateway(format!(
            "el proveedor de identidad contestó {} al descubrimiento",
            respuesta.status()
        )));
    }

    respuesta.json::<Discovery>().await.map_err(|error| {
        AppError::BadGateway(format!(
            "el documento de descubrimiento no es válido: {error}"
        ))
    })
}

/// Devuelve el descubrimiento cacheado o lo pide y lo deja cacheado.
///
/// La caché vive en el estado y es segura entre tareas: el servidor es
/// concurrente y varios logins pueden mirarla a la vez. Se lee primero bajo un
/// candado de lectura y solo se escribe la primera vez que llega un resultado,
/// de modo que el segundo login —y el siguiente— no vuelven a pedir el
/// documento. Dos logins simultáneos sobre una caché vacía podrían pedirlo los
/// dos; el requisito es no repetirlo en cada intento de entrada, no una
/// exclusión mutua sobre la primera petición.
pub async fn discovery_cached(auth: &AuthState) -> Result<Discovery, AppError> {
    if let Some(discovery) = auth.discovery.read().await.clone() {
        return Ok(discovery);
    }

    let discovery = fetch_discovery(auth.settings.as_ref()).await?;

    *auth.discovery.write().await = Some(discovery.clone());

    Ok(discovery)
}

/// Una petición de autorización ya construida.
#[derive(Debug, Clone)]
pub struct AuthorizationRequest {
    /// URL completa a la que redirigir al usuario.
    pub url: String,
    /// `state` emitido: la clave con la que el callback consumirá este login.
    pub state: String,
    /// `nonce` emitido: viaja al proveedor para compararlo con el del ID token.
    pub nonce: String,
}

/// Construye la URL de autorización y genera el `state` y el `nonce`.
///
/// Los valores añadidos a la URL van codificados. El `state` y el `nonce` son
/// de 32 caracteres aleatorios y distintos en cada petición.
pub fn build_authorization_url(
    settings: &OidcSettings,
    discovery: &Discovery,
) -> AuthorizationRequest {
    let state = token_aleatorio();
    let nonce = token_aleatorio();

    let parametros = [
        ("response_type", "code"),
        ("client_id", settings.client_id.as_str()),
        ("redirect_uri", settings.redirect_url.as_str()),
        ("scope", SCOPE),
        ("state", state.as_str()),
        ("nonce", nonce.as_str()),
    ];

    // Cada valor va codificado: el `redirect_uri` lleva `:` y `/`, que sin
    // codificar romperían la URL; y aunque hoy el `state` y el `nonce` sean
    // alfanuméricos, codificarlos es lo que hace que el flujo siga siendo
    // correcto si algún día dejan de serlo.
    let consulta = parametros
        .iter()
        .map(|(clave, valor)| {
            format!(
                "{}={}",
                urlencoding::encode(clave),
                urlencoding::encode(valor)
            )
        })
        .collect::<Vec<_>>()
        .join("&");

    // El `authorization_endpoint` lo publica el propio proveedor, así que se
    // confía en él. El separador se elige según lo que ya traiga: un `?` fijo
    // produciría dos interrogantes si el endpoint ya viniera con consulta.
    let separador = if discovery.authorization_endpoint.contains('?') {
        '&'
    } else {
        '?'
    };

    AuthorizationRequest {
        url: format!("{}{separador}{consulta}", discovery.authorization_endpoint),
        state,
        nonce,
    }
}

/// 32 caracteres aleatorios alfanuméricos.
///
/// `rand` está en 0.8.8, así que la API es `thread_rng()` y
/// `distributions::Alphanumeric`; `rand::rng()` y `distr::Alphanumeric` son de
/// 0.9/0.10 y no compilan aquí.
fn token_aleatorio() -> String {
    use rand::Rng;

    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(TOKEN_LENGTH)
        .map(char::from)
        .collect()
}

/// Tiempo máximo que se espera al proveedor en el canje y en el JWKS.
///
/// El mismo criterio que en el descubrimiento: un proveedor que acepta la
/// conexión y no contesta no puede colgar el callback para siempre.
pub const INTERCAMBIO_TIMEOUT: Duration = Duration::from_secs(10);

/// Respuesta del `token_endpoint`, con lo que de verdad se usa.
///
/// Sin `Debug` a propósito: el `id_token` es una credencial del proveedor y un
/// `?respuesta` en un `debug!` la publicaría. Solo se lee el campo que hace
/// falta; el `access_token` y el resto del cuerpo se ignoran —y, al no tocarlos,
/// no hay forma de que acaben en el registro—.
#[derive(Deserialize)]
struct TokenResponse {
    /// ID token firmado que hay que verificar antes de emitir sesión.
    id_token: String,
}

/// Claims del ID token que se releen después de que [`Validation`] compruebe
/// firma, `iss`, `aud` y `exp`.
///
/// Solo `sub`, `email`, `nonce` e `iat`: `iss` y `aud` los valida la propia
/// biblioteca sobre el token, así que releerlos aquí sería duplicar la
/// comprobación. `nonce` e `iat` sí se miran aquí porque `jsonwebtoken` no los
/// valida. `exp` **no se declara a propósito**: quien lo exige es
/// `required_spec_claims`, que `jsonwebtoken` comprueba sobre el mapa **crudo**
/// del token —no sobre este struct—, de modo que un token sin `exp` se rechaza
/// sin que aquí haya un campo.
#[derive(Deserialize)]
struct IdTokenClaims {
    /// Usuario.
    sub: String,
    /// Correo.
    email: String,
    /// `nonce` devuelto, que se compara con el emitido en el login.
    nonce: String,
    /// Emisión, en segundos desde la época.
    iat: u64,
}

/// Identidad tomada del ID token ya verificado.
#[derive(Debug, Clone)]
pub struct IdentidadVerificada {
    /// Identificador del usuario en el proveedor.
    pub sub: String,
    /// Correo del usuario.
    pub email: String,
}

/// Mensaje único con el que se rechaza un ID token que no supera la verificación.
///
/// El cliente no necesita saber qué comprobación falló; el detalle va al
/// registro. Es el mismo criterio que la sesión: `401` con un motivo genérico.
const ID_TOKEN_INVALIDO: &str = "el ID token no es válido";

/// Canjea el código de autorización en el `token_endpoint`.
///
/// Devuelve el ID token **sin verificar**: verificar es el paso siguiente y no
/// se mezcla con el canje. El `client_secret` viaja en el **cuerpo** del POST
/// —`application/x-www-form-urlencoded`—, nunca en la cadena de consulta, y no
/// se interpola en ningún registro.
pub async fn canjear_codigo(
    settings: &OidcSettings,
    discovery: &Discovery,
    code: &str,
) -> Result<String, AppError> {
    let cliente = reqwest::Client::builder()
        .timeout(INTERCAMBIO_TIMEOUT)
        .build()
        .map_err(|error| {
            AppError::BadGateway(format!("no se pudo preparar el cliente del canje: {error}"))
        })?;

    let formulario = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", settings.redirect_url.as_str()),
        ("client_id", settings.client_id.as_str()),
        ("client_secret", settings.client_secret.expose()),
    ];

    let respuesta = cliente
        .post(&discovery.token_endpoint)
        .form(&formulario)
        .send()
        .await
        .map_err(|error| {
            AppError::BadGateway(format!(
                "el proveedor de identidad no respondió al canje del código: {error}"
            ))
        })?;

    if !respuesta.status().is_success() {
        return Err(AppError::BadGateway(format!(
            "el proveedor de identidad contestó {} al canje del código",
            respuesta.status()
        )));
    }

    respuesta
        .json::<TokenResponse>()
        .await
        .map(|datos| datos.id_token)
        .map_err(|error| {
            AppError::BadGateway(format!("la respuesta del canje no es válida: {error}"))
        })
}

/// Frescura máxima del JWKS cacheado: 10 minutos.
///
/// La caché del conjunto de claves se salta el requisito de pedir el `jwks_uri` en
/// cada callback, y ese ahorro **tiene un precio**: durante este rato podpot sigue
/// verificando ID tokens con una clave que el proveedor pudo haber retirado. El TTL
/// es, por tanto, el techo de esa exposición, no un número de conveniencia.
///
/// Diez minutos: el mismo orden que el `state` de un login ([`STATE_TTL`] son cinco)
/// y muy por debajo de la hora que suele vivir un ID token, de modo que la rotación
/// normal —clave nueva, `kid` nuevo— la cubre el refresco condicional y este TTL solo
/// acota el caso que el refresco **no** puede ver: una clave retirada cuyo `kid` el
/// conjunto cacheado sigue publicando. Ni más corto —un TTL de segundos volvería a
/// pagar una petición por callback, que es justo lo que se viene a quitar— ni mucho
/// más largo, porque ese rato es exactamente la ventana en la que una clave retirada
/// sigue sirviendo aquí.
///
/// [`STATE_TTL`]: crate::auth::state::STATE_TTL
pub const JWKS_TTL: Duration = Duration::from_secs(10 * 60);

/// Un JWKS cacheado: el conjunto y el instante en que se leyó del proveedor.
///
/// El instante va **dentro** de la entrada, y no en una marca aparte, por la misma
/// razón que el `nonce` viaja junto al `state`: la caducidad y el conjunto que
/// caduca no pueden separarse sin que una lectura los desempareje. La frescura se
/// mide contra [`JWKS_TTL`] con [`JwksCacheado::esta_fresco`].
#[derive(Debug, Clone)]
pub struct JwksCacheado {
    /// El conjunto de claves tal cual lo publicó el `jwks_uri`.
    pub conjunto: JwkSet,
    /// Instante de lectura, para aplicar el TTL.
    pub leido_en: Instant,
}

impl JwksCacheado {
    /// ¿Sigue fresco el conjunto a fecha de `ahora`?
    ///
    /// La fecha entra por parámetro —y no se lee del reloj aquí— por el mismo
    /// motivo que el `ahora` de `PendingLogin::expirado_en`: deja la comparación
    /// determinista en las pruebas. El reloj monotónico de `Instant` mide desde el
    /// arranque de la máquina, así que un test que quisiera caducar una entrada
    /// restándole el TTL al reloj tendría que dar por hecho que el sistema lleva
    /// más de un TTL en marcha —falso en un runner recién arrancado—; con la fecha
    /// inyectada, el test decide cuándo se evalúa la caché y no depende del uptime.
    /// `saturating_duration_since` cierra el caso de una fecha anterior a la
    /// lectura, que no puede darse en producción y que aquí cuenta como fresca.
    fn esta_fresco(&self, ahora: Instant) -> bool {
        ahora.saturating_duration_since(self.leido_en) < JWKS_TTL
    }
}

/// Pide el JWKS contra el que se verifica el ID token.
///
/// Un JWKS que no se puede leer es un fallo del proveedor, no de podpot: la
/// causa viaja en un [`AppError::BadGateway`] y acaba en el registro, nunca en
/// el cuerpo.
async fn fetch_jwks(url: &str) -> Result<JwkSet, AppError> {
    let cliente = reqwest::Client::builder()
        .timeout(INTERCAMBIO_TIMEOUT)
        .build()
        .map_err(|error| {
            AppError::BadGateway(format!("no se pudo preparar el cliente del JWKS: {error}"))
        })?;

    let respuesta = cliente.get(url).send().await.map_err(|error| {
        AppError::BadGateway(format!(
            "el proveedor de identidad no respondió al JWKS: {error}"
        ))
    })?;

    if !respuesta.status().is_success() {
        return Err(AppError::BadGateway(format!(
            "el proveedor de identidad contestó {} al JWKS",
            respuesta.status()
        )));
    }

    respuesta
        .json::<JwkSet>()
        .await
        .map_err(|error| AppError::BadGateway(format!("el JWKS no es válido: {error}")))
}

/// Devuelve el JWKS contra el que verificar el ID token, cacheado con frescura
/// acotada por [`JWKS_TTL`].
///
/// El mismo patrón que [`discovery_cached`], con una diferencia que manda: aquí el
/// conjunto **caduca**. La entrada guarda el instante de lectura —[`JwksCacheado`]— y
/// solo se reutiliza mientras [`JwksCacheado::esta_fresco`] lo diga a fecha de `ahora`;
/// pasado el TTL, el siguiente callback vuelve a pedir el `jwks_uri` y sustituye la
/// entrada por lo recién leído.
///
/// La frescura se consulta primero bajo un candado de **lectura**, que es el camino
/// barato —el de todos los callbacks dentro del TTL, que se atienden a la vez sin
/// estorbarse—; solo cuando no hay nada fresco se pasa al candado de escritura, que
/// es donde vive [`refrescar_jwks`].
///
/// Aquí solo se refresca por **caducidad**. El otro motivo para ir al `jwks_uri` —que
/// el ID token venga firmado con un `kid` que el conjunto cacheado no publica— no se
/// decide aquí, porque depende del token y no del conjunto: lo detecta
/// [`verificar_id_token`], que en ese caso pide un refresco forzado
/// ([`refrescar_jwks`] con `forzar` a `true`) y reintenta la búsqueda.
pub async fn jwks_cached(auth: &AuthState) -> Result<JwkSet, AppError> {
    jwks_cached_en(auth, Instant::now()).await
}

/// [`jwks_cached`] con la fecha de evaluación explícita.
///
/// El reloj entra por parámetro para que la caducidad sea comprobable sin depender de
/// cuánto lleve la máquina en marcha: el camino de producción es el de arriba, que
/// pasa `Instant::now()`, mientras que las pruebas de la caducidad pasan una fecha
/// posterior al TTL sobre una entrada sembrada **fresca**. La composición —leer
/// primero, refrescar solo si no hay nada fresco— es la misma que la del camino
/// público, que es justamente lo que se quiere cubrir.
async fn jwks_cached_en(auth: &AuthState, ahora: Instant) -> Result<JwkSet, AppError> {
    if let Some(cacheado) = leer_jwks_fresco(auth, ahora).await {
        return Ok(cacheado);
    }

    refrescar_jwks(auth, false, ahora).await
}

/// Devuelve el conjunto cacheado si sigue fresco a fecha de `ahora`, y `None` si no
/// hay entrada o ya ha caducado.
///
/// Es el camino barato: basta un candado de **lectura**, así que los callbacks
/// dentro del TTL no se bloquean entre sí. La frescura la calcula
/// [`JwksCacheado::esta_fresco`] contra el instante de lectura de la propia entrada,
/// de modo que el conjunto y su caducidad no pueden desemparejarse.
async fn leer_jwks_fresco(auth: &AuthState, ahora: Instant) -> Option<JwkSet> {
    auth.jwks
        .read()
        .await
        .as_ref()
        .filter(|entrada| entrada.esta_fresco(ahora))
        .map(|entrada| entrada.conjunto.clone())
}

/// Pide el `jwks_uri` y deja el conjunto recién leído en la caché.
///
/// El candado de escritura se toma **antes** de ir a por el conjunto y se mantiene
/// durante la petición, en vez de soltarlo tras consultar la caché. Es lo que impide
/// que dos callbacks que caducan a la vez pidan el conjunto por duplicado y que el
/// segundo pise el resultado del primero: el que llega después espera al candado y ya
/// encuentra la entrada fresca. El precio es que los demás callbacks aguardan mientras
/// dura un refresco —y, en el peor caso, lo que tarde en vencer
/// [`INTERCAMBIO_TIMEOUT`]—; se acepta porque el refresco solo ocurre al caducar el TTL
/// o al rotar una clave, no en cada callback, y a cambio la caché nunca queda a medias
/// ni se pide dos veces para lo mismo.
///
/// Esa **deduplicación vale solo para `forzar` a `false`** —la caducidad—, el único
/// caso en que se vuelve a comprobar la frescura ya bajo el candado: si otra tarea
/// refrescó mientras se esperaba por él, su conjunto vale y no hay que pedir nada. Con
/// `forzar` a `true` —la rotación— esa comprobación se salta a propósito, porque el
/// motivo del refresco no es que la entrada haya caducado sino que no publica la clave
/// que el ID token pide: el conjunto puede estar fresco y seguir siendo el viejo. Por
/// eso, con `forzar` a `true`, **N callbacks concurrentes con un `kid` desconocido
/// pueden encadenar N peticiones** al `jwks_uri`, serializadas bajo el candado pero
/// todas de verdad. Se asume: el caso es raro —solo lo provoca una rotación que varios
/// callbacks pisan a la vez— y una guardia de «refresco en curso» no compensaría la
/// complejidad, porque el candado ya evita la carrera y una petición de más no rompe
/// nada.
///
/// Un fallo de la petición —o del descubrimiento— se propaga tal cual y **no se
/// cachea**: se devuelve antes de escribir, así que la entrada anterior se queda como
/// estaba. Un proveedor caído no envenena la caché con un conjunto vacío ni deja a
/// podpot sin el conjunto que ya tenía.
///
/// El conjunto se fecha con el `ahora` que entra por parámetro —la misma fecha con la
/// que se comprobó la frescura— y no con una lectura nueva del reloj: una sola fuente
/// de tiempo para decidir y para fechar evita que el TTL empiece a contar en un
/// instante distinto al que se usó para mirar la caché.
async fn refrescar_jwks(
    auth: &AuthState,
    forzar: bool,
    ahora: Instant,
) -> Result<JwkSet, AppError> {
    let mut cache = auth.jwks.write().await;

    if !forzar
        && let Some(entrada) = cache.as_ref()
        && entrada.esta_fresco(ahora)
    {
        return Ok(entrada.conjunto.clone());
    }

    let discovery = discovery_cached(auth).await?;
    let conjunto = fetch_jwks(&discovery.jwks_uri).await?;

    *cache = Some(JwksCacheado {
        conjunto: conjunto.clone(),
        leido_en: ahora,
    });

    Ok(conjunto)
}

/// Rechaza un ID token dejando la causa en el registro y un motivo genérico al
/// cliente.
fn rechazar_id_token(causa: &str) -> AppError {
    tracing::debug!(causa, "ID token rechazado");
    AppError::Unauthorized(ID_TOKEN_INVALIDO.to_owned())
}

/// Verifica el ID token y devuelve la identidad que lleva dentro.
///
/// Cinco comprobaciones: firma `RS256` contra las claves del `jwks_uri`, `iss`,
/// `aud`, vigencia de `exp` e `iat`, y `nonce` igual al emitido en el login.
/// Cualquier fallo rechaza con [`AppError::Unauthorized`]: no hay sesión que
/// emitir.
///
/// El conjunto de claves se resuelve por [`jwks_cached`] a partir del estado —y no
/// por el descubrimiento que el llamador tenga en la mano— porque el JWKS se cachea:
/// quien llama no tiene por qué saber de dónde sale el conjunto ni cuándo caduca.
///
/// Si el `kid` del ID token no está en el conjunto cacheado, se refresca **una sola
/// vez** y se reintenta: es lo que cubre una rotación de claves sin que el login
/// dependa de que el TTL haya vencido. Un refresco que falla no se convierte en
/// rechazo del token sino que se propaga tal cual —el `502` de [`AppError::BadGateway`]—,
/// porque el problema es del proveedor, no del ID token.
pub async fn verificar_id_token(
    auth: &AuthState,
    id_token: &str,
    nonce_esperado: &str,
) -> Result<IdentidadVerificada, AppError> {
    let jwks = jwks_cached(auth).await?;

    // El `kid` de la cabecera elige la clave del conjunto. Sin `kid`, o con uno
    // que el JWKS no publica, no hay clave con la que verificar.
    let cabecera = match decode_header(id_token) {
        Ok(cabecera) => cabecera,
        Err(error) => {
            return Err(rechazar_id_token(&format!(
                "la cabecera del ID token no es válida: {error}"
            )));
        }
    };
    let Some(kid) = cabecera.kid else {
        return Err(rechazar_id_token("el ID token no declara kid"));
    };

    // El conjunto cacheado se da por bueno hasta que caduque, pero una rotación de
    // claves no reinicia el TTL: el proveedor puede publicar una clave nueva con el
    // conjunto cacheado todavía fresco. Por eso, si el `kid` del token no está en lo
    // cacheado, se refresca **una sola vez** —saltándose la frescura, que es lo que
    // distingue este caso del de la caducidad— y se reintenta la búsqueda; si tampoco
    // así aparece, el rechazo es el de siempre. El refresco solo se intenta cuando el
    // `kid` falta, no en cada verificación, de modo que el camino normal no paga
    // ninguna petición de más.
    let jwk = match jwks.find(&kid) {
        Some(jwk) => jwk.clone(),
        None => match refrescar_jwks(auth, true, Instant::now()).await?.find(&kid) {
            Some(jwk) => jwk.clone(),
            None => {
                return Err(rechazar_id_token(
                    "el ID token va firmado con una clave que el jwks_uri no publica",
                ));
            }
        },
    };
    let clave = match DecodingKey::from_jwk(&jwk) {
        Ok(clave) => clave,
        Err(error) => {
            return Err(rechazar_id_token(&format!(
                "la clave del JWKS no es utilizable: {error}"
            )));
        }
    };

    // El algoritmo se fija a `RS256` y no se lee de la cabecera del token: es la
    // misma prevención de confusión de algoritmo que en la sesión. El `leeway`
    // por defecto —60 segundos— alargaría la vida de un token recién caducado, así
    // que se pone a cero. `iss` y `aud` son obligatorios: un token que no los
    // declare no vale.
    let mut validacion = Validation::new(Algorithm::RS256);
    validacion.leeway = 0;
    validacion.validate_aud = true;
    validacion.set_audience(&[auth.settings.client_id.as_str()]);
    validacion.set_issuer(&[auth.settings.issuer.as_str()]);
    validacion.set_required_spec_claims(&["exp", "iss", "aud"]);

    let datos = match jsonwebtoken::decode::<IdTokenClaims>(id_token, &clave, &validacion) {
        Ok(datos) => datos,
        Err(error) => {
            return Err(rechazar_id_token(&format!(
                "la verificación del ID token falló: {error}"
            )));
        }
    };

    // El `nonce` ata este ID token al login que lo pidió y cierra la repetición.
    // `jsonwebtoken` no lo valida, así que se compara aquí.
    if datos.claims.nonce != nonce_esperado {
        return Err(rechazar_id_token(
            "el nonce del ID token no es el emitido en el login",
        ));
    }

    // `iat` en vigor: un token emitido en el futuro no es de fiar. Tampoco lo
    // valida `jsonwebtoken`, así que se comprueba aquí. A diferencia del `exp`
    // —donde el margen es cero porque los dos extremos son podpot—, aquí hay
    // **dos relojes**, así que un desfase de un segundo entre ambos tumbaría un
    // login por lo demás en regla. De ahí el margen de cortesía: solo alarga la
    // tolerancia hacia el futuro, nunca la vigencia.
    let ahora = jsonwebtoken::get_current_timestamp();
    if datos.claims.iat > ahora.saturating_add(MARGEN_DE_RELOJ) {
        return Err(rechazar_id_token("el ID token se emitió en el futuro"));
    }

    Ok(IdentidadVerificada {
        sub: datos.claims.sub,
        email: datos.claims.email,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    use super::{
        AuthorizationRequest, Discovery, JWKS_TTL, JwksCacheado, MARGEN_DE_RELOJ, SCOPE,
        TOKEN_LENGTH, build_authorization_url, canjear_codigo, discovery_cached, discovery_url,
        jwks_cached_en, verificar_id_token,
    };
    use crate::auth::state::AuthState;
    use crate::config::OidcSettings;
    use crate::error::{AppError, BAD_GATEWAY_MESSAGE};
    use crate::secret::SecretString;
    use crate::test_support::proveedor_falso::{IdToken, ProveedorFalso, token_con_alg_none};
    use crate::test_support::{
        EMAIL, OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, SECRETO, SUB, body_json, capturar_log,
    };
    use tokio::task::JoinSet;

    /// Configuración del proveedor apuntando a un emisor dado.
    fn settings(emisor: &str) -> OidcSettings {
        OidcSettings {
            issuer: emisor.to_owned(),
            client_id: OIDC_CLIENT_ID.to_owned(),
            client_secret: SecretString::new(OIDC_CLIENT_SECRET.to_owned()),
            redirect_url: "http://127.0.0.1:8080/api/v1/auth/callback".to_owned(),
        }
    }

    /// Descubrimiento de prueba apuntando a un emisor dado.
    fn discovery(emisor: &str) -> Discovery {
        Discovery {
            authorization_endpoint: format!("{emisor}/authorize"),
            token_endpoint: format!("{emisor}/token"),
            jwks_uri: format!("{emisor}/jwks"),
        }
    }

    /// Estado de autenticación apuntando al proveedor falso dado.
    ///
    /// Es lo que necesita la verificación del ID token —y, con ella, toda la caché
    /// del JWKS—: el conjunto de claves lo resuelve el **estado**, así que el emisor
    /// tiene que ir dentro y no suelto en la llamada.
    fn auth(proveedor: &ProveedorFalso) -> AuthState {
        AuthState::new(
            settings(&proveedor.issuer),
            SecretString::new(SECRETO.to_owned()),
        )
    }

    /// Descompone una cadena de consulta en un mapa clave→valor.
    fn parametros(url: &str) -> HashMap<String, String> {
        let consulta = url
            .split_once('?')
            .map(|(_, consulta)| consulta)
            .expect("la URL debe traer cadena de consulta");

        consulta
            .split('&')
            .filter_map(|par| par.split_once('='))
            // Los valores van codificados en la URL; se decodifican para poder
            // compararlos con lo que se pidió (`openid email`, por ejemplo).
            .map(|(clave, valor)| {
                let valor = urlencoding::decode(valor)
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_else(|_| valor.to_owned());

                (clave.to_owned(), valor)
            })
            .collect()
    }

    /// Escenario «URL de descubrimiento»: se construye sobre el emisor sin
    /// duplicar la barra final.
    #[test]
    fn la_url_de_descubrimiento_recorta_la_barra_final() {
        assert_eq!(
            discovery_url("https://idp.example.test/realms/podpot/"),
            "https://idp.example.test/realms/podpot/.well-known/openid-configuration"
        );
    }

    /// Escenario «Descubrimiento con servidor simulado»: se pide al proveedor
    /// falso y se lee el documento.
    #[tokio::test]
    async fn el_descubrimiento_se_lee_del_proveedor() {
        let proveedor = ProveedorFalso::servir().await;

        let documento = super::fetch_discovery(&settings(&proveedor.issuer))
            .await
            .expect("el proveedor falso sirve el documento");

        assert_eq!(
            documento.authorization_endpoint,
            format!("{}/authorize", proveedor.issuer)
        );
        assert_eq!(documento.jwks_uri, format!("{}/jwks", proveedor.issuer));
    }

    /// Escenario «Caché de descubrimiento»: el segundo login no vuelve a pedir
    /// el documento.
    #[tokio::test]
    async fn el_descubrimiento_se_cachea_tras_la_primera_peticion() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = AuthState::new(
            settings(&proveedor.issuer),
            SecretString::new("secreto-de-firma-distinguible-000000000000000000".to_owned()),
        );

        discovery_cached(&estado).await.expect("el primero pide");
        discovery_cached(&estado)
            .await
            .expect("el segundo usa la caché");

        assert_eq!(
            proveedor.peticiones_de_descubrimiento().await,
            1,
            "el documento solo se pide una vez"
        );
    }

    /// Escenario «Redirección al proveedor»: la URL lleva exactamente los seis
    /// parámetros, codificados, con `state` y `nonce` de la longitud esperada.
    #[test]
    fn la_url_de_autorizacion_lleva_los_seis_parametros() {
        let emisor = "https://idp.example.test/realms/podpot";
        let solicitud: AuthorizationRequest =
            build_authorization_url(&settings(emisor), &discovery(emisor));

        let consulta = solicitud
            .url
            .split_once('?')
            .map(|(_, consulta)| consulta)
            .expect("la URL debe traer consulta");

        assert_eq!(
            consulta.split('&').count(),
            6,
            "deben ser exactamente seis parámetros: {}",
            solicitud.url
        );
        assert!(
            solicitud
                .url
                .starts_with("https://idp.example.test/realms/podpot/authorize?"),
            "la URL debe apuntar al authorization_endpoint: {}",
            solicitud.url
        );
        assert!(
            solicitud.url.contains(
                "redirect_uri=http%3A%2F%2F127.0.0.1%3A8080%2Fapi%2Fv1%2Fauth%2Fcallback"
            ),
            "el redirect_uri debe ir codificado: {}",
            solicitud.url
        );
        assert!(
            solicitud.url.contains("scope=openid%20email"),
            "el scope debe ir codificado (openid%20email): {}",
            solicitud.url
        );

        let mapa = parametros(&solicitud.url);
        assert_eq!(mapa.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(
            mapa.get("client_id").map(String::as_str),
            Some(OIDC_CLIENT_ID)
        );
        assert_eq!(mapa.get("scope").map(String::as_str), Some(SCOPE));
        assert_eq!(solicitud.state.len(), TOKEN_LENGTH);
        assert_eq!(solicitud.nonce.len(), TOKEN_LENGTH);
        assert_ne!(
            solicitud.state, solicitud.nonce,
            "el state y el nonce deben ser distintos"
        );
    }

    /// Escenario «Canje del código»: el `token_endpoint` devuelve un ID token y
    /// se lee tal cual, sin verificar.
    #[tokio::test]
    async fn el_canje_devuelve_el_id_token_del_proveedor() {
        let proveedor = ProveedorFalso::servir().await;
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        proveedor
            .montar_token_endpoint(&proveedor.firmar(&IdToken::valido(
                &proveedor.issuer,
                OIDC_CLIENT_ID,
                "un-nonce",
            )))
            .await;

        let id_token = canjear_codigo(&config, &documento, "codigo-de-prueba")
            .await
            .expect("el canje debe devolver el ID token");

        assert!(!id_token.is_empty(), "el ID token no puede venir vacío");
        assert_eq!(proveedor.peticiones_al_token().await, 1);
    }

    /// Escenario «ID token válido»: firma, `iss`, `aud`, `exp`/`iat` y `nonce`
    /// cuadran, así que se devuelve la identidad.
    #[tokio::test]
    async fn un_id_token_valido_devuelve_la_identidad() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let id_token = proveedor.firmar(&IdToken::valido(
            &proveedor.issuer,
            OIDC_CLIENT_ID,
            "nonce-esperado",
        ));

        let identidad = verificar_id_token(&estado, &id_token, "nonce-esperado")
            .await
            .expect("un ID token en regla debe verificar");

        assert_eq!(identidad.sub, SUB);
        assert_eq!(identidad.email, EMAIL);
    }

    /// Escenario «Firma inválida»: un ID token firmado con una clave que el JWKS
    /// no publica se rechaza.
    #[tokio::test]
    async fn un_id_token_con_firma_invalida_se_rechaza() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let id_token = proveedor.firmar_con_clave_ajena(&IdToken::valido(
            &proveedor.issuer,
            OIDC_CLIENT_ID,
            "nonce-esperado",
        ));

        let error = verificar_id_token(&estado, &id_token, "nonce-esperado")
            .await
            .expect_err("una firma que no cuadra no puede verificar");

        assert!(
            matches!(error, AppError::Unauthorized(_)),
            "el rechazo debe ser un 401 tipado: {error}"
        );
    }

    /// Escenario «Nonce distinto»: el `nonce` del ID token no es el emitido en
    /// el login, así que se rechaza.
    #[tokio::test]
    async fn un_id_token_con_nonce_distinto_se_rechaza() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let id_token = proveedor.firmar(&IdToken::valido(
            &proveedor.issuer,
            OIDC_CLIENT_ID,
            "nonce-que-no-es",
        ));

        let error = verificar_id_token(&estado, &id_token, "nonce-esperado")
            .await
            .expect_err("un nonce distinto no puede verificar");

        assert!(
            matches!(error, AppError::Unauthorized(_)),
            "el rechazo debe ser un 401 tipado: {error}"
        );
    }

    /// El margen de reloj del `iat` es el documentado: sesenta segundos.
    #[test]
    fn el_margen_de_reloj_es_de_sesenta_segundos() {
        assert_eq!(MARGEN_DE_RELOJ, 60);
    }

    // --- Los candados de la verificación del ID token ------------------------
    //
    // Todos parten de un token con **todo lo demás en regla** y tuercen una sola
    // cosa: si además fallara otra comprobación, el rechazo podría venir de ahí y
    // el candado no probaría lo que anuncia. Y todos miran la **causa
    // registrada**, no solo el `401`: la firma, el `kid`, `iss`, `aud` y el
    // algoritmo dan el mismo `401`, así que sin la causa el candado no fija de qué
    // comprobación viene el rechazo.

    /// Claims de un ID token en regla, en crudo para firmarlo con una cabecera a
    /// mano.
    ///
    /// Es el punto de partida de los ataques de cabecera y de firma: con el
    /// `nonce` del login y sin nada adelantado al reloj, lo único torcido del token
    /// es la parte que el ataque toca.
    fn claims_completos(proveedor: &ProveedorFalso, nonce: &str) -> serde_json::Value {
        let ahora = jsonwebtoken::get_current_timestamp();

        serde_json::json!({
            "iss": proveedor.issuer,
            "aud": OIDC_CLIENT_ID,
            "sub": SUB,
            "email": EMAIL,
            "nonce": nonce,
            "iat": ahora,
            "exp": ahora + 600,
        })
    }

    /// Verifica un ID token esperando un rechazo y devuelve la causa registrada.
    ///
    /// La guardia de captura se mantiene viva hasta leer el buffer —al soltarla, el
    /// hilo deja de capturar— y se suelta al volver. El rechazo tiene que ser
    /// [`AppError::Unauthorized`]: un `401` tipado, nunca un `500` ni un pánico.
    ///
    /// El estado entra por parámetro, y no el emisor suelto, porque el conjunto de
    /// claves se resuelve por él: es donde vive la caché del JWKS.
    async fn rechazo_con_su_causa(estado: &AuthState, token: &str, nonce: &str) -> String {
        let (buffer, _guardia) = capturar_log();

        let error = verificar_id_token(estado, token, nonce)
            .await
            .expect_err("el ID token tiene que rechazarse");

        assert!(
            matches!(error, AppError::Unauthorized(_)),
            "el rechazo tiene que ser un 401 tipado: {error}"
        );

        buffer.texto()
    }

    /// `alg: none` — la confusión de algoritmo más burda: el token declara que no
    /// va firmado y lleva los claims legítimos dentro.
    ///
    /// Lo que lo rechaza no es una comprobación nuestra, sino que `jsonwebtoken`
    /// no puede siquiera **representar** ese algoritmo —su `Algorithm` no tiene la
    /// variante—, y por eso se canda aquí: el día que alguien sustituya ese parseo
    /// de la cabecera por uno hecho a mano, el token entra y emite sesión sin haber
    /// comprobado nada. La aserción de la causa fija que el rechazo sale de leer la
    /// cabecera y no de otra capa que, por casualidad, también lo pare.
    #[tokio::test]
    async fn un_id_token_con_alg_none_no_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let token = token_con_alg_none(&claims_completos(&proveedor, "nonce-esperado"));

        let log = rechazo_con_su_causa(&estado, &token, "nonce-esperado").await;

        assert!(
            log.contains("la cabecera del ID token no es válida"),
            "el rechazo no viene de la cabecera: {log}"
        );
    }

    /// `HS256` con la clave pública — la confusión de algoritmo **de verdad**: el
    /// token va firmado en `HS256` usando como secreto `HMAC` la clave pública del
    /// proveedor, que es material que cualquiera tiene del `jwks_uri`. El `kid` es
    /// el legítimo, así que la clave se resuelve y lo que tiene que pararlo es la
    /// verificación.
    ///
    /// La aserción es la que distingue las dos barreras: el algoritmo está fijado a
    /// `RS256` —no se lee de la cabecera—, así que el rechazo es `InvalidAlgorithm`.
    /// Leyendo el algoritmo del token el rechazo saldría igual pero como
    /// `InvalidKeyFormat` —`jsonwebtoken` tampoco deja verificar un `HMAC` con una
    /// clave de familia RSA—, y es justo lo que esta aserción delata.
    #[tokio::test]
    async fn un_id_token_firmado_en_hs256_no_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let token = proveedor
            .firmar_confundiendo_algoritmo(&claims_completos(&proveedor, "nonce-esperado"));

        let log = rechazo_con_su_causa(&estado, &token, "nonce-esperado").await;

        assert!(
            log.contains("la verificación del ID token falló: InvalidAlgorithm"),
            "el rechazo no viene del algoritmo fijado a RS256: {log}"
        );
    }

    /// `iss` distinto del emisor configurado: el token lo firma el proveedor
    /// correcto, pero dice venir de otro.
    #[tokio::test]
    async fn un_iss_distinto_no_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let mut claims = IdToken::valido(&proveedor.issuer, OIDC_CLIENT_ID, "nonce-esperado");
        claims.iss = "https://otro-emisor.test/realms/podpot".to_owned();

        let log = rechazo_con_su_causa(&estado, &proveedor.firmar(&claims), "nonce-esperado").await;

        assert!(
            log.contains("InvalidIssuer"),
            "el rechazo no viene del emisor: {log}"
        );
    }

    /// `aud` ajeno: el token se emitió para **otro** cliente, así que no sirve para
    /// entrar aquí.
    #[tokio::test]
    async fn un_aud_ajeno_no_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let mut claims = IdToken::valido(&proveedor.issuer, OIDC_CLIENT_ID, "nonce-esperado");
        claims.aud = "otro-cliente-distinto".to_owned();

        let log = rechazo_con_su_causa(&estado, &proveedor.firmar(&claims), "nonce-esperado").await;

        assert!(
            log.contains("InvalidAudience"),
            "el rechazo no viene de la audiencia: {log}"
        );
    }

    /// El candado del **`leeway` cero**: un `exp` treinta segundos atrás lo rechaza
    /// el margen cero, y lo **aceptaría** el de cortesía que `Validation` trae por
    /// defecto —sesenta segundos—, de modo que revertir esa decisión pone este test
    /// en rojo.
    #[tokio::test]
    async fn un_id_token_caducado_no_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let ahora = jsonwebtoken::get_current_timestamp();
        let mut claims = IdToken::valido(&proveedor.issuer, OIDC_CLIENT_ID, "nonce-esperado");
        claims.iat = ahora - 60;
        claims.exp = ahora - 30;

        let log = rechazo_con_su_causa(&estado, &proveedor.firmar(&claims), "nonce-esperado").await;

        assert!(
            log.contains("ExpiredSignature"),
            "el rechazo no viene de la vigencia: {log}"
        );
    }

    /// El candado de que `exp` es **obligatorio**.
    ///
    /// El test de arriba prueba un `exp` en el pasado, pero siempre con un token que
    /// **trae** `exp`: quitara quien quitara `"exp"` de `required_spec_claims`, la
    /// suite seguiría verde y un token sin vigencia declarada pasaría. Aquí se omite
    /// el claim entero. Quien lo exige es `required_spec_claims`, que `jsonwebtoken`
    /// comprueba sobre el mapa **crudo** del token y no sobre `IdTokenClaims`.
    #[tokio::test]
    async fn un_id_token_sin_exp_no_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let mut claims = claims_completos(&proveedor, "nonce-esperado");
        claims
            .as_object_mut()
            .expect("los claims son un objeto JSON")
            .remove("exp");
        let token = proveedor.firmar_claims(&claims);

        let log = rechazo_con_su_causa(&estado, &token, "nonce-esperado").await;

        assert!(
            log.contains("Missing required claim: exp"),
            "el rechazo no viene de que falte exp: {log}"
        );
    }

    /// El candado de la comprobación de `iat`: `jsonwebtoken` no lo valida, lo mira
    /// el flujo. Con esa comprobación fuera, este token —con todo lo demás en
    /// regla— se aceptaría y emitiría sesión.
    ///
    /// Diez minutos de adelanto caen **porque están más allá del margen de
    /// cortesía**, no porque el margen sea cero: el filo de dentro lo fija el test
    /// de abajo.
    #[tokio::test]
    async fn un_id_token_emitido_en_el_futuro_no_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let ahora = jsonwebtoken::get_current_timestamp();
        let mut claims = claims_completos(&proveedor, "nonce-esperado");
        claims["iat"] = serde_json::json!(ahora + 600);
        claims["exp"] = serde_json::json!(ahora + 1800);
        let token = proveedor.firmar_claims(&claims);

        let log = rechazo_con_su_causa(&estado, &token, "nonce-esperado").await;

        assert!(
            log.contains("el ID token se emitió en el futuro"),
            "el rechazo no viene del iat adelantado al reloj: {log}"
        );
    }

    /// El otro filo de la comprobación de `iat`: el `iat` lo firma el reloj del
    /// proveedor y lo compara el de podpot —**dos relojes**—, así que un desfase de
    /// segundos entre ambos no puede tumbar un login por lo demás en regla.
    ///
    /// Es el candado de que [`MARGEN_DE_RELOJ`] existe de verdad: el test del
    /// futuro pasa igual con el margen a cero —diez minutos siguen estando por
    /// delante de cualquier margen—, así que sin este, poner el margen a cero no
    /// pondría nada en rojo.
    ///
    /// El desfase es una constante **propia** y no `MARGEN_DE_RELOJ / 2`: derivarlo
    /// del margen haría que el filo se moviera con la mutación —con el margen a
    /// cero el desfase también sería cero y el test seguiría verde—, que es justo
    /// el fallo que este candado existe para no tener. Con el desfase fijo, bajar
    /// el margen por debajo de él pone el test en rojo.
    #[tokio::test]
    async fn un_iat_dentro_del_margen_de_reloj_no_tumba_el_login() {
        /// Desfase de reloj de la prueba: la mitad del margen documentado.
        const DESFASE_DE_RELOJ: u64 = 30;

        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let mut claims = IdToken::valido(&proveedor.issuer, OIDC_CLIENT_ID, "nonce-esperado");
        claims.iat = jsonwebtoken::get_current_timestamp() + DESFASE_DE_RELOJ;

        let identidad = verificar_id_token(&estado, &proveedor.firmar(&claims), "nonce-esperado")
            .await
            .expect("un desfase de reloj dentro del margen no puede tumbar el login");

        assert_eq!(identidad.sub, SUB);
    }

    /// El candado de que `iat` está **declarado** en `IdTokenClaims`: con el campo
    /// fuera, un token sin `iat` se deserializaría y se aceptaría, porque
    /// `jsonwebtoken` no lo exige.
    ///
    /// Y aprovecha el camino de error de **deserialización** —el único que podría
    /// arrastrar al registro un fragmento del payload— para comprobar que no lo
    /// hace.
    #[tokio::test]
    async fn un_id_token_sin_iat_no_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let mut claims = claims_completos(&proveedor, "nonce-esperado");
        claims
            .as_object_mut()
            .expect("los claims son un objeto JSON")
            .remove("iat");
        let token = proveedor.firmar_claims(&claims);

        let log = rechazo_con_su_causa(&estado, &token, "nonce-esperado").await;

        assert!(
            log.contains("la verificación del ID token falló"),
            "el rechazo no viene de la verificación: {log}"
        );

        // El payload del ID token, además del token entero: es el fragmento que el
        // error de deserialización podría arrastrar.
        let payload = token.split('.').nth(1).expect("un JWT tiene tres partes");

        assert!(
            !log.contains(token.as_str()),
            "el rechazo publicó el ID token en el registro: {log}"
        );
        assert!(
            !log.contains(payload),
            "el rechazo publicó el payload del ID token en el registro: {log}"
        );
    }

    /// `kid` ausente: la cabecera no declara con qué clave va firmado, así que no
    /// hay clave que elegir. La comprobación es **anterior** a la firma, y con el
    /// JWKS publicando una sola clave un «si solo hay una, úsala» —el atajo
    /// clásico— aceptaría este token, que va firmado con esa misma clave y con
    /// todos los claims en regla.
    #[tokio::test]
    async fn un_id_token_sin_kid_no_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let token = proveedor.firmar_sin_kid(&claims_completos(&proveedor, "nonce-esperado"));

        let log = rechazo_con_su_causa(&estado, &token, "nonce-esperado").await;

        assert!(
            log.contains("el ID token no declara kid"),
            "el rechazo no viene de la ausencia de kid: {log}"
        );
    }

    /// `kid` que el `jwks_uri` no publica: el token va firmado con la clave que
    /// **sí** se publica, así que la firma cuadraría; lo que falla es la búsqueda de
    /// la clave por su `kid` —la comprobación anterior a la firma—, y sin ella el
    /// token se aceptaría.
    #[tokio::test]
    async fn un_id_token_con_un_kid_que_el_jwks_no_publica_no_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let token = proveedor.firmar_con_kid_ajeno(&claims_completos(&proveedor, "nonce-esperado"));

        let log = rechazo_con_su_causa(&estado, &token, "nonce-esperado").await;

        assert!(
            log.contains("una clave que el jwks_uri no publica"),
            "el rechazo no viene de que el kid no esté en el JWKS: {log}"
        );
    }

    // --- La caché del JWKS -----------------------------------------------------
    //
    // El conjunto de claves del proveedor se pide hoy en **cada** callback, y estas
    // pruebas son las que exigen lo contrario: que se cachee con frescura acotada,
    // que se refresque cuando aparece un `kid` que la caché no publica (rotación) y
    // que un refresco que falla siga desenlazando en el `502` de la casa. Los conteos
    // salen del contador del doble —[`ProveedorFalso::peticiones_al_jwks`]—, que
    // cuenta **idas** al `jwks_uri` responda lo que responda el proveedor: contar
    // lecturas es la única forma de probar que un callback no vuelve a pedir el
    // conjunto.

    /// Deja el conjunto del proveedor cacheado pagando **una sola** lectura.
    ///
    /// Es el punto de partida de los escenarios de rotación y de refresco fallido:
    /// en los dos, el ID token trae un `kid` que el conjunto cacheado no publica, así
    /// que lo que se mida después solo vale si antes había caché de verdad. La
    /// aserción del conteo va aquí, y no en cada prueba, porque es lo que fija que ese
    /// estado inicial sea el que el escenario supone —un conjunto **cacheado**— y no
    /// una lectura de más.
    async fn cachear_el_jwks(estado: &AuthState, proveedor: &ProveedorFalso) {
        let id_token = proveedor.firmar(&IdToken::valido(
            &proveedor.issuer,
            OIDC_CLIENT_ID,
            "nonce-esperado",
        ));

        verificar_id_token(estado, &id_token, "nonce-esperado")
            .await
            .expect("el primer callback debe verificar");
        verificar_id_token(estado, &id_token, "nonce-esperado")
            .await
            .expect("el segundo callback debe verificar con el conjunto cacheado");

        assert_eq!(
            proveedor.peticiones_al_jwks(),
            1,
            "cachear el conjunto cuesta una sola lectura del jwks_uri"
        );
    }

    /// Traduce un error del flujo a la respuesta HTTP que vería el cliente.
    ///
    /// Se hace aquí —y no en una prueba HTTP— porque el refresco del `jwks_uri` es
    /// una decisión de este módulo: es aquí donde se puede forzar que el proveedor se
    /// caiga, y el escenario del refresco fallido habla de la **respuesta**, no solo
    /// del error.
    async fn respuesta_de(error: AppError) -> (StatusCode, serde_json::Value) {
        let respuesta = error.into_response();
        let estado = respuesta.status();

        (estado, body_json(respuesta).await)
    }

    /// Escenario «Reutilización dentro del TTL»: dos callbacks válidos dentro del
    /// TTL se pagan con **una sola** lectura del `jwks_uri`, y el segundo verifica con
    /// el conjunto cacheado.
    #[tokio::test]
    async fn dos_callbacks_dentro_del_ttl_piden_el_jwks_una_sola_vez() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);
        let id_token = proveedor.firmar(&IdToken::valido(
            &proveedor.issuer,
            OIDC_CLIENT_ID,
            "nonce-esperado",
        ));

        let primero = verificar_id_token(&estado, &id_token, "nonce-esperado")
            .await
            .expect("el primer callback debe verificar");
        let segundo = verificar_id_token(&estado, &id_token, "nonce-esperado")
            .await
            .expect("el segundo callback debe verificar con el conjunto cacheado");

        assert_eq!(primero.sub, SUB);
        assert_eq!(segundo.email, EMAIL);
        assert_eq!(
            proveedor.peticiones_al_jwks(),
            1,
            "dentro del TTL el jwks_uri se lee una sola vez para los dos callbacks"
        );
    }

    /// Escenario «Expiración del TTL»: caducada la caché, la siguiente lectura
    /// vuelve a pedir el `jwks_uri` y la entrada se sustituye por la recién leída.
    ///
    /// La caducidad se fabrica **sin depender del reloj de la máquina**: se siembra la
    /// caché **fresca** y se evalúa a una fecha posterior al TTL. Restar el TTL al
    /// reloj monotónico —el recurso de la prueba del `state` caducado— daría por hecho
    /// que el sistema lleva más de un TTL en marcha, que no es cierto en un runner
    /// recién arrancado; con la fecha inyectada, la prueba decide cuándo se evalúa la
    /// caché y el vencimiento es determinista. Se siembra con el conjunto **vigente**
    /// para que el refresco solo pueda venir de la caducidad y no de un `kid` que el
    /// conjunto no publica.
    #[tokio::test]
    async fn transcurrido_el_ttl_se_vuelve_a_pedir_el_jwks() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let leido_en = Instant::now();
        *estado.jwks.write().await = Some(JwksCacheado {
            conjunto: proveedor.jwks_publicado(),
            leido_en,
        });
        let ahora = leido_en + JWKS_TTL + Duration::from_secs(1);

        jwks_cached_en(&estado, ahora)
            .await
            .expect("un conjunto caducado debe poder volver a leerse");

        assert_eq!(
            proveedor.peticiones_al_jwks(),
            1,
            "caducado el TTL, el jwks_uri se lee de nuevo"
        );

        let cacheada = estado
            .jwks
            .read()
            .await
            .clone()
            .expect("el conjunto recién leído tiene que quedar cacheado");
        assert_eq!(
            cacheada.leido_en, ahora,
            "la caché tiene que sustituirse por el conjunto recién leído, fechado a la fecha evaluada"
        );
    }

    /// Escenario «Dos callbacks concurrentes sobre una caché caducada»: con el
    /// conjunto vencido, dos callbacks que coinciden en el tiempo se pagan con **una
    /// sola** lectura del `jwks_uri`.
    ///
    /// Es el candado de que el refresco se hace con el candado de escritura en la mano
    /// y no antes de tomarlo: si cada callback pidiera el conjunto nada más ver la
    /// caché vencida, aquí habría dos lecturas. La aserción es determinista aunque el
    /// entrelazado no lo sea —da igual que el segundo callback llegue antes o después
    /// de que el primero escriba—: si mira la caché después, ya la encuentra fresca, y
    /// si espera al candado, la vuelve a mirar bajo él. Por eso no hace falta forzar un
    /// orden concreto, solo lanzarlos a la vez.
    #[tokio::test]
    async fn dos_callbacks_concurrentes_sobre_una_cache_caducada_piden_el_jwks_una_sola_vez() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        let leido_en = Instant::now();
        *estado.jwks.write().await = Some(JwksCacheado {
            conjunto: proveedor.jwks_publicado(),
            leido_en,
        });
        let ahora = leido_en + JWKS_TTL + Duration::from_secs(1);

        let mut tareas = JoinSet::new();
        for _ in 0..2 {
            let estado = estado.clone();
            tareas.spawn(async move { jwks_cached_en(&estado, ahora).await });
        }

        while let Some(resultado) = tareas.join_next().await {
            resultado
                .expect("la tarea no puede entrar en pánico")
                .expect("un conjunto caducado debe poder volver a leerse");
        }

        assert_eq!(
            proveedor.peticiones_al_jwks(),
            1,
            "dos callbacks a la vez sobre una caché caducada comparten una sola lectura"
        );
    }

    /// Escenario «Rotación de claves con un `kid` que la caché no publica»: el
    /// proveedor publica una clave nueva y el ID token va firmado con ella, pero el
    /// conjunto cacheado es el de antes; se refresca una vez y el token entra.
    ///
    /// El escenario es sobre el conjunto **cacheado**, así que la prueba primero lo
    /// cachea —y fija que ese tramo cuesta una sola lectura— y solo entonces rota:
    /// sin la caché, el `jwks_uri` publica siempre la clave nueva y el token entraría
    /// sin refrescar nada, que es justo lo que la prueba no puede dar por bueno.
    #[tokio::test]
    async fn un_kid_que_la_cache_no_publica_se_refresca_una_vez_y_se_acepta() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        cachear_el_jwks(&estado, &proveedor).await;

        // El proveedor rota: deja de publicar la clave vigente y publica otra, con
        // otro `kid`. La caché se queda con la vieja.
        let rotada = proveedor.rotar_clave();
        let token_rotado = proveedor.firmar_con_rotada(
            &rotada,
            &IdToken::valido(&proveedor.issuer, OIDC_CLIENT_ID, "nonce-esperado"),
        );

        let identidad = verificar_id_token(&estado, &token_rotado, "nonce-esperado")
            .await
            .expect("el refresco del JWKS debe aceptar el ID token rotado");

        assert_eq!(identidad.sub, SUB);
        assert_eq!(
            proveedor.peticiones_al_jwks(),
            2,
            "el refresco por un kid que la caché no publica se pide una sola vez más"
        );
    }

    /// Escenario «Proveedor caído al refrescar el JWKS»: el `jwks_uri` deja de
    /// responder justo cuando hay que refrescar. El desenlace es el `502` de la casa
    /// —la envoltura de siempre, y **sin** la causa del proveedor— y no hay identidad
    /// que emitir: no hay sesión.
    ///
    /// El refresco lo fuerza la rotación: el ID token trae un `kid` que el conjunto
    /// cacheado no publica, así que el flujo **tiene** que ir al proveedor, y es ahí
    /// donde se lo encuentra caído. Si se rindiera con un rechazo genérico en vez de
    /// con el `502`, este candado se pondría rojo.
    #[tokio::test]
    async fn un_jwks_que_falla_al_refrescar_da_un_502_sin_sesion() {
        let proveedor = ProveedorFalso::servir().await;
        let estado = auth(&proveedor);

        cachear_el_jwks(&estado, &proveedor).await;

        let rotada = proveedor.rotar_clave();
        let token_rotado = proveedor.firmar_con_rotada(
            &rotada,
            &IdToken::valido(&proveedor.issuer, OIDC_CLIENT_ID, "nonce-esperado"),
        );
        proveedor.tumbar_el_jwks();

        let error = verificar_id_token(&estado, &token_rotado, "nonce-esperado")
            .await
            .expect_err("sin JWKS al que ir no hay identidad que emitir");

        assert!(
            matches!(error, AppError::BadGateway(_)),
            "un proveedor que no responde al JWKS es un 502 tipado: {error}"
        );

        let (estado_http, cuerpo) = respuesta_de(error).await;

        assert_eq!(
            estado_http,
            StatusCode::BAD_GATEWAY,
            "el refresco que falla desenlaza en 502: {cuerpo}"
        );
        assert_eq!(cuerpo["error"]["code"], "bad_gateway");
        assert_eq!(
            cuerpo["error"]["message"], BAD_GATEWAY_MESSAGE,
            "el cuerpo no puede publicar la causa del proveedor: {cuerpo}"
        );
        assert_eq!(
            proveedor.peticiones_al_jwks(),
            2,
            "el jwks_uri se refresca una sola vez antes de rendirse"
        );
    }
}
