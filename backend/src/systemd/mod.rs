//! La capa de systemd: el vocabulario de las unidades y el contrato del cliente.
//!
//! Una quadlet **es** una unidad de systemd, así que el estado autoritativo vive
//! en systemd y no en Podman. Este módulo fija cómo se llama una unidad, cuál es
//! su estado y cómo se escapa su nombre para D-Bus; el cliente real y su doble de
//! pruebas viven en los submódulos.

use std::fmt;

use async_trait::async_trait;
use serde::{Serialize, Serializer};
use thiserror::Error;

pub mod dbus;
pub mod fake;

/// Las extensiones de unidad que podpot acepta como nombre de unidad.
///
/// No es una lista de todo lo que systemd conoce, sino de lo que una quadlet
/// puede generar: `.container` da `<stem>.service`, y los tipos con sufijo
/// (`.volume`, `.network`, `.pod`, `.kube`, `.image`, `.build`) dan
/// `<stem>-<tipo>.service`. Las unidades «crudas» (`.socket`, `.mount`, `.timer`)
/// conservan su propio nombre.
const SUFIJOS_DE_UNIDAD: &[&str] = &[
    ".service",
    ".socket",
    ".timer",
    ".mount",
    ".target",
    ".path",
    ".slice",
    ".scope",
    ".device",
    ".automount",
    ".swap",
];

/// El error de la capa de systemd.
///
/// Cada variante distingue un desenlace distinto porque la traducción a HTTP no
/// es la misma: una unidad que no existe es un `404`, un job que falla es un
/// `502` y un bus que no contesta es un `503`. El `Display` —que **sí** puede
/// nombrar la causa entera, porque acaba en el registro— no se publica nunca al
/// cliente: `src/error.rs` construye su propio mensaje para cada variante.
#[derive(Debug, Error)]
pub enum SystemdError {
    /// El nombre no es un nombre de unidad de systemd.
    #[error("`{name}` no es un nombre de unidad de systemd")]
    InvalidUnitName {
        /// El nombre tal y como llegó.
        name: String,
    },

    /// La unidad no existe en el manager.
    #[error("la unidad `{unit}` no existe")]
    UnitNotFound {
        /// Nombre de la unidad.
        unit: String,
    },

    /// El job de la operación terminó sin llegar al estado pedido.
    #[error("la operación `{action}` sobre `{unit}` terminó como `{result}`")]
    OperationFailed {
        /// Nombre de la unidad.
        unit: String,
        /// La acción pedida.
        action: &'static str,
        /// El resultado que informó systemd (`failed`, `timeout`, `canceled`, …).
        result: String,
    },

    /// El gestor rechazó la operación sobre la unidad.
    ///
    /// No es un fallo de transporte: el bus contestó y fue el gestor el que dijo
    /// que no —la unidad está enmascarada, no se puede cargar, la transacción que
    /// pide es destructiva o cíclica…—. Tampoco es culpa de quien llama: el
    /// sistema es el que se niega, y de ahí que sea un `502` y no un `4xx`.
    ///
    /// La causa va detrás de un `Box<dyn Error>` por la misma razón que en
    /// [`SystemdError::Transport`]; el texto no sale al cliente.
    #[error("el gestor rechazó la operación sobre `{unit}`: {causa}")]
    UnitRejected {
        /// Nombre de la unidad.
        unit: String,
        /// La negativa, tal y como la informó el gestor.
        #[source]
        causa: Box<dyn std::error::Error + Send + Sync>,
    },

    /// La operación no terminó dentro del plazo.
    #[error("la operación sobre `{unit}` no terminó en el plazo esperado")]
    TimedOut {
        /// Nombre de la unidad.
        unit: String,
    },

    /// El gestor no pudo recargar su configuración.
    ///
    /// No es un fallo de transporte ni de la petición: el bus contestó y fue el
    /// gestor el que dijo que no podía recargar —o la recarga falló—, así que se
    /// traduce como un `502`, igual que una unidad rechazada.
    #[error("el gestor de systemd no pudo recargar su configuración: {causa}")]
    ReloadFailed {
        /// La negativa, tal y como la informó el gestor.
        #[source]
        causa: Box<dyn std::error::Error + Send + Sync>,
    },

    /// No se pudo hablar con el gestor de systemd.
    ///
    /// La causa va detrás de un `Box<dyn Error>` —y no de un tipo de `zbus`— para
    /// que el doble de pruebas pueda construirla sin una conexión de verdad; el
    /// texto de esa causa puede llevar object paths, así que **no** sale al
    /// cliente: `src/error.rs` responde con un mensaje genérico.
    #[error("no se pudo hablar con el gestor de systemd del usuario: {causa}")]
    Transport {
        /// La causa, tal y como la informó la biblioteca.
        #[source]
        causa: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// El nombre de una unidad de systemd.
///
/// Es un tipo propio y no un `String` porque hay dos cosas que **no** se pueden
/// hacer con él y con un `String` sí: pasarlo crudo a una ruta de D-Bus —para eso
/// está [`UnitName::escaped`]— y aceptar cualquier cosa como unidad. Un
/// `UnitName` sólo existe si se construyó por [`UnitName::parse`], así que quien
/// lo tiene en la mano sabe que el nombre es válido.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnitName(String);

impl UnitName {
    /// Valida y envuelve un nombre de unidad.
    ///
    /// Se exige un sufijo conocido y se rechazan las barras —un nombre de unidad
    /// no lleva rutas, y dejar pasar una convertiría el nombre en una ruta de
    /// D-Bus arbitraria—, los espacios **interiores** y los caracteres de
    /// control: systemd no admite ninguno de los dos en un nombre, y colarlos en
    /// una ruta de D-Bus sería colar basura. No es una lista blanca de lo
    /// permitido: `\`, `@` y `:` son válidos y siguen pasando.
    pub fn parse(name: &str) -> Result<Self, SystemdError> {
        let invalido = || SystemdError::InvalidUnitName {
            name: name.to_owned(),
        };

        if name.is_empty()
            || name.contains('/')
            || name.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(invalido());
        }

        if !SUFIJOS_DE_UNIDAD
            .iter()
            .any(|sufijo| name.ends_with(sufijo))
        {
            return Err(invalido());
        }

        Ok(Self(name.to_owned()))
    }

    /// El nombre tal cual, sin escapar.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// El nombre escapado, tal y como systemd lo usa en las rutas de D-Bus.
    ///
    /// `foo-bar.service` es `/org/freedesktop/systemd1/unit/foo_2dbar_2eservice`.
    /// La regla, comprobada contra el systemd 262 de esta máquina, es que **todo**
    /// byte que no sea `[A-Za-z0-9]` se sustituye por `_` y su código en
    /// hexadecimal minúscula de dos dígitos —de ahí `_2d` para `-`, `_2e` para
    /// `.`, `_40` para `@`, `_5c` para `\`, `_3a` para `:` y `_5f` para el propio
    /// `_`—, que es lo que hace el escape inequívoco.
    pub fn escaped(&self) -> String {
        let mut escapado = String::with_capacity(self.0.len());

        for byte in self.0.bytes() {
            if byte.is_ascii_alphanumeric() {
                escapado.push(char::from(byte));
            } else {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                escapado.push('_');
                escapado.push(char::from(HEX[usize::from(byte >> 4)]));
                escapado.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }

        escapado
    }

    /// La ruta de la unidad en el árbol de objetos del gestor.
    ///
    /// Es la forma escapada ([`UnitName::escaped`]) colgando de
    /// `/org/freedesktop/systemd1/unit/`: `centinela-db.service` es
    /// `/org/freedesktop/systemd1/unit/centinela_2ddb_2eservice`. Es la ruta a
    /// la que systemd expone las propiedades de la unidad, y sólo la construye
    /// este método: el nombre crudo no viaja nunca como ruta.
    pub fn object_path(&self) -> String {
        format!("/org/freedesktop/systemd1/unit/{}", self.escaped())
    }

    /// Envuelve un nombre **sin validarlo**.
    ///
    /// Es la puerta del propio crate para los nombres que controla de antemano
    /// —los datos de demostración del doble de systemd—: pasarlos por
    /// [`UnitName::parse`] obligaría a elegir entre un `expect` (prohibido fuera
    /// de las pruebas) o silenciar un fallo que no puede ocurrir. Es
    /// `pub(crate)`: desde fuera del crate, todo nombre entra por `parse`.
    pub(crate) fn sin_validar(nombre: &str) -> Self {
        Self(nombre.to_owned())
    }
}

impl fmt::Display for UnitName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for UnitName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// El `ActiveState` de una unidad: si está viva, parada o rota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveState {
    /// En ejecución.
    Active,
    /// Parada.
    Inactive,
    /// Falló.
    Failed,
    /// Arrancando.
    Activating,
    /// Parándose.
    Deactivating,
    /// Recargando su configuración.
    Reloading,
    /// En mantenimiento.
    Maintenance,
    /// Cualquier estado que esta versión no conozca, sin perderlo por el camino.
    Unknown(String),
}

impl ActiveState {
    /// Lee el `ActiveState` tal y como lo publica systemd.
    pub fn from_dbus(estado: &str) -> Self {
        match estado {
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            "failed" => Self::Failed,
            "activating" => Self::Activating,
            "deactivating" => Self::Deactivating,
            "reloading" => Self::Reloading,
            "maintenance" => Self::Maintenance,
            otro => Self::Unknown(otro.to_owned()),
        }
    }

    /// El estado tal y como lo publica systemd.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Failed => "failed",
            Self::Activating => "activating",
            Self::Deactivating => "deactivating",
            Self::Reloading => "reloading",
            Self::Maintenance => "maintenance",
            Self::Unknown(otro) => otro,
        }
    }
}

impl fmt::Display for ActiveState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ActiveState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// El `UnitFileState` de una unidad: de dónde viene.
///
/// A podpot sólo le importa una cosa de aquí: que `generated` es lo que delata a
/// una unidad creada por el generador de quadlets, que no es ni `enabled` ni
/// `disabled`. `transient` **no** cuenta como `generated`: una unidad transitoria
/// se crea en tiempo de ejecución y no sale de ningún fichero, así que no puede
/// ser una quadlet —que siempre es un fichero del que el generador **deriva** una
/// unidad—.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitFileState {
    /// Habilitada.
    Enabled,
    /// Deshabilitada.
    Disabled,
    /// Estática: no se habilita, pero se puede arrancar.
    Static,
    /// **Generada** por el generador de quadlets.
    Generated,
    /// Creada en tiempo de ejecución, sin fichero de unidad detrás.
    ///
    /// Es lo que systemd informa de los `scope` de sesión
    /// (`app-…-transient-13530.scope`) y de los `dbus-:1.2-…@0.service`. No sale
    /// de un fichero, así que **no** es una quadlet.
    Transient,
    /// Un alias de otra unidad.
    Alias,
    /// Enmascarada.
    Masked,
    /// Enlazada a un fichero externo.
    Linked,
    /// Indirecta.
    Indirect,
    /// Cualquier estado que esta versión no conozca.
    Unknown(String),
}

impl UnitFileState {
    /// Lee el `UnitFileState` tal y como lo publica systemd.
    pub fn from_dbus(estado: &str) -> Self {
        match estado {
            "enabled" | "enabled-runtime" => Self::Enabled,
            "disabled" => Self::Disabled,
            "static" => Self::Static,
            "generated" => Self::Generated,
            "transient" => Self::Transient,
            "alias" => Self::Alias,
            "masked" | "masked-runtime" => Self::Masked,
            "linked" | "linked-runtime" => Self::Linked,
            "indirect" => Self::Indirect,
            otro => Self::Unknown(otro.to_owned()),
        }
    }

    /// El estado tal y como lo publica systemd.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Static => "static",
            Self::Generated => "generated",
            Self::Transient => "transient",
            Self::Alias => "alias",
            Self::Masked => "masked",
            Self::Linked => "linked",
            Self::Indirect => "indirect",
            Self::Unknown(otro) => otro,
        }
    }

    /// ¿El gestor marca la unidad como generada?
    ///
    /// Es el primer filtro del descubrimiento: las quadlets aparecen como
    /// `generated`, **no** como `enabled` ni `disabled`, así que tratarlas como
    /// unidades normales las dejaría fuera del listado. Una unidad `transient`
    /// —sin fichero— **no** cuenta.
    pub fn is_generated(&self) -> bool {
        matches!(self, Self::Generated)
    }
}

impl fmt::Display for UnitFileState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for UnitFileState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// El fichero del que systemd generó una unidad.
///
/// Existe porque `UnitFileState=generated` **no** basta para reconocer una
/// quadlet: systemd llama `generated` a todo lo que sale de un generador, y de
/// ahí salen también las unidades de inicio automático de escritorio
/// (`app-…@autostart.service`, generadas a partir de un `.desktop`) y las
/// unidades **transitorias** (`app-…-transient-13530.scope`, sin fichero alguno).
/// Las primeras se distinguen por la extensión; las segundas, porque no tienen
/// procedencia: una quadlet **siempre** viene de un fichero.
///
/// **No viaja al cliente**: la API publica nombre y estados, no rutas del
/// sistema de ficheros del usuario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnitOrigin(String);

impl UnitOrigin {
    /// La procedencia, a partir de la ruta que informa el gestor.
    pub fn desde(origen: &str) -> Self {
        Self(origen.to_owned())
    }

    /// Una procedencia que no se pudo leer.
    ///
    /// Es «sin fichero»: una unidad transitoria no tiene fichero de origen, y por
    /// eso mismo **no** es una quadlet (ver [`UnitOrigin::es_fuente_de_quadlet`]).
    pub fn desconocido() -> Self {
        Self(String::new())
    }

    /// La ruta del fichero de origen, tal cual.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// El nombre de fichero de la quadlet: el **basename** de la procedencia.
    ///
    /// Es la identidad con la que operan `PATCH`/`DELETE /api/v1/quadlets/{name}`
    /// —el fichero `<stem>.<kind>`, p. ej. `web.container`—. Se publica **sólo**
    /// este último componente y nunca la ruta entera, que es una ruta del sistema
    /// de ficheros del usuario. Devuelve `None` cuando la procedencia es
    /// desconocida: una unidad transitoria no tiene fichero.
    pub fn file_name(&self) -> Option<&str> {
        if self.es_desconocido() {
            return None;
        }

        std::path::Path::new(&self.0)
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
    }

    /// ¿No se pudo leer la procedencia?
    pub fn es_desconocido(&self) -> bool {
        self.0.is_empty()
    }

    /// ¿Esta procedencia es la de un fichero del que puede salir una quadlet?
    ///
    /// Es la mitad de [`UnitStatus::is_quadlet`] que mira la procedencia; ver
    /// allí por qué `UnitFileState=generated` no basta. Tres casos:
    ///
    /// * **Sin fichero**: no. Es lo que delata a una unidad transitoria, que
    ///   systemd marca como `generated` pero que no salió de ningún fichero.
    /// * **Extensión de quadlet** ([`QUADLET_EXTENSIONS`]): sí.
    /// * **Cualquier otra extensión con fichero**: sí, por prudencia —una
    ///   extensión desconocida puede ser un tipo de quadlet que esta versión aún
    ///   no nombre—, salvo las de escritorio ([`EXTENSIONES_DE_ESCRITORIO`]), que
    ///   salen del mismo generador y **con** fichero pero no son quadlets.
    pub fn es_fuente_de_quadlet(&self) -> bool {
        // Sin fichero no hay quadlet: es una unidad transitoria.
        if self.es_desconocido() {
            return false;
        }

        // Con fichero, una extensión de quadlet lo confirma…
        if es_origen_de_quadlet(self.as_str()) {
            return true;
        }

        // …y una que no conozcamos no lo descarta, salvo las de escritorio.
        !termina_en(self.as_str(), EXTENSIONES_DE_ESCRITORIO)
    }
}

/// El estado de una unidad, tal y como lo informa podpot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnitStatus {
    /// El nombre de la unidad.
    pub name: UnitName,
    /// Si está viva, parada o rota.
    pub active_state: ActiveState,
    /// El subestado (`running`, `dead`, `exited`, …).
    pub sub_state: String,
    /// De dónde viene la unidad: para una quadlet, `generated`.
    pub unit_file_state: UnitFileState,
    /// El fichero del que systemd la generó. No se publica.
    #[serde(skip)]
    pub origin: UnitOrigin,
}

impl UnitStatus {
    /// ¿La unidad la generó un generador de systemd?
    pub fn is_generated(&self) -> bool {
        self.unit_file_state.is_generated()
    }

    /// ¿Es una quadlet de Podman?
    ///
    /// La regla vive aquí, y no en cada cliente, porque los dos tienen que
    /// contestar lo mismo. Dos condiciones, y las dos hacen falta:
    ///
    /// * `generated` **para el gestor**: una quadlet aparece así, no como
    ///   `enabled` ni como `transient`.
    /// * un **fichero de origen** ([`UnitOrigin::es_fuente_de_quadlet`]): una
    ///   quadlet siempre sale de un `.container`/`.volume`/… Una unidad
    ///   transitoria —un `scope` de sesión, un `dbus-…@0.service`— también sale
    ///   como `generated` pero no tiene fichero, así que no es una quadlet.
    ///
    /// Antes bastaba con que el origen fuera desconocido para dejar pasar la
    /// unidad; aquello enseñaba las transitorias como si fueran contenedores.
    pub fn is_quadlet(&self) -> bool {
        self.is_generated() && self.origin.es_fuente_de_quadlet()
    }

    /// El nombre de fichero de la quadlet, derivado de su procedencia.
    ///
    /// Delega en [`UnitOrigin::file_name`]: el basename del `SourcePath` es la
    /// identidad real de la quadlet (`web.container`), y el dato vive aquí —en el
    /// dominio— para poder probarlo con el doble y no recalcularlo en la capa API.
    /// Devuelve `None` cuando la procedencia es desconocida.
    pub fn file_name(&self) -> Option<&str> {
        self.origin.file_name()
    }
}

/// Las tres acciones de control que la API expone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitAction {
    /// Arrancar.
    Start,
    /// Parar.
    Stop,
    /// Reiniciar.
    Restart,
}

impl UnitAction {
    /// La acción a partir del segmento de ruta, o `None` si no es ninguna.
    pub fn parse(accion: &str) -> Option<Self> {
        match accion {
            "start" => Some(Self::Start),
            "stop" => Some(Self::Stop),
            "restart" => Some(Self::Restart),
            _ => None,
        }
    }

    /// La acción como texto, que es lo que viaja en la URL y en el registro.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }

    /// El nombre del método de D-Bus que le corresponde.
    pub const fn dbus_method(&self) -> &'static str {
        match self {
            Self::Start => "StartUnit",
            Self::Stop => "StopUnit",
            Self::Restart => "RestartUnit",
        }
    }
}

/// Las extensiones de fichero con las que Podman reconoce una quadlet.
///
/// Una quadlet no es un tipo de unidad de systemd: es un fichero de Podman del
/// que el generador **deriva** una unidad. De ahí que el nombre de la unidad no
/// coincida con el del fichero (`.container` da `<stem>.service` y `.volume` da
/// `<stem>-volume.service`) y que la procedencia haya que mirarla en el fichero
/// de origen y no en el nombre.
pub const QUADLET_EXTENSIONS: &[&str] = &[
    ".container",
    ".volume",
    ".network",
    ".pod",
    ".kube",
    ".image",
    ".build",
];

/// Las extensiones de fichero de las que sale una unidad **que no** es quadlet.
///
/// Las unidades de inicio automático de escritorio salen del mismo generador
/// (`UnitFileState=generated`) y **con** fichero —el `.desktop`—, así que ni el
/// estado ni la presencia del fichero las distingue: sólo la extensión. Sin esto,
/// el panel listaría cada programa de escritorio como si fuera un contenedor.
const EXTENSIONES_DE_ESCRITORIO: &[&str] = &[".desktop"];

/// ¿El fichero de origen termina en alguna de estas extensiones?
fn termina_en(origen: &str, extensiones: &[&str]) -> bool {
    extensiones
        .iter()
        .any(|extension| origen.ends_with(extension))
}

/// ¿El fichero de origen de una unidad tiene extensión de quadlet?
///
/// Es la parte de [`UnitOrigin::es_fuente_de_quadlet`] que reconoce una quadlet
/// por su extensión; ver allí el caso de las extensiones desconocidas.
fn es_origen_de_quadlet(origen: &str) -> bool {
    termina_en(origen, QUADLET_EXTENSIONS)
}

/// El contrato del cliente de systemd.
///
/// Es un trait y no un tipo concreto porque hay dos formas de hablar con
/// systemd —el bus de usuario de verdad y el doble de desarrollo— y porque las
/// pruebas no pueden depender de que la máquina que compila tenga un systemd de
/// usuario en pie. El estado entra y sale en tipos del dominio
/// ([`UnitStatus`]), nunca en los de `zbus`: así el doble no tiene que fingir un
/// bus y la traducción de errores vive en un solo sitio.
#[async_trait]
pub trait SystemdClient: Send + Sync + fmt::Debug {
    /// Las quadlets del usuario, con su estado.
    ///
    /// Sólo las quadlets: ni las unidades del sistema, ni las de escritorio, ni
    /// las que no genera nadie. Si no hay ninguna, la lista viene vacía y **no**
    /// es un error: un usuario sin quadlets es un usuario con el panel vacío.
    async fn quadlets(&self) -> Result<Vec<UnitStatus>, SystemdError>;

    /// El estado de una unidad concreta.
    async fn status(&self, unit: &UnitName) -> Result<UnitStatus, SystemdError>;

    /// Ejecuta la acción sobre la unidad y **espera** su resultado.
    ///
    /// Devuelve el estado en el que queda la unidad. Las operaciones de systemd
    /// son asíncronas —devuelven un *job*—, así que volver de aquí significa que
    /// el job terminó, no que se haya pedido.
    async fn action(&self, unit: &UnitName, action: UnitAction)
    -> Result<UnitStatus, SystemdError>;

    /// Pide al gestor una recarga de su configuración (`daemon-reload`).
    ///
    /// Es lo que hace visible una quadlet recién escrita: el generador lee los
    /// ficheros cuando el gestor recarga, y hasta entonces la unidad de esa
    /// quadlet no existe. Se pide por D-Bus y no con `systemctl` por la misma
    /// razón que todo lo demás: el binario no está en la imagen y el estado es
    /// mejor hablarlo que parsearlo.
    ///
    /// Cubre también el ciclo de vida de la configuración del propio gestor
    /// —volver a leer todas las unidades—, no sólo las quadlets.
    async fn reload(&self) -> Result<(), SystemdError>;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        ActiveState, SystemdClient, SystemdError, UnitAction, UnitFileState, UnitName, UnitOrigin,
        UnitStatus, es_origen_de_quadlet, fake::FakeSystemd,
    };

    /// Escenario «Nombre con caracteres especiales»: `.` y `-` van escapados a la
    /// ruta de D-Bus.
    #[test]
    fn un_nombre_con_punto_y_guion_se_escapa() {
        let unidad = UnitName::parse("centinela-db.service").expect("es un nombre válido");

        assert_eq!(unidad.escaped(), "centinela_2ddb_2eservice");
        assert_eq!(unidad.as_str(), "centinela-db.service");
    }

    /// El escape cubre todo lo que no sea alfanumérico, con los valores medidos
    /// contra el systemd 262 de esta máquina.
    #[test]
    fn el_escape_cubre_todo_lo_que_no_es_alfanumerico() {
        // El `_` se escapa a `_5f`: sin eso, `_2e` sería ambiguo.
        let unidad = UnitName::parse("ata-CT2000_2152E5F69E4D.device").expect("nombre válido");
        assert_eq!(unidad.escaped(), "ata_2dCT2000_5f2152E5F69E4D_2edevice");

        // `@` y `\` también.
        let unidad =
            UnitName::parse("app-ca\\x2dautostart@autostart.service").expect("nombre válido");
        assert_eq!(
            unidad.escaped(),
            "app_2dca_5cx2dautostart_40autostart_2eservice"
        );
    }

    /// Un nombre sin nada que escapar se queda igual.
    #[test]
    fn un_nombre_llano_no_cambia() {
        let unidad = UnitName::parse("centineladb.service").expect("nombre válido");
        assert_eq!(unidad.escaped(), "centineladb_2eservice");
    }

    /// Requisito «Solo la instancia de usuario»: un nombre de unidad no lleva
    /// barras, así que no puede convertirse en una ruta arbitraria.
    #[test]
    fn un_nombre_con_barra_o_sin_sufijo_se_rechaza() {
        for malo in [
            "",
            "   ",
            "foo",
            "foo.container",
            "/etc/systemd/system/foo.service",
            " foo.service",
            "foo.service ",
        ] {
            let error = UnitName::parse(malo).expect_err("no puede colarse como nombre de unidad");

            assert!(
                matches!(error, SystemdError::InvalidUnitName { .. }),
                "se esperaba un nombre inválido para {malo:?}: {error}"
            );
        }
    }

    /// Un nombre con un espacio **interior** o un carácter de control se rechaza:
    /// el sufijo lo delataría como válido, pero el nombre no lo es, y colarlo en
    /// una ruta de D-Bus sería colar basura.
    ///
    /// No es una lista blanca: `\`, `@` y `:` siguen siendo válidos (ver
    /// [`el_escape_cubre_todo_lo_que_no_es_alfanumerico`]).
    #[test]
    fn un_nombre_con_espacio_interior_o_control_se_rechaza() {
        for malo in [
            "foo bar.service",
            "foo\nbar.service",
            "foo\tbar.service",
            "foo\rbar.service",
            "foo\0bar.service",
        ] {
            let error = UnitName::parse(malo).expect_err("no puede colarse como nombre de unidad");

            assert!(
                matches!(error, SystemdError::InvalidUnitName { .. }),
                "se esperaba un nombre inválido para {malo:?}: {error}"
            );
        }
    }

    /// Los caracteres que systemd sí admite —y que una lista blanca habría
    /// tumbado por error— siguen pasando.
    #[test]
    fn los_caracteres_legitimos_siguen_siendo_validos() {
        for bueno in [
            "app-ca\\x2dautostart@autostart.service",
            "dbus-:1.1-org.gnome.Identity@0.service",
        ] {
            UnitName::parse(bueno).expect("es un nombre de unidad válido");
        }
    }

    /// La unidad de una quadlet sale como `generated` y no como `enabled`.
    #[test]
    fn solo_generated_cuenta_como_quadlet() {
        let generada = unidad("centinela-db.service", ActiveState::Active, "running");

        assert!(generada.is_generated());
        assert!(generada.is_quadlet());

        let habilitada = UnitStatus {
            unit_file_state: UnitFileState::from_dbus("enabled"),
            ..generada.clone()
        };

        assert!(!habilitada.is_generated());
        assert!(!habilitada.is_quadlet());
    }

    /// Una unidad **transitoria** no cuenta como generada.
    ///
    /// Antes `transient` se mapeaba a `Generated`, y con la lenidad de la
    /// procedencia desconocida eso dejaba colarse los `scope` de sesión y los
    /// `dbus-…@0.service` en el listado de quadlets.
    #[test]
    fn una_unidad_transitoria_no_cuenta_como_generada() {
        let transitoria = UnitFileState::from_dbus("transient");

        assert_eq!(transitoria, UnitFileState::Transient);
        assert_eq!(transitoria.as_str(), "transient");
        assert!(
            !transitoria.is_generated(),
            "una unidad transitoria no la generó ninguna quadlet"
        );
    }

    /// Regresión: una unidad `generated` **sin** fichero de origen —una
    /// transitoria— no es una quadlet; una con fichero de quadlet, sí.
    ///
    /// Es el falso positivo que el listado real enseñaba
    /// (`app-ghostty-surface-transient-13530.scope`, `<hash>.timer`): tienen
    /// `UnitFileState=transient`/`generated` y `SourcePath` vacío.
    #[test]
    fn una_unidad_generada_sin_fichero_no_es_una_quadlet() {
        let sin_fichero = UnitStatus {
            origin: UnitOrigin::desconocido(),
            ..unidad(
                "app-ghostty-surface-transient-13530.scope",
                ActiveState::Active,
                "running",
            )
        };

        assert!(sin_fichero.is_generated());
        assert!(
            !sin_fichero.is_quadlet(),
            "una unidad sin SourcePath no puede ser una quadlet"
        );

        let contenedor = UnitStatus {
            origin: UnitOrigin::desde("/x/y.container"),
            ..unidad("centinela-db.service", ActiveState::Active, "running")
        };

        assert!(
            contenedor.is_quadlet(),
            "una unidad generada con un fichero de quadlet sí lo es"
        );
    }

    /// Y las extensiones desconocidas **con** fichero no se descartan: pueden ser
    /// un tipo de quadlet que esta versión aún no nombre.
    #[test]
    fn una_extension_desconocida_con_fichero_no_se_descarta() {
        let futura = UnitStatus {
            origin: UnitOrigin::desde("/home/lorenzo/.config/containers/systemd/algo.quadlet"),
            ..unidad("algo.service", ActiveState::Active, "running")
        };

        assert!(
            futura.is_quadlet(),
            "una extensión desconocida con fichero no puede descartarse"
        );
    }

    /// Requisito «Descubrimiento de quadlets»: `generated` es del gestor, pero no
    /// distingue una quadlet de una unidad de escritorio que también es generada.
    #[test]
    fn una_unidad_generada_de_escritorio_no_es_una_quadlet() {
        let escritorio = UnitStatus {
            origin: UnitOrigin::desde("/home/lorenzo/.config/autostart/opendeck.desktop"),
            ..unidad(
                "app-opendeck@autostart.service",
                ActiveState::Inactive,
                "dead",
            )
        };

        assert!(escritorio.is_generated());
        assert!(
            !escritorio.is_quadlet(),
            "una unidad de escritorio no es una quadlet"
        );

        let contenedor = UnitStatus {
            origin: UnitOrigin::desde(
                "/home/lorenzo/.config/containers/systemd/centinela-db.container",
            ),
            ..unidad("centinela-db.service", ActiveState::Active, "running")
        };

        assert!(contenedor.is_quadlet());
    }

    /// Un `ActiveState` desconocido no se pierde: se conserva tal cual.
    #[test]
    fn un_estado_desconocido_no_se_pierde() {
        assert_eq!(ActiveState::from_dbus("active"), ActiveState::Active);
        assert_eq!(
            ActiveState::from_dbus("cocinando"),
            ActiveState::Unknown("cocinando".to_owned())
        );
        assert_eq!(
            ActiveState::Unknown("cocinando".to_owned()).as_str(),
            "cocinando"
        );
    }

    /// Las tres acciones que expone la API se reconocen; ninguna otra.
    #[test]
    fn las_acciones_reconocidas_son_tres() {
        assert_eq!(UnitAction::parse("start"), Some(UnitAction::Start));
        assert_eq!(UnitAction::parse("stop"), Some(UnitAction::Stop));
        assert_eq!(UnitAction::parse("restart"), Some(UnitAction::Restart));
        assert_eq!(UnitAction::parse("saltar"), None);

        assert_eq!(UnitAction::Start.dbus_method(), "StartUnit");
        assert_eq!(UnitAction::Stop.dbus_method(), "StopUnit");
        assert_eq!(UnitAction::Restart.dbus_method(), "RestartUnit");
    }

    /// Escenario «Nombre con caracteres especiales»: la ruta que se pasa a D-Bus
    /// es la escapada, no el nombre crudo.
    #[test]
    fn la_ruta_de_la_unidad_usa_el_nombre_escapado() {
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        assert_eq!(
            unidad.object_path(),
            "/org/freedesktop/systemd1/unit/centinela_2ddb_2eservice"
        );
    }

    /// Y la procedencia se reconoce por la extensión del fichero de origen: una
    /// unidad de escritorio generada (`generated`, como una quadlet) no lo es.
    #[test]
    fn solo_los_origenes_de_quadlet_cuentan() {
        for quadlet in [
            "/home/lorenzo/.config/containers/systemd/centinela-db.container",
            "/home/lorenzo/.config/containers/systemd/atareao-mariadb-data-volume.volume",
            "/home/lorenzo/.config/containers/systemd/atareao-network.network",
            "/home/lorenzo/.config/containers/systemd/stack.pod",
            "/home/lorenzo/.config/containers/systemd/app.kube",
        ] {
            assert!(es_origen_de_quadlet(quadlet), "{quadlet} es una quadlet");
        }

        for otra in [
            "/home/lorenzo/.config/autostart/opendeck.desktop",
            "/etc/xdg/autostart/blueman.desktop",
            "/etc/fstab",
            "",
        ] {
            assert!(!es_origen_de_quadlet(otra), "{otra} no es una quadlet");
        }
    }

    /// Escenarios «Quadlet con fichero» y «Procedencia desconocida»: el nombre de
    /// fichero es el **basename** de la procedencia —no la ruta entera—, y sólo
    /// los bordes de `Path::file_name()` deciden cuándo no hay nombre.
    ///
    /// La tabla fija los bordes de la regla: una ruta normal, una sin separador,
    /// una terminada en `/` —que `Path::file_name()` resuelve al último
    /// componente, no a vacío— y los orígenes que **no** tienen basename: el
    /// vacío (`desconocido()`), la raíz y el directorio padre.
    #[test]
    fn el_nombre_de_fichero_es_el_basename_de_la_procedencia() {
        let casos: [(&str, Option<&str>); 6] = [
            // Escenario «Quadlet con fichero»: ruta normal.
            (
                "/home/lorenzo/.config/containers/systemd/web.container",
                Some("web.container"),
            ),
            // Una procedencia relativa también tiene basename.
            ("web.container", Some("web.container")),
            // El `/` final lo ignora `Path`: el último componente sigue siendo
            // `systemd`. Se documenta tal cual porque systemd siempre informa un
            // fichero, no un directorio.
            ("/home/lorenzo/.config/containers/systemd/", Some("systemd")),
            // Escenario «Procedencia desconocida»: sin fichero no hay nombre.
            ("", None),
            // La raíz y el padre no tienen componente final.
            ("/", None),
            ("..", None),
        ];

        for (origen, esperado) in casos {
            assert_eq!(
                UnitOrigin::desde(origen).file_name(),
                esperado,
                "procedencia {origen:?}"
            );
        }

        // `desconocido()` es el vacío, y no tiene nombre de fichero.
        assert_eq!(UnitOrigin::desconocido().file_name(), None);
    }

    /// Y el estado lo expone delegando en su procedencia.
    #[test]
    fn el_estado_expone_el_nombre_de_fichero_de_su_procedencia() {
        let estado = UnitStatus {
            origin: UnitOrigin::desde(
                "/home/lorenzo/.config/containers/systemd/centinela-db.container",
            ),
            ..unidad("centinela-db.service", ActiveState::Active, "running")
        };

        assert_eq!(estado.file_name(), Some("centinela-db.container"));
    }

    /// La serialización del estado **no** publica la ruta de la que salió la
    /// unidad —la procedencia sigue omitida—, pero el accesor del dominio sí da
    /// su nombre de fichero.
    #[test]
    fn la_serializacion_del_estado_no_publica_la_ruta_pero_el_accesor_da_el_nombre() {
        let estado = unidad("centinela-db.service", ActiveState::Active, "running");

        let json = serde_json::to_value(&estado).expect("el estado se serializa");

        assert!(
            json.get("origin").is_none(),
            "la procedencia no se publica: {json}"
        );
        assert!(
            !json.to_string().contains("/home/lorenzo"),
            "la ruta del sistema de ficheros no puede viajar: {json}"
        );
        assert_eq!(
            estado.file_name(),
            Some("prueba.container"),
            "el accesor sí da el basename, aunque la ruta no se publique"
        );
    }

    /// Una unidad de prueba: una quadlet —generada y con fichero de origen—. Las
    /// pruebas que miran la procedencia la fijan con la sintaxis de actualización
    /// de estructura.
    fn unidad(nombre: &str, active_state: ActiveState, sub_state: &str) -> UnitStatus {
        UnitStatus {
            name: UnitName::parse(nombre).expect("el nombre de prueba tiene que ser válido"),
            active_state,
            sub_state: sub_state.to_owned(),
            unit_file_state: UnitFileState::Generated,
            origin: UnitOrigin::desde("/home/lorenzo/.config/containers/systemd/prueba.container"),
        }
    }

    /// Escenario «Hay quadlets instaladas»: el listado trae cada quadlet por su
    /// nombre. Lo que no es una quadlet se queda fuera aunque systemd lo llame
    /// `generated` —o aunque sea una unidad transitoria con `SourcePath` vacío—.
    #[tokio::test]
    async fn el_doble_lista_las_quadlets_y_solo_las_quadlets() {
        let cliente = FakeSystemd::con([
            unidad("centinela-db.service", ActiveState::Active, "running"),
            // `generated` también, pero de escritorio: no es una quadlet.
            UnitStatus {
                origin: UnitOrigin::desde("/home/lorenzo/.config/autostart/opendeck.desktop"),
                ..unidad(
                    "app-opendeck@autostart.service",
                    ActiveState::Inactive,
                    "dead",
                )
            },
            // Transitoria de sesión: `SourcePath` vacío, no es una quadlet.
            UnitStatus {
                unit_file_state: UnitFileState::from_dbus("transient"),
                origin: UnitOrigin::desconocido(),
                ..unidad(
                    "app-ghostty-surface-transient-13530.scope",
                    ActiveState::Active,
                    "running",
                )
            },
            // Generada pero sin fichero: tampoco.
            UnitStatus {
                origin: UnitOrigin::desconocido(),
                ..unidad(
                    "14757e563eb9460c713735ab904e0b59f1aac91762104efc23554b7c68ee54da.timer",
                    ActiveState::Active,
                    "running",
                )
            },
            // Una unidad normal: ni siquiera la generó nadie.
            UnitStatus {
                unit_file_state: UnitFileState::Enabled,
                ..unidad("cron.service", ActiveState::Active, "running")
            },
        ]);

        let listado = cliente.quadlets().await.expect("el doble no falla");

        let nombres: Vec<&str> = listado.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(nombres, ["centinela-db.service"]);
    }

    /// Escenario «No hay quadlets»: el listado queda vacío sin error.
    #[tokio::test]
    async fn sin_quadlets_el_listado_queda_vacio_sin_error() {
        let cliente = FakeSystemd::new();

        let listado = cliente
            .quadlets()
            .await
            .expect("un usuario sin quadlets no es un error");

        assert!(
            listado.is_empty(),
            "se esperaba un listado vacío: {listado:?}"
        );
    }

    /// Escenario «Unidad activa»: el estado reportado es activa / en ejecución.
    #[tokio::test]
    async fn una_unidad_activa_se_reporta_activa_y_en_ejecucion() {
        let cliente = FakeSystemd::con([unidad(
            "centinela-db.service",
            ActiveState::Active,
            "running",
        )]);

        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");
        let estado = cliente.status(&unidad).await.expect("la unidad existe");

        assert_eq!(estado.active_state, ActiveState::Active);
        assert_eq!(estado.sub_state, "running");
        assert!(estado.is_generated());
    }

    /// Escenario «Arrancar una unidad inactiva»: la operación se completa y la
    /// unidad queda activa.
    #[tokio::test]
    async fn arrancar_una_unidad_inactiva_la_deja_activa() {
        let cliente = FakeSystemd::con([unidad(
            "centinela-db.service",
            ActiveState::Inactive,
            "dead",
        )]);

        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");
        let estado = cliente
            .action(&unidad, UnitAction::Start)
            .await
            .expect("arrancar una unidad que existe no falla");

        assert_eq!(estado.active_state, ActiveState::Active);

        // Y no es sólo lo que devuelve la acción: es lo que queda.
        let despues = cliente.status(&unidad).await.expect("la unidad existe");
        assert_eq!(despues.active_state, ActiveState::Active);
    }

    /// Parar una unidad activa la deja parada.
    #[tokio::test]
    async fn parar_una_unidad_activa_la_deja_inactiva() {
        let cliente = FakeSystemd::con([unidad(
            "centinela-db.service",
            ActiveState::Active,
            "running",
        )]);

        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");
        let estado = cliente
            .action(&unidad, UnitAction::Stop)
            .await
            .expect("parar una unidad que existe no falla");

        assert_eq!(estado.active_state, ActiveState::Inactive);
    }

    /// Reiniciar una unidad activa la deja activa.
    #[tokio::test]
    async fn reiniciar_una_unidad_activa_la_deja_activa() {
        let cliente = FakeSystemd::con([unidad(
            "centinela-db.service",
            ActiveState::Active,
            "running",
        )]);

        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");
        let estado = cliente
            .action(&unidad, UnitAction::Restart)
            .await
            .expect("reiniciar una unidad que existe no falla");

        assert_eq!(estado.active_state, ActiveState::Active);
    }

    /// Escenario «Unidad inexistente»: el error es tipado y nombra la unidad.
    #[tokio::test]
    async fn una_accion_sobre_una_unidad_que_no_existe_nombra_la_unidad() {
        let cliente = FakeSystemd::new();

        let fantasma = UnitName::parse("fantasma.service").expect("nombre válido");
        let error = cliente
            .action(&fantasma, UnitAction::Start)
            .await
            .expect_err("no se puede accionar una unidad que no existe");

        assert!(
            matches!(&error, SystemdError::UnitNotFound { unit } if unit == "fantasma.service"),
            "se esperaba «no existe» nombrando la unidad: {error}"
        );
    }

    /// Y preguntar por su estado falla igual, no informa de una unidad parada.
    #[tokio::test]
    async fn el_estado_de_una_unidad_que_no_existe_nombra_la_unidad() {
        let cliente = FakeSystemd::new();

        let fantasma = UnitName::parse("fantasma.service").expect("nombre válido");
        let error = cliente
            .status(&fantasma)
            .await
            .expect_err("una unidad que no existe no tiene estado que informar");

        assert!(
            matches!(&error, SystemdError::UnitNotFound { unit } if unit == "fantasma.service"),
            "se esperaba «no existe» nombrando la unidad: {error}"
        );
    }

    /// El trait tiene que valer como objeto: el estado de la aplicación guarda un
    /// `Arc<dyn SystemdClient>`, y el doble se inyecta por ahí.
    #[tokio::test]
    async fn el_cliente_vale_detras_de_un_arco() {
        let cliente: Arc<dyn SystemdClient> = Arc::new(FakeSystemd::new());

        assert!(
            cliente
                .quadlets()
                .await
                .expect("el doble no falla")
                .is_empty()
        );
    }
}
