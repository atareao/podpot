//! El flujo OIDC: descubrimiento del proveedor, URL de autorización,
//! `state`/`nonce` y, ya en el retorno, el canje del código y la verificación
//! del ID token.
//!
//! Aquí vive todo el flujo salvo el HTTP: el descubrimiento que el `login`
//! necesita para redirigir al proveedor, y lo que el `callback` necesita para
//! convertir el retorno en una identidad verificada. El flujo se escribe a mano
//! —patrón de kesto— y no con `openidconnect`: la caja no está en la caché de
//! Cargo de esta máquina y el repositorio tiene que poder compilarse sin conexión.

use std::time::Duration;

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
pub async fn verificar_id_token(
    settings: &OidcSettings,
    discovery: &Discovery,
    id_token: &str,
    nonce_esperado: &str,
) -> Result<IdentidadVerificada, AppError> {
    let jwks = fetch_jwks(&discovery.jwks_uri).await?;

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
    let Some(jwk) = jwks.find(&kid) else {
        return Err(rechazar_id_token(
            "el ID token va firmado con una clave que el jwks_uri no publica",
        ));
    };
    let clave = match DecodingKey::from_jwk(jwk) {
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
    validacion.set_audience(&[settings.client_id.as_str()]);
    validacion.set_issuer(&[settings.issuer.as_str()]);
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

    use super::{
        AuthorizationRequest, Discovery, MARGEN_DE_RELOJ, SCOPE, TOKEN_LENGTH,
        build_authorization_url, canjear_codigo, discovery_cached, discovery_url,
        verificar_id_token,
    };
    use crate::auth::state::AuthState;
    use crate::config::OidcSettings;
    use crate::error::AppError;
    use crate::secret::SecretString;
    use crate::test_support::proveedor_falso::{IdToken, ProveedorFalso, token_con_alg_none};
    use crate::test_support::{EMAIL, OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, SUB, capturar_log};

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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let id_token = proveedor.firmar(&IdToken::valido(
            &proveedor.issuer,
            OIDC_CLIENT_ID,
            "nonce-esperado",
        ));

        let identidad = verificar_id_token(&config, &documento, &id_token, "nonce-esperado")
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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let id_token = proveedor.firmar_con_clave_ajena(&IdToken::valido(
            &proveedor.issuer,
            OIDC_CLIENT_ID,
            "nonce-esperado",
        ));

        let error = verificar_id_token(&config, &documento, &id_token, "nonce-esperado")
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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let id_token = proveedor.firmar(&IdToken::valido(
            &proveedor.issuer,
            OIDC_CLIENT_ID,
            "nonce-que-no-es",
        ));

        let error = verificar_id_token(&config, &documento, &id_token, "nonce-esperado")
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
    async fn rechazo_con_su_causa(
        config: &OidcSettings,
        documento: &Discovery,
        token: &str,
        nonce: &str,
    ) -> String {
        let (buffer, _guardia) = capturar_log();

        let error = verificar_id_token(config, documento, token, nonce)
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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let token = token_con_alg_none(&claims_completos(&proveedor, "nonce-esperado"));

        let log = rechazo_con_su_causa(&config, &documento, &token, "nonce-esperado").await;

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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let token = proveedor
            .firmar_confundiendo_algoritmo(&claims_completos(&proveedor, "nonce-esperado"));

        let log = rechazo_con_su_causa(&config, &documento, &token, "nonce-esperado").await;

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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let mut claims = IdToken::valido(&proveedor.issuer, OIDC_CLIENT_ID, "nonce-esperado");
        claims.iss = "https://otro-emisor.test/realms/podpot".to_owned();

        let log = rechazo_con_su_causa(
            &config,
            &documento,
            &proveedor.firmar(&claims),
            "nonce-esperado",
        )
        .await;

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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let mut claims = IdToken::valido(&proveedor.issuer, OIDC_CLIENT_ID, "nonce-esperado");
        claims.aud = "otro-cliente-distinto".to_owned();

        let log = rechazo_con_su_causa(
            &config,
            &documento,
            &proveedor.firmar(&claims),
            "nonce-esperado",
        )
        .await;

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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let ahora = jsonwebtoken::get_current_timestamp();
        let mut claims = IdToken::valido(&proveedor.issuer, OIDC_CLIENT_ID, "nonce-esperado");
        claims.iat = ahora - 60;
        claims.exp = ahora - 30;

        let log = rechazo_con_su_causa(
            &config,
            &documento,
            &proveedor.firmar(&claims),
            "nonce-esperado",
        )
        .await;

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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let mut claims = claims_completos(&proveedor, "nonce-esperado");
        claims
            .as_object_mut()
            .expect("los claims son un objeto JSON")
            .remove("exp");
        let token = proveedor.firmar_claims(&claims);

        let log = rechazo_con_su_causa(&config, &documento, &token, "nonce-esperado").await;

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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let ahora = jsonwebtoken::get_current_timestamp();
        let mut claims = claims_completos(&proveedor, "nonce-esperado");
        claims["iat"] = serde_json::json!(ahora + 600);
        claims["exp"] = serde_json::json!(ahora + 1800);
        let token = proveedor.firmar_claims(&claims);

        let log = rechazo_con_su_causa(&config, &documento, &token, "nonce-esperado").await;

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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let mut claims = IdToken::valido(&proveedor.issuer, OIDC_CLIENT_ID, "nonce-esperado");
        claims.iat = jsonwebtoken::get_current_timestamp() + DESFASE_DE_RELOJ;

        let identidad = verificar_id_token(
            &config,
            &documento,
            &proveedor.firmar(&claims),
            "nonce-esperado",
        )
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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let mut claims = claims_completos(&proveedor, "nonce-esperado");
        claims
            .as_object_mut()
            .expect("los claims son un objeto JSON")
            .remove("iat");
        let token = proveedor.firmar_claims(&claims);

        let log = rechazo_con_su_causa(&config, &documento, &token, "nonce-esperado").await;

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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let token = proveedor.firmar_sin_kid(&claims_completos(&proveedor, "nonce-esperado"));

        let log = rechazo_con_su_causa(&config, &documento, &token, "nonce-esperado").await;

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
        let config = settings(&proveedor.issuer);
        let documento = discovery(&proveedor.issuer);

        let token = proveedor.firmar_con_kid_ajeno(&claims_completos(&proveedor, "nonce-esperado"));

        let log = rechazo_con_su_causa(&config, &documento, &token, "nonce-esperado").await;

        assert!(
            log.contains("una clave que el jwks_uri no publica"),
            "el rechazo no viene de que el kid no esté en el JWKS: {log}"
        );
    }
}
