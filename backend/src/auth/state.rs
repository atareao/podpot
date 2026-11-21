//! El estado de la autenticación: la caché de descubrimiento y el registro de
//! los `state`/`nonce` de un solo uso.
//!
//! Existe **sólo** cuando el modo es `Oidc`: en `Disabled` no hay capa ninguna
//! que montar y guardar un secreto que nadie va a usar sería guardar un secreto
//! de más. Por eso [`crate::state::AppState::auth`] es un `Option` y no un campo
//! vacío: el modo se lee en el tipo, y quien tenga un [`AuthState`] en la mano
//! sabe que hay autenticación sin consultar la configuración.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, RwLock};

use crate::auth::oidc::Discovery;
use crate::config::{AuthMode, Config, OidcSettings};
use crate::secret::SecretString;

/// Caducidad de un `state` emitido en el login: 5 minutos.
///
/// Un login que no se completa en ese plazo es un intento abandonado. La
/// comprobación se hace **al consumir**, en [`AuthState::consumir_login`], no
/// solo al registrar: registrar la marca de tiempo sin mirarla nunca dejaría
/// pasar un `state` de hace horas.
pub const STATE_TTL: Duration = Duration::from_secs(5 * 60);

/// Un login en curso: el `nonce` emitido y el instante en que se emitió.
///
/// El `nonce` se guarda **junto** al `state` y no en un mapa aparte: el callback
/// lo compara con el del ID token al verificar, y con la asociación hecha
/// —`state → (nonce, instante)`— esa comparación no depende de nada más.
#[derive(Debug, Clone)]
pub struct PendingLogin {
    /// `nonce` que viajó en la petición de autorización.
    pub nonce: String,
    /// Instante de emisión, para aplicar el TTL.
    pub issued_at: Instant,
}

impl PendingLogin {
    /// ¿Ha caducado esta entrada en `ahora`?
    ///
    /// `saturating_duration_since` y no `duration_since`: un instante anterior
    /// al de emisión —imposible con un reloj monótono, pero gratis de cubrir—
    /// no puede entrar en pánico.
    pub fn expirado_en(&self, ahora: Instant) -> bool {
        ahora.saturating_duration_since(self.issued_at) >= STATE_TTL
    }
}

/// Por qué se rechaza un `state` al consumirlo en el callback.
///
/// Las tres condiciones son distintas y se distinguen en el código y en el
/// registro, aunque el mensaje que ve el cliente sea el mismo: quien no ha
/// entrado no aprende si su `state` era inventado, caducado o ya gastado.
/// Separar los tres casos es lo que hace que la comprobación de caducidad no se
/// pueda perder sin que una prueba lo note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RechazoState {
    /// El `state` no se emitió nunca: no está en el mapa ni entre los gastados.
    Desconocido,
    /// El `state` se emitió, pero hace más de [`STATE_TTL`].
    Caducado,
    /// El `state` ya lo gastó un callback anterior.
    Reutilizado,
}

/// Estado de la autenticación en modo `Oidc`.
#[derive(Debug, Clone)]
pub struct AuthState {
    /// Clave de firma de la sesión (`PODPOT_SECRET`), ya resuelta por la config.
    ///
    /// Es un [`SecretString`] y no un `String` para que no pueda salir por
    /// `Debug` ni acabar en una línea de registro por descuido.
    pub secret: SecretString,
    /// Configuración del proveedor: emisor, `client_id`, secreto y retorno.
    ///
    /// Va tras un [`Arc`] porque el estado se clona en cada petición: clonar el
    /// `Arc` no vuelve a copiar las cadenas.
    pub settings: Arc<OidcSettings>,
    /// Descubrimiento cacheado del proveedor: `None` hasta que conteste.
    ///
    /// Va tras un [`RwLock`] y no un `Mutex`: el servidor es concurrente, y
    /// varios logins pueden leer la caché a la vez sin estorbarse. Se escribe
    /// solo cuando llega el primer resultado.
    pub discovery: Arc<RwLock<Option<Discovery>>>,
    /// Logins en curso: `state → (nonce, instante de emisión)`.
    ///
    /// Tras un [`Mutex`] porque insertar y purgar son mutaciones: el mapa no
    /// puede crecer sin límite con cada intento de entrada que nadie completa.
    pub logins: Arc<Mutex<HashMap<String, PendingLogin>>>,
    /// `state` ya consumidos, con el instante en que se gastaron.
    ///
    /// Se recuerdan para poder distinguir «reutilizado» de «desconocido» sin
    /// confundirlos: si un `state` consumido se descartara sin dejar rastro, un
    /// segundo callback con el mismo `state` parecería uno inventado. La purga
    /// por TTL acota su crecimiento igual que la de los pendientes.
    pub consumidos: Arc<Mutex<HashMap<String, Instant>>>,
}

impl AuthState {
    /// Construye el estado de autenticación con el proveedor y el secreto dados.
    pub fn new(settings: OidcSettings, secret: SecretString) -> Self {
        Self {
            secret,
            settings: Arc::new(settings),
            discovery: Arc::new(RwLock::new(None)),
            logins: Arc::new(Mutex::new(HashMap::new())),
            consumidos: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Deriva el estado de autenticación de la configuración, si la hay.
    ///
    /// Devuelve `Some` **sólo** en modo `Oidc`, donde la configuración garantiza
    /// que hay secreto de firma. En `Disabled` devuelve `None`: no hay capa que
    /// montar, y eso es exactamente lo que el `Option` declara.
    pub fn from_config(config: &Config) -> Option<Self> {
        match (&config.auth, &config.secret) {
            (AuthMode::Oidc(settings), Some(secret)) => {
                Some(Self::new(settings.clone(), secret.clone()))
            }
            _ => None,
        }
    }

    /// Registra un `state` emitido junto a su `nonce` y su marca de tiempo.
    ///
    /// Purga las entradas caducadas **al insertar**: sin esto el mapa crecería
    /// con cada login que nadie termina —un `state` que caduca sin consumirse— y
    /// el proceso acabaría guardando la memoria de intentos abandonados. La
    /// purga acota el mapa a la ventana del TTL; no le pone un tope duro, que la
    /// spec no exige.
    pub async fn registrar_login(&self, state: &str, nonce: &str) {
        let ahora = Instant::now();
        let mut logins = self.logins.lock().await;

        logins.retain(|_, pendiente| !pendiente.expirado_en(ahora));
        logins.insert(
            state.to_owned(),
            PendingLogin {
                nonce: nonce.to_owned(),
                issued_at: ahora,
            },
        );
    }

    /// Consume un `state`: lo extrae del mapa y lo descarta una sola vez.
    ///
    /// Distingue los tres rechazos —desconocido, caducado y reutilizado— y
    /// comprueba la caducidad **de verdad** al consumir, no solo la registra. La
    /// purga de entradas caducadas y de marcas de consumo viejas ocurre aquí, de
    /// modo que ni el mapa ni su rastro de gastados crecen sin límite.
    pub async fn consumir_login(&self, state: &str) -> Result<PendingLogin, RechazoState> {
        let ahora = Instant::now();
        let mut logins = self.logins.lock().await;
        let mut consumidos = self.consumidos.lock().await;

        // La entrada objetivo se saca **antes** de purgar: si la purga la
        // borrara primero, una entrada caducada se volvería indistinguible de una
        // desconocida y las dos condiciones dejarían de separarse.
        let entrada = logins.remove(state);

        logins.retain(|_, pendiente| !pendiente.expirado_en(ahora));
        consumidos.retain(|_, gastado_en| ahora.saturating_duration_since(*gastado_en) < STATE_TTL);

        match entrada {
            None => {
                if consumidos.contains_key(state) {
                    Err(RechazoState::Reutilizado)
                } else {
                    Err(RechazoState::Desconocido)
                }
            }
            Some(pendiente) if pendiente.expirado_en(ahora) => Err(RechazoState::Caducado),
            Some(pendiente) => {
                consumidos.insert(state.to_owned(), ahora);
                Ok(pendiente)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{AuthState, PendingLogin, RechazoState, STATE_TTL};
    use crate::config::OidcSettings;
    use crate::secret::SecretString;

    /// Estado de autenticación de prueba, con un proveedor cualquiera.
    fn estado() -> AuthState {
        AuthState::new(
            OidcSettings {
                issuer: "https://idp.example.test/realms/podpot".to_owned(),
                client_id: "podpot-panel".to_owned(),
                client_secret: SecretString::new("secreto-de-cliente-distinguible".to_owned()),
                redirect_url: "http://127.0.0.1:8080/api/v1/auth/callback".to_owned(),
            },
            SecretString::new("secreto-de-firma-distinguible-000000000000000000".to_owned()),
        )
    }

    /// Escenario «State desconocido»: un `state` que nunca se emitió se rechaza
    /// como tal.
    #[tokio::test]
    async fn un_state_desconocido_se_rechaza_como_desconocido() {
        let estado = estado();

        let rechazo = estado
            .consumir_login("state-que-nadie-emitio")
            .await
            .expect_err("un state no emitido no puede consumirse");

        assert_eq!(rechazo, RechazoState::Desconocido);
    }

    /// Escenario «State reutilizado»: el mismo `state` dos veces; la segunda se
    /// rechaza como reutilizado, no como desconocido.
    #[tokio::test]
    async fn un_state_ya_gastado_se_rechaza_como_reutilizado() {
        let estado = estado();
        estado
            .registrar_login("state-de-un-login", "nonce-de-un-login")
            .await;

        let primero = estado
            .consumir_login("state-de-un-login")
            .await
            .expect("el primer consumo debe valer");
        assert_eq!(primero.nonce, "nonce-de-un-login");

        let rechazo = estado
            .consumir_login("state-de-un-login")
            .await
            .expect_err("el segundo consumo no puede valer");

        assert_eq!(
            rechazo,
            RechazoState::Reutilizado,
            "un state ya gastado no es uno desconocido"
        );
    }

    /// Escenario «State caducado»: un `state` emitido hace más de [`STATE_TTL`]
    /// se rechaza como caducado al consumirlo.
    #[tokio::test]
    async fn un_state_caducado_se_rechaza_como_caducado() {
        let estado = estado();
        // Se registra a mano con una marca de tiempo vieja: es la única forma de
        // fabricar un state caducado sin esperar cinco minutos.
        estado.logins.lock().await.insert(
            "state-viejo".to_owned(),
            PendingLogin {
                nonce: "nonce-viejo".to_owned(),
                issued_at: Instant::now() - STATE_TTL - Duration::from_secs(1),
            },
        );

        let rechazo = estado
            .consumir_login("state-viejo")
            .await
            .expect_err("un state caducado no puede consumirse");

        assert_eq!(rechazo, RechazoState::Caducado);
    }
}
