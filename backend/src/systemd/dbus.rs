//! El cliente de systemd que habla por D-Bus.
//!
//! Va contra `org.freedesktop.systemd1` del bus **de usuario**: el mismo gestor
//! que atiende `systemctl --user`. Podpot no invoca `systemctl` —ni ningún otro
//! binario— porque el estado de una unidad es mejor leerlo tipado que parsearlo,
//! y porque la imagen no tiene por qué llevar el binario dentro.
//!
//! Dos cosas de systemd marcan todo lo que hay aquí:
//!
//! * Los nombres de unidad **se escapan** al construir una ruta
//!   ([`UnitName::object_path`]): `centinela-db.service` vive en
//!   `/org/freedesktop/systemd1/unit/centinela_2ddb_2eservice`.
//! * Las operaciones son **asíncronas**: `StartUnit`, `StopUnit` y `RestartUnit`
//!   devuelven un *job* y sólo el aviso de que el job terminó dice si la unidad
//!   llegó a su estado.

use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use zbus::proxy::SignalStream;
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, Proxy};

use super::{
    ActiveState, SystemdClient, SystemdError, UnitAction, UnitFileState, UnitName, UnitOrigin,
    UnitStatus,
};

/// El servicio del gestor de systemd.
const DESTINO: &str = "org.freedesktop.systemd1";

/// La ruta del objeto del gestor.
const RUTA_MANAGER: &str = "/org/freedesktop/systemd1";

/// La interfaz del gestor.
const IFACE_MANAGER: &str = "org.freedesktop.systemd1.Manager";

/// La interfaz de una unidad.
const IFACE_UNIT: &str = "org.freedesktop.systemd1.Unit";

/// El error con el que systemd dice que la unidad no está cargada.
const ERROR_NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnit";

/// El error con el que el gestor dice que no conoce el fichero de la unidad.
///
/// Medido contra el systemd 262 de esta máquina: `GetUnitFileState` sobre una
/// unidad que no existe no devuelve `NoSuchUnit` —eso lo devuelven `StartUnit` y
/// compañía— sino `FileNotFound`. Para podpot los dos significan lo mismo.
const ERROR_FILE_NOT_FOUND: &str = "org.freedesktop.DBus.Error.FileNotFound";

/// El error con el que el gestor dice que esa unidad no tiene esa propiedad.
///
/// Es lo que contesta por el `SourcePath` de una unidad transitoria: existe, pero
/// no la generó ningún fichero.
const ERROR_UNKNOWN_PROPERTY: &str = "org.freedesktop.DBus.Error.UnknownProperty";

/// El prefijo de los errores propios del gestor de systemd.
///
/// Todo lo que el gestor contesta por su cuenta —unidad enmascarada, que no se
/// puede cargar, transacción destructiva o cíclica…— va bajo su espacio de
/// nombres (`org.freedesktop.systemd1.*`). Es lo que distingue «el gestor dijo
/// que no» de «no hubo respuesta que traducir».
const PREFIJO_ERROR_SYSTEMD: &str = "org.freedesktop.systemd1.";

/// Los errores que significan «esa unidad —o esa propiedad— no está».
const ERRORES_DE_AUSENCIA: &[&str] = &[
    ERROR_NO_SUCH_UNIT,
    ERROR_FILE_NOT_FOUND,
    ERROR_UNKNOWN_PROPERTY,
];

/// El modo con el que se pide una operación; `replace` es el que usa `systemctl`.
const MODO: &str = "replace";

/// Cuánto se espera a que un job termine antes de darlo por perdido.
///
/// Un arranque de contenedor puede tardar —descargar una imagen, por ejemplo—,
/// así que el plazo es generoso. Pero un plazo existe: sin él, una unidad que se
/// quedara en `activating` para siempre dejaría la petición colgada.
const ESPERA_DEL_JOB: Duration = Duration::from_secs(60);

/// Una entrada del listado de unidades de systemd.
///
/// La firma de D-Bus de `ListUnits` es `a(ssssssouso)`: nombre, descripción,
/// `LoadState`, `ActiveState`, `SubState`, `Following`, ruta del objeto y el job
/// (identificador, tipo, ruta). Podpot sólo mira tres de los diez, así que se
/// recorre como la tupla que es —y no con una estructura de diez campos, siete de
/// ellos muertos— y se desestructura nombrando lo que se usa.
type Entrada = (
    String,
    String,
    String,
    String,
    String,
    String,
    OwnedObjectPath,
    u32,
    String,
    OwnedObjectPath,
);

/// El cliente de systemd por D-Bus contra el bus de usuario.
#[derive(Debug)]
pub struct DbusSystemdClient {
    conexion: Connection,
}

impl DbusSystemdClient {
    /// Se conecta al bus **de usuario**.
    ///
    /// `Connection::session()` es el bus de sesión, que en systemd es el del
    /// usuario: se resuelve por `DBUS_SESSION_BUS_ADDRESS` y, si no está, por
    /// `$XDG_RUNTIME_DIR/bus` —nunca el bus del sistema—. Ésa es la mitad del
    /// requisito de ámbito; la otra mitad es que el destino sea el
    /// `org.freedesktop.systemd1` de *ese* bus, que es el gestor de
    /// `systemctl --user`.
    pub async fn connect_user() -> Result<Self, SystemdError> {
        let conexion = Connection::session().await.map_err(transporte)?;

        Ok(Self { conexion })
    }

    /// El proxy del gestor de systemd.
    async fn manager(&self) -> Result<Proxy<'_>, SystemdError> {
        Proxy::new(&self.conexion, DESTINO, RUTA_MANAGER, IFACE_MANAGER)
            .await
            .map_err(transporte)
    }

    /// El proxy de una unidad, en su ruta **escapada**.
    async fn unidad(&self, unit: &UnitName) -> Result<Proxy<'_>, SystemdError> {
        Proxy::new(&self.conexion, DESTINO, unit.object_path(), IFACE_UNIT)
            .await
            .map_err(transporte)
    }

    /// El fichero del que systemd generó la unidad, si lo publica.
    ///
    /// Se lee `SourcePath` porque `UnitFileState=generated` no basta para
    /// reconocer una quadlet: entre las generadas también están las unidades de
    /// inicio automático de escritorio, que salen de un `.desktop`, y las
    /// **transitorias**, que no salen de ningún fichero. Cuando el origen no se
    /// puede leer —una unidad transitoria contesta `UnknownProperty`— se devuelve
    /// el origen desconocido, y [`UnitStatus::is_quadlet`] **descarta** la unidad:
    /// una quadlet siempre viene de un fichero, así que sin fichero no hay quadlet.
    async fn origen(&self, unit: &UnitName) -> Result<UnitOrigin, SystemdError> {
        let proxy = self.unidad(unit).await?;

        match proxy.get_property::<String>("SourcePath").await {
            Ok(origen) => Ok(UnitOrigin::desde(&origen)),
            Err(error) if es_ausencia(&error) => Ok(UnitOrigin::desconocido()),
            Err(error) => Err(transporte(error)),
        }
    }
}

#[async_trait]
impl SystemdClient for DbusSystemdClient {
    async fn quadlets(&self) -> Result<Vec<UnitStatus>, SystemdError> {
        let manager = self.manager().await?;

        let listado: Vec<Entrada> = manager.call("ListUnits", &()).await.map_err(transporte)?;

        let mut quadlets = Vec::new();

        for entrada in listado {
            let (nombre, _, _, active_state, sub_state, _, _, _, _, _) = entrada;

            let unit_file_state = match manager
                .call::<_, _, String>("GetUnitFileState", &(nombre.as_str(),))
                .await
            {
                Ok(estado) => UnitFileState::from_dbus(&estado),
                // La unidad se descargó entre el listado y esta llamada: no está,
                // y una unidad que no está no es una quadlet que listar.
                Err(error) if es_ausencia(&error) => continue,
                Err(error) => return Err(transporte(error)),
            };

            if !unit_file_state.is_generated() {
                continue;
            }

            let Ok(name) = UnitName::parse(&nombre) else {
                tracing::warn!(
                    unidad = %nombre,
                    "systemd informó de una unidad con un nombre que podpot no reconoce"
                );
                continue;
            };

            let status = UnitStatus {
                name: name.clone(),
                active_state: ActiveState::from_dbus(&active_state),
                sub_state,
                unit_file_state,
                origin: self.origen(&name).await?,
            };

            // La decisión de qué es una quadlet no se toma aquí: se toma en
            // `UnitStatus::is_quadlet`, que es la misma que toma el doble.
            if !status.is_quadlet() {
                continue;
            }

            quadlets.push(status);
        }

        // El orden del gestor no es estable entre llamadas; el del panel sí tiene
        // que serlo.
        quadlets.sort_by(|izquierda, derecha| izquierda.name.cmp(&derecha.name));

        Ok(quadlets)
    }

    async fn status(&self, unit: &UnitName) -> Result<UnitStatus, SystemdError> {
        let manager = self.manager().await?;

        // La existencia se pregunta al gestor en vez de deducirse de una lectura
        // de propiedades: por una ruta de unidad que no existe, systemd contesta
        // `inactive` igual que por una unidad parada, así que una lectura de
        // propiedades no distingue «no está» de «está parada». De paso, la
        // respuesta trae el `UnitFileState`.
        let unit_file_state: String = manager
            .call("GetUnitFileState", &(unit.as_str(),))
            .await
            .map_err(|error| traducir(unit, error))?;

        let proxy = self.unidad(unit).await?;

        let active_state: String = proxy
            .get_property("ActiveState")
            .await
            .map_err(transporte)?;
        let sub_state: String = proxy.get_property("SubState").await.map_err(transporte)?;

        Ok(UnitStatus {
            name: unit.clone(),
            active_state: ActiveState::from_dbus(&active_state),
            sub_state,
            unit_file_state: UnitFileState::from_dbus(&unit_file_state),
            origin: self.origen(unit).await?,
        })
    }

    async fn action(
        &self,
        unit: &UnitName,
        action: UnitAction,
    ) -> Result<UnitStatus, SystemdError> {
        let manager = self.manager().await?;

        // El flujo de señales se abre **antes** de pedir la operación. Al revés
        // hay una carrera: si el job termina entre la llamada y la suscripción,
        // su `JobRemoved` se pierde y la petición espera hasta el plazo teniendo
        // el resultado delante.
        let mut senales = manager
            .receive_signal("JobRemoved")
            .await
            .map_err(transporte)?;

        let job: OwnedObjectPath = manager
            .call(action.dbus_method(), &(unit.as_str(), MODO))
            .await
            .map_err(|error| traducir(unit, error))?;

        let resultado = esperar_al_job(&mut senales, &job, unit).await?;

        // El job terminó, pero «terminar» no es «salir bien»: `done` es el único
        // resultado que significa que la unidad llegó al estado pedido.
        if resultado != "done" {
            return Err(SystemdError::OperationFailed {
                unit: unit.as_str().to_owned(),
                action: action.as_str(),
                result: resultado,
            });
        }

        self.status(unit).await
    }

    async fn reload(&self) -> Result<(), SystemdError> {
        let manager = self.manager().await?;

        // `Reload` es el `daemon-reload` de `systemctl`: no lleva unidad y no
        // devuelve nada. Un error del gestor aquí **no** es «eso no existe»
        // —no hay unidad que pueda faltar—, así que se traduce como una recarga
        // fallida y no como una ausencia.
        manager
            .call::<_, _, ()>("Reload", &())
            .await
            .map_err(|error| recarga(clase(&error), error))
    }
}

/// Espera a que terminen el job indicado y devuelve su resultado.
///
/// `JobRemoved` se emite para **todos** los jobs del gestor, así que se compara
/// la ruta del job y no el nombre de la unidad: entre medias puede terminar otro
/// job de la misma unidad y adelantar un resultado que no es el suyo.
async fn esperar_al_job(
    senales: &mut SignalStream<'_>,
    job: &OwnedObjectPath,
    unit: &UnitName,
) -> Result<String, SystemdError> {
    let espera = async {
        while let Some(mensaje) = senales.next().await {
            let cuerpo = mensaje.body();

            let (_id, ruta, _unidad, resultado): (u32, OwnedObjectPath, String, String) =
                cuerpo.deserialize().map_err(transporte)?;

            if ruta.as_str() == job.as_str() {
                return Ok(resultado);
            }
        }

        Err(SystemdError::Transport {
            causa: Box::new(std::io::Error::other(
                "el bus de usuario cerró la conexión antes de que el job terminara",
            )),
        })
    };

    match tokio::time::timeout(ESPERA_DEL_JOB, espera).await {
        Ok(resultado) => resultado,
        Err(_) => Err(SystemdError::TimedOut {
            unit: unit.as_str().to_owned(),
        }),
    }
}

/// Un fallo de transporte: no se pudo hablar con el gestor.
fn transporte(causa: impl std::error::Error + Send + Sync + 'static) -> SystemdError {
    SystemdError::Transport {
        causa: Box::new(causa),
    }
}

/// Traduce un error de D-Bus al de la capa.
///
/// La traducción separa tres desenlaces que el bus mete en el mismo tipo: que la
/// unidad no esté (`UnitNotFound`, y de ahí a un `404`), que el gestor haya
/// contestado que **no** a la operación (`UnitRejected`, y de ahí a un `502`) y
/// que no haya habido respuesta que traducir (`Transport`, y de ahí a un `503`).
/// El detalle del error no viaja nunca al cliente: lo consume el registro de
/// `src/error.rs`.
fn traducir(unit: &UnitName, error: zbus::Error) -> SystemdError {
    match clase(&error) {
        Clase::Ausencia => SystemdError::UnitNotFound {
            unit: unit.as_str().to_owned(),
        },
        Clase::Rechazo => SystemdError::UnitRejected {
            unit: unit.as_str().to_owned(),
            causa: Box::new(error),
        },
        Clase::Transporte => transporte(error),
    }
}

/// Traduce el fallo de una recarga de la configuración del gestor.
///
/// No hay unidad de la que hablar: la recarga es del gestor entero, así que lo
/// único que se distingue es si hubo respuesta —el gestor dijo que no, y eso es
/// una recarga fallida, un `502`— o si no la hubo, que es un transporte y de ahí
/// a un `503`.
fn recarga(clase: Clase, error: zbus::Error) -> SystemdError {
    match clase {
        Clase::Transporte => transporte(error),
        Clase::Ausencia | Clase::Rechazo => SystemdError::ReloadFailed {
            causa: Box::new(error),
        },
    }
}

/// A qué desenlace corresponde un fallo de D-Bus.
///
/// Aislar la decisión en un enumerado —y no resolverla con dos `if` sobre
/// [`zbus::Error`]— es lo que la hace medible: la clase depende sólo del nombre
/// del error, así que se puede comprobar sin fabricar un `Message` de D-Bus ni
/// levantar un bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clase {
    /// La unidad —o la propiedad— no está.
    Ausencia,
    /// El gestor contestó y dijo que no.
    Rechazo,
    /// No hubo respuesta que traducir.
    Transporte,
}

/// La clase de un fallo de D-Bus.
fn clase(error: &zbus::Error) -> Clase {
    match error {
        zbus::Error::MethodError(nombre, _, _) => clase_del_nombre(nombre.as_str()),
        // Lo que no es un error de método no lo dijo el gestor: un `InputOutput`
        // o un `Connection` son el bus que no llegó a contestar.
        _ => Clase::Transporte,
    }
}

/// La clase que corresponde al nombre de un error de D-Bus.
fn clase_del_nombre(nombre: &str) -> Clase {
    if ERRORES_DE_AUSENCIA.contains(&nombre) {
        return Clase::Ausencia;
    }

    // Cualquier otro error del espacio de nombres del gestor es una negativa
    // suya: `UnitMasked`, `LoadFailed`, `TransactionIsDestructive`,
    // `TransactionOrderIsCyclic`, `UnitExists`… La lista no se cierra a mano
    // para no traducir como avería de red un rechazo que systemd estrene.
    if nombre.starts_with(PREFIJO_ERROR_SYSTEMD) {
        return Clase::Rechazo;
    }

    Clase::Transporte
}

/// ¿El error de D-Bus significa «esa unidad —o esa propiedad— no está»?
fn es_ausencia(error: &zbus::Error) -> bool {
    matches!(clase(error), Clase::Ausencia)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        Clase, ERROR_FILE_NOT_FOUND, ERROR_NO_SUCH_UNIT, ERROR_UNKNOWN_PROPERTY, clase,
        clase_del_nombre, recarga,
    };
    use crate::systemd::SystemdError;

    /// Unidad que no está: los tres códigos que systemd usa para decirlo caen en
    /// la misma clase, que es lo que permite dar un solo `404`.
    #[test]
    fn los_errores_de_ausencia_son_ausencia() {
        for nombre in [
            ERROR_NO_SUCH_UNIT,
            ERROR_FILE_NOT_FOUND,
            ERROR_UNKNOWN_PROPERTY,
        ] {
            assert_eq!(clase_del_nombre(nombre), Clase::Ausencia, "{nombre}");
        }
    }

    /// Unidad enmascarada, que no se puede cargar, transacción destructiva o
    /// cíclica: el gestor contestó que no, y eso es un rechazo —un `502`—, no un
    /// fallo de transporte.
    #[test]
    fn los_errores_propios_del_gestor_son_rechazo() {
        for nombre in [
            "org.freedesktop.systemd1.UnitMasked",
            "org.freedesktop.systemd1.LoadFailed",
            "org.freedesktop.systemd1.TransactionIsDestructive",
            "org.freedesktop.systemd1.TransactionOrderIsCyclic",
            "org.freedesktop.systemd1.UnitExists",
        ] {
            assert_eq!(clase_del_nombre(nombre), Clase::Rechazo, "{nombre}");
        }
    }

    /// Un error del bus —y no del gestor— no es una negativa a operar: se queda
    /// como transporte. La lista de rechazos no se ensancha a todo lo que conteste
    /// el bus, sólo al espacio de nombres de systemd.
    #[test]
    fn los_errores_del_bus_no_son_rechazo() {
        assert_eq!(
            clase_del_nombre("org.freedesktop.DBus.Error.AccessDenied"),
            Clase::Transporte
        );
        assert_eq!(
            clase_del_nombre("org.freedesktop.DBus.Error.Disconnected"),
            Clase::Transporte
        );
    }

    /// Un error que no es un error de método —el bus que no llegó a contestar— no
    /// es un rechazo: es un transporte.
    #[test]
    fn un_fallo_de_io_es_transporte() {
        let error = zbus::Error::InputOutput(Arc::new(std::io::Error::other("sin bus")));

        assert_eq!(clase(&error), Clase::Transporte);
    }

    /// Una recarga que el gestor rechaza no es un transporte: hubo respuesta, y
    /// el desenlace es una recarga fallida —un `502`—, no un bus caído.
    #[test]
    fn una_recarga_que_el_gestor_rechaza_no_es_un_transporte() {
        let error = zbus::Error::InputOutput(Arc::new(std::io::Error::other("el gestor dijo no")));

        assert!(
            matches!(
                recarga(Clase::Rechazo, error),
                SystemdError::ReloadFailed { .. }
            ),
            "el gestor contestó que no: no puede tratarse como un bus caído"
        );
    }

    /// Y una recarga sin respuesta sigue siendo un transporte.
    #[test]
    fn una_recarga_sin_respuesta_es_un_transporte() {
        let error = zbus::Error::InputOutput(Arc::new(std::io::Error::other("sin bus")));

        assert!(matches!(
            recarga(Clase::Transporte, error),
            SystemdError::Transport { .. }
        ));
    }
}
