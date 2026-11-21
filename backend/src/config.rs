//! La configuración de podpot, leída del entorno.
//!
//! Tres variables sin prefijo —[`BIND_ADDRESS_VAR`], [`DATABASE_URL_VAR`] y
//! [`RUST_LOG_VAR`]— y el resto con el prefijo [`ENV_PREFIX`]. La lectura entra
//! por [`Config::from_source`], no por `std::env` directo, para que las pruebas
//! fijen cada regla sin tocar el entorno del proceso.

use std::fmt;
use std::net::SocketAddr;
use std::str::FromStr;

use thiserror::Error;

/// La variable con la dirección de escucha.
pub const BIND_ADDRESS_VAR: &str = "BIND_ADDRESS";

/// La variable con el DSN de la base de datos. Es obligatoria.
pub const DATABASE_URL_VAR: &str = "DATABASE_URL";

/// La variable con el filtro de registro.
pub const RUST_LOG_VAR: &str = "RUST_LOG";

/// El prefijo de las variables propias de podpot.
///
/// Lo de fuera de este prefijo son tres nombres convenidos con el ecosistema
/// (`BIND_ADDRESS`, `DATABASE_URL`, `RUST_LOG`); todo lo demás que podpot invente
/// va con el prefijo, para no chocar con lo que espere otro programa del mismo
/// contenedor.
pub const ENV_PREFIX: &str = "PODPOT_";

/// El backend de systemd que debe usar el proceso.
pub const SYSTEMD_VAR: &str = "PODPOT_SYSTEMD";

/// La dirección de escucha por defecto: sólo loopback.
///
/// El defecto es loopback a propósito: podpot controla contenedores del usuario y
/// no lleva autenticación en esta entrega, así que exponerlo sin querer sería
/// regalar el control de los contenedores de la máquina. Exponerlo es una
/// decisión explícita, y se hace escribiendo `BIND_ADDRESS`.
pub const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";

/// El filtro de registro por defecto.
pub const DEFAULT_LOG_FILTER: &str = "info";

/// De dónde saca podpot el estado de las unidades.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemdBackend {
    /// D-Bus contra el manager de systemd del usuario. Es el modo de verdad.
    Dbus,
    /// Un doble en memoria: fabrica datos y no habla con systemd.
    ///
    /// Existe para desarrollo y para las pruebas de proceso —arrancar podpot sin
    /// un bus de usuario, por ejemplo en un contenedor que no lo monta—, y **no**
    /// es un modo degradado: sus datos no son el estado real de ninguna unidad.
    Fake,
}

impl SystemdBackend {
    /// Lee el backend del valor de la variable, o `None` si no es ninguno.
    fn parse(valor: &str) -> Option<Self> {
        match valor {
            "dbus" => Some(Self::Dbus),
            "fake" => Some(Self::Fake),
            _ => None,
        }
    }

    /// El nombre del backend, tal y como lo escribe la configuración.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Dbus => "dbus",
            Self::Fake => "fake",
        }
    }
}

impl fmt::Display for SystemdBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Por qué no se puede arrancar con esta configuración.
///
/// Cada mensaje **nombra la variable**, nunca su valor: quien arranca el proceso
/// tiene que poder ir a arreglar el entorno sin que el registro publique lo que
/// llevaba dentro.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    /// La variable es obligatoria y no está.
    #[error("falta la variable de entorno `{var}`")]
    Missing {
        /// La variable que falta.
        var: &'static str,
    },

    /// La variable está definida pero vacía. **No** se cae al valor por defecto.
    ///
    /// Es deliberado y es el caso de un fichero de entorno o un `compose` con la
    /// variable declarada y sin valor: eso es un error de quien despliega, y
    /// resolverlo en silencio con el defecto dejaría el proceso escuchando donde
    /// no se pidió.
    #[error("la variable de entorno `{var}` está definida pero vacía")]
    Empty {
        /// La variable vacía.
        var: &'static str,
    },

    /// La variable tiene un valor que no se puede leer.
    #[error("la variable de entorno `{var}` no tiene un valor válido: {causa}")]
    Invalid {
        /// La variable malformada.
        var: &'static str,
        /// El motivo, tal y como lo informó el lector.
        causa: String,
    },
}

/// La configuración ya resuelta.
#[derive(Debug, Clone)]
pub struct Config {
    /// La dirección en la que escuchar.
    pub bind_address: SocketAddr,
    /// El DSN de la base de datos.
    pub database_url: String,
    /// El filtro de registro.
    pub log_filter: String,
    /// De dónde se lee el estado de las unidades.
    pub systemd_backend: SystemdBackend,
}

impl Config {
    /// Lee la configuración del entorno del proceso.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_source(|clave| std::env::var(clave).ok())
    }

    /// Lee la configuración de la fuente que se le dé.
    ///
    /// Es la puerta de las pruebas: `std::env::set_var` no es seguro con hilos
    /// —y en `edition = "2024"` ni siquiera es seguro llamarlo—, así que las
    /// variables entran por parámetro y el entorno del proceso no se toca.
    pub fn from_source(leer: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        Ok(Self {
            bind_address: bind_address(&leer)?,
            database_url: obligatoria(&leer, DATABASE_URL_VAR)?,
            log_filter: opcional(&leer, RUST_LOG_VAR)
                .unwrap_or_else(|| DEFAULT_LOG_FILTER.to_owned()),
            systemd_backend: backend(&leer)?,
        })
    }

    /// Un volcado de la configuración para el registro de arranque.
    ///
    /// No lleva la `DATABASE_URL`: es lo único que puede parecerse a una
    /// credencial en un DSN de otro motor, y el proceso tiene que poder publicar
    /// su configuración efectiva sin depender de qué DSN le hayan puesto.
    pub fn describe(&self) -> String {
        format!(
            "bind_address={} systemd={} log_filter={}",
            self.bind_address, self.systemd_backend, self.log_filter
        )
    }
}

/// Cómo está definida una variable: ausente, vacía o con valor.
enum Presencia {
    Ausente,
    Vacia,
    Valor(String),
}

/// Lee una variable distinguiendo los tres casos que la configuración trata distinto.
fn presencia(leer: &impl Fn(&str) -> Option<String>, var: &'static str) -> Presencia {
    match leer(var) {
        None => Presencia::Ausente,
        Some(valor) if valor.trim().is_empty() => Presencia::Vacia,
        Some(valor) => Presencia::Valor(valor.trim().to_owned()),
    }
}

/// Una variable obligatoria: ausente y vacía son errores distintos.
fn obligatoria(
    leer: &impl Fn(&str) -> Option<String>,
    var: &'static str,
) -> Result<String, ConfigError> {
    match presencia(leer, var) {
        Presencia::Ausente => Err(ConfigError::Missing { var }),
        Presencia::Vacia => Err(ConfigError::Empty { var }),
        Presencia::Valor(valor) => Ok(valor),
    }
}

/// Una variable opcional: sin valor, `None`; vacía también es `None`.
fn opcional(leer: &impl Fn(&str) -> Option<String>, var: &'static str) -> Option<String> {
    match presencia(leer, var) {
        Presencia::Valor(valor) => Some(valor),
        Presencia::Ausente | Presencia::Vacia => None,
    }
}

/// La dirección de escucha: el defecto, o la que pida la variable.
fn bind_address(leer: &impl Fn(&str) -> Option<String>) -> Result<SocketAddr, ConfigError> {
    match presencia(leer, BIND_ADDRESS_VAR) {
        // El defecto se compone en el código y no se parsea de `DEFAULT_BIND_ADDRESS`:
        // el valor por defecto no puede depender de que una cadena se pueda leer.
        Presencia::Ausente => Ok(SocketAddr::from(([127, 0, 0, 1], 8080))),
        Presencia::Vacia => Err(ConfigError::Empty {
            var: BIND_ADDRESS_VAR,
        }),
        Presencia::Valor(valor) => {
            SocketAddr::from_str(&valor).map_err(|causa| ConfigError::Invalid {
                var: BIND_ADDRESS_VAR,
                causa: causa.to_string(),
            })
        }
    }
}

/// El backend de systemd: por defecto, D-Bus contra el bus de usuario.
fn backend(leer: &impl Fn(&str) -> Option<String>) -> Result<SystemdBackend, ConfigError> {
    match opcional(leer, SYSTEMD_VAR) {
        None => Ok(SystemdBackend::Dbus),
        Some(valor) => SystemdBackend::parse(&valor).ok_or_else(|| ConfigError::Invalid {
            var: SYSTEMD_VAR,
            causa: format!(
                "se esperaba `{}` o `{}`",
                SystemdBackend::Dbus.as_str(),
                SystemdBackend::Fake.as_str()
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::{Config, ConfigError, DEFAULT_BIND_ADDRESS, SystemdBackend};

    /// Un entorno con sólo las variables indicadas: el resto se lee como ausente.
    fn fuente(pares: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let pares: Vec<(String, String)> = pares
            .iter()
            .map(|(clave, valor)| ((*clave).to_owned(), (*valor).to_owned()))
            .collect();

        move |clave: &str| {
            pares
                .iter()
                .find(|(nombre, _)| nombre == clave)
                .map(|(_, valor)| valor.clone())
        }
    }

    /// El caso mínimo que arranca: sin `BIND_ADDRESS` y con base de datos.
    fn minimo() -> impl Fn(&str) -> Option<String> + use<> {
        fuente(&[("DATABASE_URL", "sqlite:///tmp/podpot.db")])
    }

    /// Escenario «Valor por defecto del bind»: sin `BIND_ADDRESS`, se escucha en
    /// `127.0.0.1:8080`.
    #[test]
    fn sin_bind_address_se_escucha_en_loopback_8080() {
        let config = Config::from_source(minimo()).expect("la configuración mínima debe leerse");

        assert_eq!(
            config.bind_address,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080),
            "el defecto del bind tiene que ser 127.0.0.1:8080"
        );
        assert_eq!(DEFAULT_BIND_ADDRESS, "127.0.0.1:8080");
    }

    /// Escenario «Bind vacío no cae al defecto»: definida y vacía es un error que
    /// nombra la variable, nunca el valor por defecto.
    #[test]
    fn un_bind_vacio_no_cae_al_defecto() {
        let error = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("BIND_ADDRESS", ""),
        ]))
        .expect_err("un BIND_ADDRESS vacío no puede dar una configuración en pie");

        assert_eq!(
            error,
            ConfigError::Empty {
                var: "BIND_ADDRESS"
            },
            "el error tiene que nombrar la variable vacía: {error}"
        );
        assert!(
            error.to_string().contains("BIND_ADDRESS"),
            "el mensaje tiene que nombrar la variable: {error}"
        );
    }

    /// Escenario «Valor malformado»: lo que no es `host:puerto` falla nombrando la
    /// variable, y sin dejar entrever el valor por defecto.
    #[test]
    fn un_bind_malformado_falla_nombrando_la_variable() {
        for malo in [
            "8080",
            "127.0.0.1",
            "localhost",
            "127.0.0.1:no-es-un-puerto",
        ] {
            let error = Config::from_source(fuente(&[
                ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
                ("BIND_ADDRESS", malo),
            ]))
            .expect_err("un BIND_ADDRESS malformado no puede dar una configuración en pie");

            assert!(
                matches!(
                    error,
                    ConfigError::Invalid {
                        var: "BIND_ADDRESS",
                        ..
                    }
                ),
                "se esperaba un error de valor inválido nombrando la variable: {error}"
            );
        }
    }

    /// Escenario «Falta la base de datos»: sin `DATABASE_URL` no hay arranque, y el
    /// error nombra la variable.
    #[test]
    fn sin_database_url_falla_nombrando_la_variable() {
        let error = Config::from_source(fuente(&[]))
            .expect_err("sin DATABASE_URL no puede haber configuración");

        assert_eq!(
            error,
            ConfigError::Missing {
                var: "DATABASE_URL"
            }
        );
        assert!(
            error.to_string().contains("DATABASE_URL"),
            "el mensaje tiene que nombrar la variable: {error}"
        );
    }

    /// Requisito «Variables del entorno»: `RUST_LOG` no es obligatoria y su ausencia
    /// tiene un defecto sensato.
    #[test]
    fn rust_log_es_opcional_y_tiene_defecto() {
        let config = Config::from_source(minimo()).expect("la configuración mínima debe leerse");

        assert_eq!(config.log_filter, "info");
    }

    /// Requisito «Variables del entorno»: `RUST_LOG` definida se respeta.
    #[test]
    fn rust_log_definida_se_respeta() {
        let config = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("RUST_LOG", "podpot=debug,tower_http=warn"),
        ]))
        .expect("RUST_LOG es un filtro válido");

        assert_eq!(config.log_filter, "podpot=debug,tower_http=warn");
    }

    /// Requisito «Variables del entorno»: el backend de systemd es una variable con
    /// prefijo `PODPOT_`, y sin ella se habla por D-Bus con el bus de usuario.
    #[test]
    fn el_backend_de_systemd_va_con_prefijo_y_por_defecto_es_dbus() {
        let config = Config::from_source(minimo()).expect("la configuración mínima debe leerse");
        assert_eq!(config.systemd_backend, SystemdBackend::Dbus);

        let config = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("PODPOT_SYSTEMD", "fake"),
        ]))
        .expect("`fake` es un backend conocido");
        assert_eq!(config.systemd_backend, SystemdBackend::Fake);

        let error = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("PODPOT_SYSTEMD", "telepatia"),
        ]))
        .expect_err("un backend desconocido no puede arrancar");
        assert!(
            matches!(
                error,
                ConfigError::Invalid {
                    var: "PODPOT_SYSTEMD",
                    ..
                }
            ),
            "el error tiene que nombrar la variable: {error}"
        );
    }

    /// El volcado de arranque no puede publicar el valor de las variables que no son
    /// suyas: describe lo que sí puede contar.
    #[test]
    fn describe_cuenta_lo_que_puede_contar() {
        let config = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("BIND_ADDRESS", "127.0.0.1:9999"),
        ]))
        .expect("la configuración se lee");

        let descrito = config.describe();

        assert!(descrito.contains("127.0.0.1:9999"), "{descrito}");
        assert!(descrito.contains("dbus"), "{descrito}");
    }
}
