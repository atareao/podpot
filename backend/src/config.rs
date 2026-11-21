//! La configuración de podpot, leída del entorno.
//!
//! Tres variables sin prefijo —[`BIND_ADDRESS_VAR`], [`DATABASE_URL_VAR`] y
//! [`RUST_LOG_VAR`]— y el resto con el prefijo [`ENV_PREFIX`]. La lectura entra
//! por [`Config::from_source`], no por `std::env` directo, para que las pruebas
//! fijen cada regla sin tocar el entorno del proceso.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
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

/// El directorio del panel estático que sirve el propio backend.
///
/// La imagen de producción copia el build del panel a `/app/static` y exporta
/// esta variable (ver `Containerfile`); en desarrollo, sin ella, el proceso
/// arranca sólo como API.
pub const STATIC_DIR_VAR: &str = "PODPOT_STATIC_DIR";

/// El directorio canónico en el que se escriben las quadlets nuevas.
///
/// Es el **destino** de la creación, y no el origen: podpot escribe la definición
/// donde el generador de systemd la busca, sin factoría de symlinks (ver
/// `docs/creacion-quadlets`). Se valida contra el disco al leer la configuración:
/// un destino que no es un directorio impide arrancar, porque arrancar con un
/// destino imposible sólo serviría para fallar en la primera creación.
pub const QUADLET_DIR_VAR: &str = "PODPOT_QUADLET_DIR";

/// El `$HOME` con el que se compone el directorio de quadlets por defecto.
///
/// **No** es una variable de podpot y por eso no lleva el prefijo: es la del
/// sistema, y sólo se lee para resolver el defecto de [`QUADLET_DIR_VAR`].
pub const HOME_VAR: &str = "HOME";

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
    /// El directorio del panel estático, si el entorno pidió uno.
    ///
    /// Es lo que dijo `PODPOT_STATIC_DIR`, sin tocar el sistema de ficheros:
    /// para saber si hay panel que servir hay que preguntarle al disco, y eso lo
    /// hace [`Config::confirmar_panel`] —con el registro ya levantado—.
    pub static_dir: Option<PathBuf>,
    /// El directorio canónico en el que se escriben las quadlets nuevas.
    ///
    /// A diferencia del panel, **sí** se valida contra el disco al leerlo: es el
    /// destino de la creación, y un destino que no existe no se puede crear.
    pub quadlet_dir: PathBuf,
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
            static_dir: static_dir(&leer),
            quadlet_dir: quadlet_dir(&leer)?,
        })
    }

    /// Confirma el panel estático: definido **y** existente, o nada.
    ///
    /// El chequeo contra el disco va aquí —y no al leer el entorno— porque el
    /// registro todavía no está en pie cuando la configuración se lee, y el aviso
    /// de un directorio que no existe se daría en el vacío. Se llama una vez, al
    /// arrancar, con el registro ya levantado: un `PODPOT_STATIC_DIR` que apunta a
    /// un directorio que no existe se descarta con un aviso —es un error de
    /// despliegue, y callarlo dejaría un panel en blanco sin explicación— y el
    /// proceso sigue en pie, sólo como API.
    pub fn confirmar_panel(&mut self) {
        let Some(dir) = self.static_dir.as_ref() else {
            return;
        };

        if dir.is_dir() {
            return;
        }

        tracing::warn!(
            directorio = %dir.display(),
            "`{STATIC_DIR_VAR}` apunta a un directorio que no existe: se arranca sólo como API"
        );
        self.static_dir = None;
    }

    /// Un volcado de la configuración para el registro de arranque.
    ///
    /// No lleva la `DATABASE_URL`: es lo único que puede parecerse a una
    /// credencial en un DSN de otro motor, y el proceso tiene que poder publicar
    /// su configuración efectiva sin depender de qué DSN le hayan puesto.
    pub fn describe(&self) -> String {
        format!(
            "bind_address={} systemd={} log_filter={} panel={} quadlets={}",
            self.bind_address,
            self.systemd_backend,
            self.log_filter,
            match &self.static_dir {
                Some(dir) => dir.display().to_string(),
                None => "ausente".to_owned(),
            },
            self.quadlet_dir.display()
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

/// El directorio del panel estático tal y como lo pidió el entorno.
///
/// Es `Some` sólo cuando `PODPOT_STATIC_DIR` está definida; aquí **no** se toca el
/// sistema de ficheros. Que el directorio exista —y que por tanto haya panel que
/// servir— lo confirma [`Config::confirmar_panel`] al arrancar, donde el registro
/// ya está en pie para avisar de un directorio que no existe.
fn static_dir(leer: &impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    Some(PathBuf::from(opcional(leer, STATIC_DIR_VAR)?))
}

/// El directorio de quadlets: el que pida la variable, o el del `$HOME`.
///
/// A diferencia del panel estático, aquí sí se pregunta al disco —y se falla—:
/// el destino de la creación tiene que ser un directorio de verdad, y arrancar
/// con uno imposible sólo serviría para fallar en la primera creación con un
/// mensaje peor. Un `PODPOT_QUADLET_DIR` mal puesto es un error de quien
/// despliega, y el error lo nombra a él y nunca el valor.
fn quadlet_dir(leer: &impl Fn(&str) -> Option<String>) -> Result<PathBuf, ConfigError> {
    // Una variable definida manda: se valida contra el disco y, si no es un
    // directorio, el arranque falla. No se cae en silencio al defecto —quien la
    // puso creía que se iba a usar, y resolverlo a la callada escribiría donde no
    // se pidió—.
    if let Some(ruta) = opcional(leer, QUADLET_DIR_VAR) {
        let ruta = PathBuf::from(ruta);

        if ruta.is_dir() {
            return Ok(ruta);
        }

        return Err(ConfigError::Invalid {
            var: QUADLET_DIR_VAR,
            causa: "el valor no es un directorio existente".to_owned(),
        });
    }

    // Sin variable, el destino cuelga del home del usuario. Sin `$HOME` no hay
    // defecto que componer: se falla nombrando la variable en vez de inventarse
    // un directorio.
    let home = obligatoria(leer, HOME_VAR)?;

    Ok(PathBuf::from(home).join(".config/containers/systemd"))
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::{Config, ConfigError, DEFAULT_BIND_ADDRESS, SystemdBackend};

    /// Un entorno con sólo las variables indicadas: el resto se lee como ausente.
    ///
    /// `HOME` entra siempre, y al final, para que el defecto del directorio de
    /// quadlets se resuelva sin que cada prueba tenga que acordarse: como la
    /// búsqueda se queda con la **primera** coincidencia, una prueba que declare
    /// su propio `HOME` sigue mandando.
    fn fuente(pares: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let pares: Vec<(String, String)> = pares
            .iter()
            .map(|(clave, valor)| ((*clave).to_owned(), (*valor).to_owned()))
            .chain([(super::HOME_VAR.to_owned(), "/home/tester".to_owned())])
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

    /// Requisito «Servicio del panel»: sin `PODPOT_STATIC_DIR` no hay panel y la
    /// configuración sigue siendo válida —la aplicación arranca sólo como API—.
    #[test]
    fn sin_podpot_static_dir_la_app_es_solo_api() {
        let config = Config::from_source(minimo()).expect("la configuración mínima debe leerse");

        assert_eq!(config.static_dir, None);
    }

    /// Requisito «Servicio del panel»: definido **y** existente, se sirve desde ahí.
    #[test]
    fn un_podpot_static_dir_que_existe_se_usa() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let ruta = dir.path().to_str().expect("la ruta de prueba es UTF-8");

        let mut config = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("PODPOT_STATIC_DIR", ruta),
        ]))
        .expect("un directorio de panel existente es configuración válida");

        config.confirmar_panel();

        assert_eq!(config.static_dir.as_deref(), Some(dir.path()));
    }

    /// El entorno pide un panel, pero quien sabe si existe es el disco: leer la
    /// configuración no toca el sistema de ficheros.
    #[test]
    fn podpot_static_dir_se_lee_tal_y_como_llega() {
        let config = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("PODPOT_STATIC_DIR", "/no/existe/este/panel"),
        ]))
        .expect("un panel ausente no puede impedir el arranque");

        assert_eq!(
            config.static_dir.as_deref(),
            Some(std::path::Path::new("/no/existe/este/panel"))
        );
    }

    /// Y al confirmarlo contra el disco, uno que no existe se descarta —con el
    /// aviso que el arranque ya puede dar—: definido pero inexistente **no**
    /// rompe el arranque, se arranca sólo como API.
    #[test]
    fn un_podpot_static_dir_inexistente_se_descarta_al_confirmar() {
        let mut config = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("PODPOT_STATIC_DIR", "/no/existe/este/panel"),
        ]))
        .expect("un panel ausente no puede impedir el arranque");

        config.confirmar_panel();

        assert_eq!(config.static_dir, None);
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
        assert!(
            descrito.contains("panel=ausente"),
            "el volcado tiene que decir si hay panel: {descrito}"
        );
    }

    /// Escenario «Valor por defecto»: sin `PODPOT_QUADLET_DIR` el destino es
    /// `$HOME/.config/containers/systemd`.
    #[test]
    fn sin_podpot_quadlet_dir_el_destino_cuelga_del_home() {
        let config = Config::from_source(minimo()).expect("la configuración mínima debe leerse");

        assert_eq!(
            config.quadlet_dir,
            std::path::Path::new("/home/tester/.config/containers/systemd"),
            "el defecto del directorio de quadlets tiene que ser el del usuario"
        );

        assert!(
            config.describe().contains("quadlets="),
            "el volcado de arranque tiene que decir dónde escribe: {}",
            config.describe()
        );
    }

    /// Sin `$HOME` no hay defecto que componer, y el error lo dice: no se
    /// inventa un directorio.
    #[test]
    fn sin_home_no_hay_directorio_de_quadlets_por_defecto() {
        let error = Config::from_source(|clave| match clave {
            "DATABASE_URL" => Some("sqlite:///tmp/podpot.db".to_owned()),
            _ => None,
        })
        .expect_err("sin HOME no se puede resolver el defecto");

        assert_eq!(
            error,
            ConfigError::Missing { var: "HOME" },
            "el error tiene que nombrar la variable que falta: {error}"
        );
    }

    /// Escenario «Valor no es un directorio»: el arranque falla nombrando la
    /// variable, nunca el valor.
    #[test]
    fn un_podpot_quadlet_dir_que_no_es_directorio_no_arranca() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let fichero = dir.path().join("no-es-un-directorio");
        std::fs::write(&fichero, "no soy un directorio").expect("el fichero de prueba se escribe");
        let ruta = fichero.to_str().expect("la ruta de prueba es UTF-8");

        let error = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("PODPOT_QUADLET_DIR", ruta),
        ]))
        .expect_err("un destino que no es un directorio no puede dar un arranque");

        assert!(
            matches!(
                error,
                ConfigError::Invalid {
                    var: "PODPOT_QUADLET_DIR",
                    ..
                }
            ),
            "se esperaba un error nombrando la variable: {error}"
        );
        assert!(
            error.to_string().contains("PODPOT_QUADLET_DIR"),
            "el mensaje tiene que nombrar la variable: {error}"
        );
        assert!(
            !error.to_string().contains("no-es-un-directorio"),
            "el mensaje no puede publicar el valor: {error}"
        );
    }

    /// Y un `PODPOT_QUADLET_DIR` que no existe tampoco vale: el destino tiene que
    /// estar en pie, porque la creación no inventa directorios.
    #[test]
    fn un_podpot_quadlet_dir_ausente_no_arranca() {
        let error = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("PODPOT_QUADLET_DIR", "/no/existe/esta/carpeta"),
        ]))
        .expect_err("un destino que no existe no puede dar un arranque");

        assert!(
            matches!(
                error,
                ConfigError::Invalid {
                    var: "PODPOT_QUADLET_DIR",
                    ..
                }
            ),
            "se esperaba un error nombrando la variable: {error}"
        );
    }

    /// Escenario «Directorio válido»: existe, se usa tal cual.
    #[test]
    fn un_podpot_quadlet_dir_que_existe_se_usa() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let ruta = dir.path().to_str().expect("la ruta de prueba es UTF-8");

        let config = Config::from_source(fuente(&[
            ("DATABASE_URL", "sqlite:///tmp/podpot.db"),
            ("PODPOT_QUADLET_DIR", ruta),
        ]))
        .expect("un directorio existente es configuración válida");

        assert_eq!(config.quadlet_dir, dir.path());
    }
}
