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

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{self, Stream, StreamExt};
use tokio::sync::{broadcast, mpsc, watch};
use zbus::message::Type;
use zbus::proxy::SignalStream;
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, MatchRule, MessageStream, Proxy};

use super::{
    ActiveState, ProgresoDeJob, ProgresoDeTrabajos, ResultadoDeJob, SystemdClient, SystemdError,
    UnitAction, UnitFileState, UnitName, UnitOrigin, UnitStatus, UnitWatcher,
};

/// El servicio del gestor de systemd.
const DESTINO: &str = "org.freedesktop.systemd1";

/// La ruta del objeto del gestor.
const RUTA_MANAGER: &str = "/org/freedesktop/systemd1";

/// La interfaz del gestor.
const IFACE_MANAGER: &str = "org.freedesktop.systemd1.Manager";

/// La interfaz de una unidad.
const IFACE_UNIT: &str = "org.freedesktop.systemd1.Unit";

/// La interfaz del objeto de un **job**.
///
/// Es donde vive la propiedad [`PROPIEDAD_JOB_TYPE`], que es el único sitio del que
/// se puede sacar el tipo de job: la señal de alta **no** lo trae (ver
/// [`tipo_de_job`]).
const IFACE_JOB: &str = "org.freedesktop.systemd1.Job";

/// La propiedad del objeto de un job que dice de qué tipo es (`start`, `stop`, …).
///
/// Es `const`, así que se puede leer una vez y vale para toda la vida del job; y es
/// la única fuente: ni `JobNew` ni `JobRemoved` la llevan en su cuerpo.
const PROPIEDAD_JOB_TYPE: &str = "JobType";

/// La interfaz estándar por la que cualquier objeto de D-Bus publica sus cambios
/// de propiedades.
const IFACE_PROPIEDADES: &str = "org.freedesktop.DBus.Properties";

/// La señal con la que esa interfaz anuncia que una propiedad cambió.
const SENAL_PROPIEDADES: &str = "PropertiesChanged";

/// El espacio de rutas bajo el que cuelgan **todas** las unidades.
///
/// Es lo que hace que una sola regla de D-Bus cubra cualquier unidad: podpot no
/// puede observar una por una —no sabe cuáles hay hasta que las lista, y una
/// quadlet nueva aparece sin avisar—, así que escucha el prefijo entero.
///
/// Va **sin barra final**: el bus compara un espacio de rutas contra la ruta del
/// mensaje como «igual, o prefijo seguido de `/`», así que
/// `/org/freedesktop/systemd1/unit` cubre `/org/freedesktop/systemd1/unit/x_2eservice`
/// sin dejar fuera la ruta del propio espacio. Añadir la barra no cambiaría el
/// filtro del bus, pero `zbus` valida el valor como ruta de objeto y una barra
/// final no lo es.
const ESPACIO_DE_UNIDADES: &str = "/org/freedesktop/systemd1/unit";

/// Cuántos avisos de cambio caben en el canal del vigilante antes de perder el
/// más viejo.
///
/// Perder un aviso no rompe nada —dispara un recálculo, y el recálculo es siempre
/// de la instantánea entera—, pero la capacidad no puede ser cero: con cero no
/// habría avisos y el panel se quedaría congelado sin que nada lo delatara.
const CAPACIDAD_DE_AVISOS: usize = 32;

/// Cuántas señales crudas caben antes de que el reenvío tenga que esperar.
///
/// Una ráfaga —un `daemon-reload` con muchas unidades— cabe de sobra; si no
/// cupiera, el reenvío esperaría y el bus acumularía, que es lo correcto: no se
/// pierde ninguna señal.
const CAPACIDAD_DE_SENALES: usize = 256;

/// Cuántos eventos de progreso de job caben antes de perder el más viejo.
///
/// Perder uno no rompe el panel —los jobs en curso de un usuario caben de sobra—,
/// y el estado final llega igual por el canal de unidades; pero con cero no habría
/// ningún evento y el canal sería un decorado.
const CAPACIDAD_DE_TRABAJOS: usize = 64;

/// La señal con la que el gestor anuncia que ha creado un job.
const SENAL_JOB_NEW: &str = "JobNew";

/// La señal con la que el gestor anuncia que ha quitado un job.
const SENAL_JOB_REMOVED: &str = "JobRemoved";

/// El tipo que se publica cuando el tipo del job no se puede saber.
///
/// Son dos casos, y los dos pasan de verdad:
///
/// * **no se le vio nacer** —el vigilante se resuscribe cuando el bus cierra el
///   stream (una caída, un corte del socket), y un job que ya estuviera en curso
///   entonces no tiene `JobNew` en la conexión nueva—;
/// * **nació y murió antes de poder preguntar** —el tipo vive en el objeto del job,
///   no en la señal, así que se pide aparte; si el job fue tan corto que el objeto
///   ya no estaba cuando se preguntó, no hay tipo que publicar—.
///
/// Publicar `unknown` es más honesto que inventarse el tipo —que era `start` o
/// `stop`, pero no se sabe— y mejor que no publicar nada: el fin del job sí se sabe.
const KIND_DESCONOCIDO: &str = "unknown";

/// Cuánto silencio convierte una ráfaga de señales en un solo aviso.
///
/// Un `daemon-reload` emite decenas de señales seguidas; sin este plazo, cada una
/// sería un recálculo del listado entero. Con él, la ráfaga se convierte en un
/// aviso y el coste es una latencia de un cuarto de segundo —imperceptible para
/// un panel— a cambio de no recalcular en vano.
const DEBOUNCE: Duration = Duration::from_millis(250);

/// Cuánto se espera antes del primer reintento de la suscripción.
///
/// Corto a propósito: lo normal es que un cierre del stream sea un parpadeo del
/// bus, y con esta espera el panel vuelve a refrescarse casi sin enterarse.
const REINTENTO_INICIAL: Duration = Duration::from_secs(1);

/// El tope de la espera entre reintentos.
///
/// Sin tope, un bus que no vuelve haría esperas cada vez más largas y el log se
/// llenaría de avisos cada vez más espaciados; con tope, el coste de un bus muerto
/// es un intento cada medio minuto —suficiente para volver solo cuando el bus
/// regrese, y barato mientras no lo haga—.
const REINTENTO_MAXIMO: Duration = Duration::from_secs(30);

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

/// El error con el que el gestor contesta que la conexión **ya** está suscrita.
///
/// `Subscribe()` se pide **por conexión**, no por regla, y podpot tiene dos tareas
/// escuchando por la misma conexión —el vigilante de unidades y el de trabajos—: la
/// segunda en llamar recibe esto, que no es un fallo sino la confirmación de que ya
/// se reciben señales, que es justo lo que se iba a pedir.
const ERROR_YA_SUSCRITO: &str = "org.freedesktop.systemd1.AlreadySubscribed";

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

/// Cuánto se espera a que el gestor conteste por el `JobType` de un job.
///
/// Es una consulta **de adorno**: su resultado sólo mejora el evento —si no llega,
/// el job sale con `unknown`— y quien la hace no puede quedarse esperándola. Un
/// plazo corto y explícito evita depender del que traiga puesto el bus por defecto
/// (que puede ser «ninguno») y que un gestor atascado deje la escucha de jobs
/// parada detrás de una consulta que no vuelve.
const ESPERA_DEL_TIPO: Duration = Duration::from_secs(2);

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
    /// El canal por el que el vigilante avisa de los cambios de unidades.
    ///
    /// Es `broadcast` y no `mpsc` porque los suscriptores son varios y ninguno
    /// manda a los demás: el fan-out escucha para recalcular y publicar, y una
    /// prueba de integración escucha para medir el camino entero.
    avisos: broadcast::Sender<()>,
    /// El canal por el que el vigilante publica el ciclo de vida de los jobs.
    ///
    /// Es un canal **aparte** del de avisos y no el mismo: un aviso de cambio de
    /// unidad no lleva datos y dispara un recálculo, mientras que un evento de job
    /// ya es el dato y no hay nada que recalcular. También es `broadcast` porque
    /// los suscriptores son varios —cada cliente del canal de trabajos— y ninguno
    /// manda a los demás.
    trabajos: broadcast::Sender<ProgresoDeJob>,
    /// El cierre del vigilante: cuando vale `true`, la tarea de escucha termina.
    ///
    /// El disparador vive en el cliente —y no lo comparte con el cierre del
    /// servidor— porque el vigilante nace en [`DbusSystemdClient::connect_user`],
    /// antes de que existan los canales de la aplicación: el cliente no puede
    /// depender de un canal que todavía no está construido. Se dispara al soltarse
    /// el cliente, que es exactamente cuando ya no hay a quién avisar.
    apagado: watch::Sender<bool>,
}

impl Drop for DbusSystemdClient {
    /// Al soltarse el cliente, el vigilante termina.
    ///
    /// Es lo que evita dejar una tarea zombi reintentando la suscripción contra un
    /// bus que ya nadie mira —y, con ella, el bucle de reintentos del apagado—.
    /// Sin receptores el envío falla, y no es un error: el vigilante ya se fue.
    fn drop(&mut self) {
        let _ = self.apagado.send(true);
    }
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
    ///
    /// La conexión deja arrancada la tarea del vigilante —[`UnitWatcher`]—: se
    /// lanza aquí y **no** se espera, porque el vigilante no termina nunca y
    /// `connect_user` sí tiene que volver. Que no se espere no la hace opcional:
    /// si el bus no admite la suscripción, el vigilante lo registra y la reintenta,
    /// mientras el cliente sigue contestando listados y acciones.
    ///
    /// Y con ella la del progreso de trabajos —[`ProgresoDeTrabajos`]—, por la misma
    /// razón: escucha lo que dure el proceso y publica lo que el gestor le cuente.
    pub async fn connect_user() -> Result<Self, SystemdError> {
        let conexion = Connection::session().await.map_err(transporte)?;
        let (avisos, _) = broadcast::channel(CAPACIDAD_DE_AVISOS);
        let (trabajos, _) = broadcast::channel(CAPACIDAD_DE_TRABAJOS);
        let (apagado, _) = watch::channel(false);

        let cliente = Self {
            conexion,
            avisos,
            trabajos,
            apagado,
        };
        cliente.vigilar();
        cliente.vigilar_trabajos();

        Ok(cliente)
    }

    /// Arranca la tarea que escucha las señales de systemd y avisa de los cambios.
    ///
    /// Se lanza en segundo plano porque la escucha dura lo que dure el proceso: la
    /// tarea termina cuando el bus cierra la conexión —y entonces reintenta— o
    /// cuando se suelta el cliente, que dispara el cierre.
    fn vigilar(&self) {
        let conexion = self.conexion.clone();
        let avisos = self.avisos.clone();
        let apagado = self.apagado.subscribe();

        tokio::spawn(async move {
            vigilar_sin_descanso(
                |apagado| {
                    let conexion = conexion.clone();
                    let avisos = avisos.clone();

                    async move { escuchar(&conexion, &avisos, apagado).await }
                },
                apagado,
            )
            .await;
        });
    }

    /// Arranca la tarea que escucha el ciclo de vida de los jobs y los publica.
    ///
    /// Es la gemela de [`DbusSystemdClient::vigilar`]: se lanza en segundo plano
    /// —dura lo que el proceso— y termina cuando el bus cierra el stream, que
    /// entonces se reintenta, o cuando se suelta el cliente. Comparte con ella el
    /// canal de cierre, que es lo que impide que una de las dos sobreviva al
    /// apagado.
    fn vigilar_trabajos(&self) {
        let conexion = self.conexion.clone();
        let trabajos = self.trabajos.clone();
        let apagado = self.apagado.subscribe();

        tokio::spawn(async move {
            vigilar_sin_descanso(
                |apagado| {
                    let conexion = conexion.clone();
                    let trabajos = trabajos.clone();

                    async move { escuchar_trabajos(&conexion, &trabajos, apagado).await }
                },
                apagado,
            )
            .await;
        });
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

impl UnitWatcher for DbusSystemdClient {
    /// Se suscribe a los avisos que emite la tarea de escucha de señales.
    ///
    /// Lo que se recibe no lleva datos: sólo dice «algo cambió». Quien lo recibe
    /// vuelve a preguntar por el listado, y así la instantánea se construye
    /// siempre por el mismo camino —el que ya está probado—.
    fn subscribe(&self) -> broadcast::Receiver<()> {
        self.avisos.subscribe()
    }
}

/// Pide al gestor que mande sus señales por esta conexión.
///
/// `Subscribe()` es **idempotente en la intención** aunque el gestor no lo sea en la
/// respuesta: lo que se quiere es que la conexión reciba señales, y
/// [`ERROR_YA_SUSCRITO`] significa exactamente eso —lo pidió antes la otra tarea
/// escuchando por la misma conexión—, así que se toma por buena y no se rompe la
/// escucha por pedir algo que ya estaba hecho.
///
/// **La verificación de esa rama, contra el gestor de verdad, depende del test de
/// bus real** (`tests/systemd_real.rs`): es el único sitio donde dos tareas piden la
/// suscripción por la misma conexión y systemd contesta lo que contesta. Sin bus,
/// lo que sí se mide es la decisión —[`es_ya_suscrito`] sobre un error homónimo
/// construido a mano—, pero el viaje de ida y vuelta no.
async fn suscribir(conexion: &Connection) -> Result<(), SystemdError> {
    let manager = Proxy::new(conexion, DESTINO, RUTA_MANAGER, IFACE_MANAGER)
        .await
        .map_err(transporte)?;

    match manager.call::<_, _, ()>("Subscribe", &()).await {
        Ok(()) => Ok(()),
        Err(error) if es_ya_suscrito(&error) => Ok(()),
        Err(error) => Err(transporte(error)),
    }
}

/// ¿Es este error del bus la confirmación de que la conexión **ya** estaba suscrita?
///
/// Se separa del `match` de [`suscribir`] —donde nació, como guarda— porque así la
/// decisión se puede medir sin bus: un error de método con el nombre
/// [`ERROR_YA_SUSCRITO`] y otro con un nombre distinto la agotan entera, y el
/// resto de la función queda reducido a lo que sólo un bus real puede ejercer.
fn es_ya_suscrito(error: &zbus::Error) -> bool {
    match error {
        zbus::Error::MethodError(nombre, _, _) => nombre.as_str() == ERROR_YA_SUSCRITO,
        // Lo que no es un error de método no lo dijo el gestor: no es esta señal.
        _ => false,
    }
}

/// Escucha las señales de systemd y avisa de los cambios hasta que el bus se cae
/// o el servidor pide el cierre.
///
/// Cuatro manantiales y un solo destino:
///
/// * `PropertiesChanged` sobre el espacio de rutas de las unidades: un
///   `ActiveState`/`SubState` que cambia —una quadlet que arranca, para o falla—.
/// * `UnitNew` / `UnitRemoved` del gestor: aparece o desaparece una unidad.
/// * `UnitFilesChanged`: alguien recargó la configuración —un `daemon-reload`
///   tras crear, editar o borrar una quadlet—.
///
/// **Antes de escuchar hay que llamar a `Subscribe()`**: sin esa llamada, systemd
/// sólo emite las señales de las unidades que alguien está observando, y podpot no
/// observa ninguna —pide el listado y se va—, así que los cambios hechos fuera del
/// panel no llegarían nunca.
///
/// Volver no es rendirse: cuando el bus cierra el stream, quien la llama reintenta
/// —con `Subscribe()` incluido, que es lo que hay que rehacer—.
async fn escuchar(
    conexion: &Connection,
    avisos: &broadcast::Sender<()>,
    mut apagado: watch::Receiver<bool>,
) -> Result<(), SystemdError> {
    suscribir(conexion).await?;

    let manager = Proxy::new(conexion, DESTINO, RUTA_MANAGER, IFACE_MANAGER)
        .await
        .map_err(transporte)?;

    let propiedades = MessageStream::for_match_rule(
        regla_de_propiedades()?,
        conexion,
        Some(CAPACIDAD_DE_SENALES),
    )
    .await
    .map_err(transporte)?;

    let altas = manager
        .receive_signal("UnitNew")
        .await
        .map_err(transporte)?;
    let bajas = manager
        .receive_signal("UnitRemoved")
        .await
        .map_err(transporte)?;
    let ficheros = manager
        .receive_signal("UnitFilesChanged")
        .await
        .map_err(transporte)?;

    // Los cuatro manantiales se juntan en uno: al reenvío le da igual de dónde
    // venga la señal, porque lo único que transporta es «algo cambió».
    let manantiales: Vec<Pin<Box<dyn Stream<Item = ()> + Send + '_>>> = vec![
        Box::pin(propiedades.map(|_| ())),
        Box::pin(altas.map(|_| ())),
        Box::pin(bajas.map(|_| ())),
        Box::pin(ficheros.map(|_| ())),
    ];

    let (crudo, entrantes) = mpsc::channel(CAPACIDAD_DE_SENALES);
    let coalescedor = tokio::spawn(coalescer(entrantes, avisos.clone()));

    // La escucha dura hasta que el bus cierra el stream **o** hasta que el cierre
    // del servidor lo pide: sin lo segundo, el apagado dejaría la tarea pegada a un
    // bus que ya nadie mira. `wait_for` —y no `changed()`— porque mira el valor
    // actual antes de esperar: con `changed()`, un cierre ya disparado se quedaría
    // esperando un cambio que no va a llegar.
    tokio::select! {
        biased;

        _ = apagado.wait_for(|apagado| *apagado) => {}
        _ = reenviar(stream::select_all(manantiales), crudo) => {}
    }

    // Sin nadie reenviando, el canal crudo se cierra y el coalescedor termina
    // solo: se espera para no dejar una tarea colgando a la espalda.
    let _ = coalescedor.await;

    Ok(())
}

/// Mantiene viva la escucha de señales, reintentándola cuando el bus la cierra.
///
/// `escuchar` termina en cuanto el bus cierra el stream —una caída, un corte del
/// socket—, y rendirse ahí dejaría el panel sin refrescarse para siempre y sin un
/// solo log. Así que cada desenlace se registra y se reintenta con una espera
/// creciente; lo único que termina el bucle es el cierre del servidor.
///
/// La escucha entra por parámetro —y no se llama a `escuchar` directamente— para
/// poder medir el bucle sin un bus de verdad: el reintento se mide con una escucha
/// de mentira, que es lo que lo hace determinista.
async fn vigilar_sin_descanso<F, Fut>(mut escuchar: F, mut apagado: watch::Receiver<bool>)
where
    F: FnMut(watch::Receiver<bool>) -> Fut,
    Fut: Future<Output = Result<(), SystemdError>>,
{
    let mut retraso = REINTENTO_INICIAL;

    loop {
        let inicio = tokio::time::Instant::now();
        let desenlace = escuchar(apagado.clone()).await;

        // El cierre del servidor se mira **antes** de registrar nada: un cierre
        // pedido no es una caída del bus, y anunciarlo como tal sería mentir en el
        // log. Se pregunta por el valor y por el canal —el disparador puede haberse
        // soltado, y eso también es un cierre—.
        if se_pidio_el_cierre(&apagado) {
            return;
        }

        match desenlace {
            Ok(()) => {
                tracing::warn!("el bus cerró el stream de señales; se reintenta la suscripción")
            }
            Err(error) => tracing::warn!(
                %error,
                "el bus cerró el stream de señales; se reintenta la suscripción"
            ),
        }

        // Una sesión que duró lo suficiente no fue un parpadeo: el bus estuvo sano,
        // así que la espera vuelve a empezar por el principio en vez de arrastrar el
        // tope de una racha de caídas vieja. El backoff crece para no martillear un
        // bus caído, no para castigar a uno que vuelve.
        if inicio.elapsed() >= REINTENTO_MAXIMO {
            retraso = REINTENTO_INICIAL;
        }

        // La espera se parte en dos: el cierre del servidor la interrumpe, para no
        // tardar hasta medio minuto en apagarse.
        tokio::select! {
            biased;

            _ = apagado.wait_for(|apagado| *apagado) => return,
            _ = tokio::time::sleep(retraso) => {}
        }

        retraso = siguiente_retraso(retraso);
    }
}

/// El retraso del reintento siguiente: el doble del anterior, sin pasar del tope.
///
/// Es una función pura —y por eso se puede medir sin reloj y sin bus—: la política
/// de espera no tiene nada que ver con D-Bus.
fn siguiente_retraso(actual: Duration) -> Duration {
    (actual * 2).min(REINTENTO_MAXIMO)
}

/// ¿El cierre lo pidió el servidor?
///
/// Se pregunta por el valor y por el canal: el disparador puede haberse soltado
/// —el cliente se fue y nadie más va a mandar— y eso también es un cierre.
fn se_pidio_el_cierre(apagado: &watch::Receiver<bool>) -> bool {
    *apagado.borrow() || apagado.has_changed().is_err()
}

impl ProgresoDeTrabajos for DbusSystemdClient {
    /// Se suscribe a los eventos que publica la tarea de escucha de jobs.
    ///
    /// Cada suscriptor recibe las transiciones **a partir de su suscripción**: el
    /// canal no guarda historia, así que no hay instantánea inicial —un job en
    /// curso es efímero y su estado final lo entrega el canal de unidades—.
    fn observar(&self) -> broadcast::Receiver<ProgresoDeJob> {
        self.trabajos.subscribe()
    }
}

/// Escucha el ciclo de vida de los jobs del gestor y lo publica como eventos.
///
/// Dos manantiales y ningún *debounce*: `JobNew` al crear un job y `JobRemoved` al
/// quitarlo. Aquí no se coalesce nada a propósito —a diferencia del vigilante de
/// unidades—: cada transición es un dato por derecho propio, y juntar dos jobs en
/// un evento perdería el aviso de uno de ellos.
///
/// **Antes de escuchar hay que ser suscrito**: lo pide [`suscribir`], la misma
/// llamada que hace el vigilante de unidades —que suele ganarle la mano, en cuyo
/// caso la respuesta es «ya estabas» y se toma por buena—; sin la suscripción,
/// systemd no emite las señales.
///
/// Volver no es rendirse: cuando el bus cierra el stream, quien la llama reintenta
/// —con la suscripción incluida, que es lo que hay que rehacer—.
async fn escuchar_trabajos(
    conexion: &Connection,
    trabajos: &broadcast::Sender<ProgresoDeJob>,
    mut apagado: watch::Receiver<bool>,
) -> Result<(), SystemdError> {
    suscribir(conexion).await?;

    let manager = Proxy::new(conexion, DESTINO, RUTA_MANAGER, IFACE_MANAGER)
        .await
        .map_err(transporte)?;

    let altas = manager
        .receive_signal(SENAL_JOB_NEW)
        .await
        .map_err(transporte)?;
    let bajas = manager
        .receive_signal(SENAL_JOB_REMOVED)
        .await
        .map_err(transporte)?;

    // Los dos manantiales se juntan en uno: la traducción mira el nombre de la
    // señal, y el orden de llegada es el que el bus manda.
    let manantiales: Vec<Pin<Box<dyn Stream<Item = zbus::Message> + Send + '_>>> =
        vec![Box::pin(altas), Box::pin(bajas)];

    tokio::select! {
        biased;

        _ = apagado.wait_for(|apagado| *apagado) => {}
        _ = publicar_jobs(
            stream::select_all(manantiales),
            // El lector del tipo de job entra por parámetro: la traducción no habla
            // con el bus por su cuenta, así que se puede medir sin uno.
            |ruta: OwnedObjectPath| async move { tipo_de_job(conexion, &ruta).await },
            trabajos,
        ) => {}
    }

    Ok(())
}

/// Publica un evento de progreso por cada señal de job que llegue.
///
/// Genérico sobre el manantial **y sobre el lector del tipo de job** —y no atado a
/// una conexión de verdad— para poder medir la traducción sin bus: lo que entra son
/// mensajes del gestor, lo que sale son eventos, y las dos mitades se pueden ejercer
/// con mensajes construidos a mano y un lector de mentira.
///
/// El mapa de jobs en curso es la memoria que hace falta para completar el evento
/// final: la señal de baja **no** trae el tipo de job —y para entonces el objeto del
/// job ya no existe, así que tampoco se puede preguntar—, de modo que el tipo se
/// guarda al nacer y se recupera al morir. Se vacía solo —cada baja borra su
/// entrada—, así que no crece con el tiempo.
///
/// «No crece» es literal, y conviene decirlo porque un `HashMap` en un bucle
/// ilimitado pide explicación: cada entrada se escribe con el alta de un job y se
/// borra con su baja, y todo job que systemd crea lo quita —el gestor no acumula
/// jobs—, así que el mapa no puede quedar más alto que los jobs que estén vivos a
/// la vez. Lo único que lo reinicia de golpe es la reconexión del vigilante, que
/// empieza con un mapa vacío; y los jobs que estuvieran en curso entonces no se
/// pierden: su baja sale con [`KIND_DESCONOCIDO`].
async fn publicar_jobs<S, L, F>(
    mut senales: S,
    leer_tipo: L,
    trabajos: &broadcast::Sender<ProgresoDeJob>,
) where
    S: Stream<Item = zbus::Message> + Unpin,
    L: Fn(OwnedObjectPath) -> F,
    F: Future<Output = Option<String>>,
{
    let mut en_curso: HashMap<u32, String> = HashMap::new();

    while let Some(mensaje) = senales.next().await {
        let progreso = match senal_de_job(&mensaje) {
            None => continue,
            Some(SenalDeJob::Nueva { job, ruta, unit }) => {
                // El tipo se pregunta **aquí**, con el job recién creado: es el único
                // momento en que el objeto existe con seguridad.
                let kind = leer_tipo(ruta)
                    .await
                    .unwrap_or_else(|| KIND_DESCONOCIDO.to_owned());

                en_curso.insert(job, kind.clone());

                ProgresoDeJob::nuevo(job, unit, kind)
            }
            Some(SenalDeJob::Quitada {
                job,
                unit,
                resultado,
            }) => {
                let kind = en_curso
                    .remove(&job)
                    .unwrap_or_else(|| KIND_DESCONOCIDO.to_owned());

                ProgresoDeJob::terminado(job, unit, kind, ResultadoDeJob::from_dbus(&resultado))
            }
        };

        // Sin suscriptores el evento se pierde, y no es un error: nadie mira. El
        // panel que se conecte después no se pierde nada suyo —el estado final está
        // en el canal de unidades—.
        let _ = trabajos.send(progreso);
    }
}

/// Lo que una señal del gestor dice de un job, todavía **sin** el tipo.
///
/// Se separa de [`ProgresoDeJob`] porque el tipo no viaja en la señal: hay que ir a
/// buscarlo al objeto del job, y eso es una llamada aparte que sólo se puede hacer
/// con el alta —cuando el objeto existe—. Un tipo intermedio hace explícito que al
/// traducir la señal todavía falta un dato.
enum SenalDeJob {
    /// El gestor acaba de crear el job. La ruta es la del objeto del job, de donde
    /// se lee el tipo.
    Nueva {
        /// El identificador del job.
        job: u32,
        /// La ruta del objeto del job, que es donde vive `JobType`.
        ruta: OwnedObjectPath,
        /// La unidad sobre la que actúa.
        unit: UnitName,
    },
    /// El gestor acaba de quitar el job, con su desenlace.
    Quitada {
        /// El identificador del job.
        job: u32,
        /// La unidad sobre la que actuaba.
        unit: UnitName,
        /// El desenlace que informa el gestor.
        resultado: String,
    },
}

/// Traduce una señal del gestor, o `None` si no es una señal de job.
///
/// Se mira el **nombre** de la señal y no la interfaz: es lo que distingue un alta
/// de una baja, y lo que evita que cualquier otra cosa que diga el bus (los `UnitNew`
/// y `UnitRemoved`, que también cuelgan del manager) se cuele en el canal.
///
/// Las dos firmas son distintas y **no** se pueden confundir: `JobNew` es `(uos)` y
/// `JobRemoved` es `(uoss)`. La documentación de systemd de más de un sitio dice que
/// el alta lleva el tipo de job como cuarto argumento; el systemd de esta máquina
/// (262) no lo hace, y el test contra el bus real es el que lo deja por escrito.
fn senal_de_job(mensaje: &zbus::Message) -> Option<SenalDeJob> {
    let miembro = mensaje.header().member()?.as_str().to_owned();

    match miembro.as_str() {
        SENAL_JOB_NEW => {
            let (job, ruta, unidad): (u32, OwnedObjectPath, String) =
                match mensaje.body().deserialize() {
                    Ok(cuerpo) => cuerpo,
                    Err(error) => {
                        aviso_de_cuerpo_inesperado(&miembro, &error);

                        return None;
                    }
                };

            Some(SenalDeJob::Nueva {
                job,
                ruta,
                unit: unidad_de_senal(&unidad),
            })
        }
        SENAL_JOB_REMOVED => {
            let (job, _ruta, unidad, resultado): (u32, OwnedObjectPath, String, String) =
                match mensaje.body().deserialize() {
                    Ok(cuerpo) => cuerpo,
                    Err(error) => {
                        aviso_de_cuerpo_inesperado(&miembro, &error);

                        return None;
                    }
                };

            Some(SenalDeJob::Quitada {
                job,
                unit: unidad_de_senal(&unidad),
                resultado,
            })
        }
        _ => None,
    }
}

/// Deja constancia de una señal de job cuyo cuerpo no encaja, y no publica nada.
///
/// Un cuerpo que no encaja se descarta, pero **no en silencio**: perder la
/// transición sin dejar rastro dejaría el panel mudo y sin causa visible. Y no es
/// un caso hipotético: si systemd cambia la firma —una versión que añada el tipo de
/// job al alta, como da por hecho su documentación— todo lo que llegue deja de
/// encajar, y este aviso es lo único que lo delata.
///
/// El mensaje vive aquí, y no repetido en las dos señales, para que la alta y la
/// baja lo digan con las mismas palabras: dos copias acaban divergiendo y quien
/// busque en el registro ya no sabrá qué patrón mirar.
fn aviso_de_cuerpo_inesperado(miembro: &str, error: &zbus::Error) {
    tracing::warn!(%error, senal = %miembro, "señal de job con cuerpo inesperado");
}

/// El tipo de un job, leído de la propiedad `JobType` del objeto del job.
///
/// Es una llamada **de más** por cada job y no hay forma de evitarla: ni `JobNew` ni
/// `JobRemoved` llevan el tipo en el cuerpo, y el único sitio donde systemd lo
/// publica es la propiedad `JobType` del objeto del job. El precio —un viaje de
/// bus por job, y los jobs de un usuario se cuentan con los dedos— es despreciable
/// frente a publicar un tipo inventado.
///
/// Devuelve `None` cuando el objeto ya no está —un job que nació y murió entre el
/// aviso y la pregunta— o cuando el gestor no contesta **a tiempo**: quien la llama
/// publica entonces [`KIND_DESCONOCIDO`], que es lo honesto. El plazo es explícito
/// ([`ESPERA_DEL_TIPO`]) porque de esta consulta depende el ritmo de la escucha
/// entera: sin plazo, un gestor que no contesta pararía detrás de ella todos los
/// avisos que vinieran después.
async fn tipo_de_job(conexion: &Connection, ruta: &OwnedObjectPath) -> Option<String> {
    let consulta = async {
        let job = Proxy::new(conexion, DESTINO, ruta.as_str(), IFACE_JOB)
            .await
            .ok()?;

        job.get_property::<String>(PROPIEDAD_JOB_TYPE).await.ok()
    };

    tokio::time::timeout(ESPERA_DEL_TIPO, consulta)
        .await
        // Vencido el plazo no hay tipo, y tampoco un fallo que contar: la escucha
        // sigue con el siguiente aviso.
        .ok()
        .flatten()
}

/// La unidad de una señal de job, como nombre de unidad de podpot.
///
/// El gestor informa siempre un nombre de unidad de verdad, así que el camino
/// normal es [`UnitName::parse`]. Si alguna vez no lo fuera, el evento **no se
/// pierde**: se conserva el nombre tal cual y se deja un aviso en el registro —un
/// job que termina es un dato del panel aunque su nombre no pase el filtro—.
fn unidad_de_senal(nombre: &str) -> UnitName {
    match UnitName::parse(nombre) {
        Ok(unit) => unit,
        Err(_) => {
            tracing::warn!(
                unidad = %nombre,
                "una señal de job nombró una unidad que podpot no reconoce"
            );

            UnitName::sin_validar(nombre)
        }
    }
}

/// La regla de D-Bus que selecciona los cambios de propiedades de las unidades.
///
/// Es una regla de **espacio de rutas** y no de ruta: no se puede observar unidad
/// por unidad porque podpot no sabe cuáles hay hasta que las lista, y una quadlet
/// nueva aparece sin avisar a nadie.
fn regla_de_propiedades() -> Result<MatchRule<'static>, SystemdError> {
    let constructor = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface(IFACE_PROPIEDADES)
        .map_err(transporte)?
        .member(SENAL_PROPIEDADES)
        .map_err(transporte)?
        .path_namespace(ESPACIO_DE_UNIDADES)
        .map_err(transporte)?;

    Ok(constructor.build())
}

/// Reenvía cada señal recibida al canal crudo, sin mirar qué señal es.
///
/// Es genérico sobre el manantial —y no está atado a D-Bus— a propósito: lo único
/// que hace es convertir «llegó una señal» en «un aviso crudo», y eso se puede
/// medir con un flujo de prueba, sin bus.
async fn reenviar<S>(mut senales: S, crudo: mpsc::Sender<()>)
where
    S: Stream + Unpin,
{
    while senales.next().await.is_some() {
        // Si el coalescedor se fue no hay a quién avisar, y esperar aquí sería
        // quedarse pegado a un canal sin receptor.
        if crudo.send(()).await.is_err() {
            return;
        }
    }
}

/// Convierte una ráfaga de señales en un solo aviso.
///
/// El *debounce* es de cola: la primera señal abre una espera, cada señal nueva la
/// reinicia y sólo el silencio la cierra. Así un `daemon-reload` que dispara
/// decenas de señales seguidas produce **un** aviso, y no uno por señal.
async fn coalescer(mut entrantes: mpsc::Receiver<()>, avisos: broadcast::Sender<()>) {
    while entrantes.recv().await.is_some() {
        esperar_la_calma(&mut entrantes).await;

        // Sin suscriptores el aviso se pierde, y no es un error: nadie mira.
        let _ = avisos.send(());
    }
}

/// Espera a que la ráfaga se calle.
///
/// Vuelve cuando pasa [`DEBOUNCE`] sin recibir nada nuevo, o cuando el reenvío se
/// fue —el canal se cerró—.
async fn esperar_la_calma(entrantes: &mut mpsc::Receiver<()>) {
    while let Ok(Some(())) = tokio::time::timeout(DEBOUNCE, entrantes.recv()).await {
        // Sigue la ráfaga: se espera el silencio entero otra vez.
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
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use futures_util::stream;
    use tokio::sync::{broadcast, mpsc, watch};
    use zbus::match_rule::PathSpec;
    use zbus::message::Type;

    use super::{
        CAPACIDAD_DE_SENALES, Clase, DEBOUNCE, ERROR_FILE_NOT_FOUND, ERROR_NO_SUCH_UNIT,
        ERROR_UNKNOWN_PROPERTY, ERROR_YA_SUSCRITO, ESPACIO_DE_UNIDADES, IFACE_PROPIEDADES,
        KIND_DESCONOCIDO, REINTENTO_INICIAL, REINTENTO_MAXIMO, SENAL_PROPIEDADES, clase,
        clase_del_nombre, coalescer, es_ya_suscrito, publicar_jobs, recarga, reenviar,
        regla_de_propiedades, senal_de_job, siguiente_retraso, vigilar_sin_descanso,
    };
    use crate::systemd::{FaseDeJob, ProgresoDeJob, ResultadoDeJob, SystemdError};
    use crate::test_support::capturar_log;

    /// Una señal de **alta** de job del gestor, con la firma que systemd emite de
    /// verdad: `JobNew` es `(uos)` —identificador, ruta del job y unidad— y **no**
    /// trae el tipo de job.
    ///
    /// Se construye un `Message` de D-Bus **de verdad**, sin bus: es lo que mide que
    /// la firma que podpot espera es la que systemd emite. El test contra el bus real
    /// (`tests/systemd_real.rs`) es el que fija el número de argumentos; aquí se
    /// repite para que un cambio de firma se rompa también sin bus.
    fn senal_de_alta(job: u32, unidad: &str) -> zbus::Message {
        zbus::Message::signal(
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "JobNew",
        )
        .expect("la cabecera de la señal es válida")
        .build(&(job, ruta_del_job(job), unidad))
        .expect("el cuerpo de la señal se serializa")
    }

    /// Una señal de **baja** de job del gestor, con la firma real: `JobRemoved` es
    /// `(uoss)` —identificador, ruta del job, unidad y resultado—.
    fn senal_de_baja(job: u32, unidad: &str, resultado: &str) -> zbus::Message {
        zbus::Message::signal(
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "JobRemoved",
        )
        .expect("la cabecera de la señal es válida")
        .build(&(job, ruta_del_job(job), unidad, resultado))
        .expect("el cuerpo de la señal se serializa")
    }

    /// La ruta del objeto de un job, que es de donde se lee su tipo.
    fn ruta_del_job(job: u32) -> zbus::zvariant::OwnedObjectPath {
        zbus::zvariant::OwnedObjectPath::try_from(format!("/org/freedesktop/systemd1/job/{job}"))
            .expect("la ruta del job es una ruta de objeto válida")
    }

    /// Un error de método de D-Bus con el nombre pedido, construido a mano.
    ///
    /// Es lo que hace medible la traducción de errores sin bus: podpot sólo mira el
    /// **nombre** del error, así que el mensaje que lo acompaña —la respuesta del
    /// gestor— se sustituye por una señal cualquiera.
    fn error_de_metodo(nombre: &str) -> zbus::Error {
        let respuesta = zbus::Message::signal(
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "JobNew",
        )
        .expect("la cabecera del mensaje es válida")
        .build(&())
        .expect("el cuerpo vacío se serializa");

        zbus::Error::MethodError(
            zbus::names::OwnedErrorName::try_from(nombre.to_owned())
                .expect("el nombre del error es válido"),
            None,
            respuesta,
        )
    }

    /// Publica unas señales con un lector de tipo **fijo**: el tipo que se le pida,
    /// o `None` para medir el caso de que no se pueda saber.
    async fn publicar_con_tipo(
        senales: Vec<zbus::Message>,
        tipo: Option<&str>,
        trabajos: &broadcast::Sender<ProgresoDeJob>,
    ) {
        let tipo = tipo.map(str::to_owned);

        publicar_jobs(
            stream::iter(senales),
            move |_ruta| {
                let tipo = tipo.clone();

                async move { tipo }
            },
            trabajos,
        )
        .await;
    }

    /// Espera el siguiente evento del canal de trabajos, con el plazo de la casa.
    async fn siguiente_evento(
        suscriptor: &mut broadcast::Receiver<ProgresoDeJob>,
    ) -> ProgresoDeJob {
        tokio::time::timeout(Duration::from_secs(5), suscriptor.recv())
            .await
            .expect("el evento tiene que llegar antes del plazo")
            .expect("el canal sigue vivo mientras el emisor exista")
    }

    /// Escenarios «Alta de job reenviada», «Baja de job reenviada con su resultado»
    /// y «Job de origen ajeno al panel»: las señales `JobNew`/`JobRemoved` del
    /// gestor se publican como eventos, con independencia de quién pidiera el job
    /// —la unidad de la prueba no la conoce nadie del panel—.
    #[tokio::test]
    async fn las_senales_de_job_del_gestor_se_publican_como_progreso() {
        let (trabajos, _) = broadcast::channel(8);
        let mut suscriptor = trabajos.subscribe();

        publicar_con_tipo(
            vec![
                senal_de_alta(7, "centinela-db.service"),
                senal_de_baja(7, "centinela-db.service", "done"),
            ],
            Some("start"),
            &trabajos,
        )
        .await;

        let alta = siguiente_evento(&mut suscriptor).await;
        assert_eq!(alta.job, 7);
        assert_eq!(alta.unit.as_str(), "centinela-db.service");
        assert_eq!(
            alta.kind, "start",
            "el tipo de job sale del objeto del job, que es donde vive"
        );
        assert_eq!(alta.phase, FaseDeJob::New);
        assert_eq!(
            alta.result, None,
            "un job que acaba de nacer no tiene resultado"
        );

        let baja = siguiente_evento(&mut suscriptor).await;
        assert_eq!(baja.job, 7);
        assert_eq!(baja.unit.as_str(), "centinela-db.service");
        assert_eq!(
            baja.kind, "start",
            "la señal de baja no trae el tipo de job: se recuerda del alta"
        );
        assert_eq!(baja.phase, FaseDeJob::Done);
        assert_eq!(
            baja.result,
            Some(ResultadoDeJob::from_dbus("done")),
            "el resultado que informa systemd tiene que llegar entero"
        );
    }

    /// Al lector del tipo le llega **la ruta del job de la señal**, que es lo que
    /// hace falta para preguntar por su `JobType`: con otra ruta, o con ninguna, el
    /// tipo saldría del job equivocado —o de ninguno—.
    #[tokio::test]
    async fn el_tipo_se_lee_de_la_ruta_del_job_de_la_senal() {
        let (trabajos, _) = broadcast::channel(8);
        let mut suscriptor = trabajos.subscribe();

        // Un lector que devuelve el último segmento de la ruta: si llegara vacía o
        // fuera otra, el aserto de abajo lo diría.
        publicar_jobs(
            stream::iter([senal_de_alta(42, "centinela-db.service")]),
            |ruta| async move { ruta.as_str().rsplit('/').next().map(str::to_owned) },
            &trabajos,
        )
        .await;

        let alta = siguiente_evento(&mut suscriptor).await;

        assert_eq!(
            alta.kind, "42",
            "el lector tiene que recibir la ruta del job de la señal"
        );
    }

    /// Escenario «Tipo de job desconocido»: si el tipo no se puede leer —el objeto
    /// del job ya no estaba, o el gestor no contestó—, el alta **se publica
    /// igualmente** con `unknown`. Es lo honesto: el job existe y su unidad se sabe,
    /// aunque su tipo no.
    #[tokio::test]
    async fn sin_tipo_conocido_el_evento_sale_igual_con_unknown() {
        let (trabajos, _) = broadcast::channel(8);
        let mut suscriptor = trabajos.subscribe();

        publicar_con_tipo(
            vec![senal_de_alta(7, "centinela-db.service")],
            None,
            &trabajos,
        )
        .await;

        let alta = siguiente_evento(&mut suscriptor).await;

        assert_eq!(alta.phase, FaseDeJob::New);
        assert_eq!(alta.job, 7);
        assert_eq!(alta.unit.as_str(), "centinela-db.service");
        assert_eq!(
            alta.kind, KIND_DESCONOCIDO,
            "sin tipo legible, el evento sale con `unknown` y no se pierde"
        );
    }

    /// Una señal que **no** es de un job no publica nada: el canal de trabajos sólo
    /// lleva transiciones de job, no cualquier cosa que diga el bus.
    #[tokio::test]
    async fn una_senal_que_no_es_de_un_job_no_publica_nada() {
        let (trabajos, _) = broadcast::channel(8);
        let mut suscriptor = trabajos.subscribe();

        let ajena = zbus::Message::signal(
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "UnitFilesChanged",
        )
        .expect("la cabecera es válida")
        .build(&())
        .expect("el cuerpo vacío se serializa");

        publicar_con_tipo(vec![ajena], Some("start"), &trabajos).await;

        assert!(
            suscriptor.try_recv().is_err(),
            "una señal ajena no puede colarse en el canal de trabajos"
        );
    }

    /// Y un job que **no** se vio nacer —el caso de una reconexión del vigilante—
    /// termina igual: no se pierde el fin del job porque falte su alta.
    #[tokio::test]
    async fn un_job_que_no_se_vio_nacer_termina_sin_tipo() {
        let (trabajos, _) = broadcast::channel(8);
        let mut suscriptor = trabajos.subscribe();

        publicar_con_tipo(
            vec![senal_de_baja(9, "otra.service", "failed")],
            Some("start"),
            &trabajos,
        )
        .await;

        let baja = siguiente_evento(&mut suscriptor).await;
        assert_eq!(baja.phase, FaseDeJob::Done);
        assert_eq!(baja.unit.as_str(), "otra.service");
        assert_eq!(baja.kind, KIND_DESCONOCIDO);
        assert_eq!(baja.result, Some(ResultadoDeJob::from_dbus("failed")));
    }

    /// Una señal de job cuyo cuerpo **no** encaja con la firma que podpot espera no
    /// publica nada —pero tampoco se traga en silencio: el descarte **deja rastro**
    /// en el registro.
    ///
    /// Es el caso de una versión de systemd que añadiera un argumento al alta: el
    /// panel se quedaría sin eventos, y lo único que diría por qué es el aviso. Se
    /// mide con el arnés de captura del registro, así que la prueba falla tanto si
    /// se publica un evento inventado como si el descarte se hace en silencio.
    #[test]
    fn una_senal_de_job_con_cuerpo_inesperado_se_descarta_con_aviso() {
        let (buffer, _guardia) = capturar_log();

        // Un `JobNew` con el cuarto argumento que la documentación de systemd
        // promete y que este systemd no manda: el cuerpo deja de encajar.
        let inesperada = zbus::Message::signal(
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "JobNew",
        )
        .expect("la cabecera de la señal es válida")
        .build(&(7, ruta_del_job(7), "centinela-db.service", "start"))
        .expect("el cuerpo de la señal se serializa");

        assert!(
            senal_de_job(&inesperada).is_none(),
            "un cuerpo que no encaja no puede inventarse un evento"
        );
        assert!(
            buffer.texto().contains("cuerpo inesperado"),
            "el descarte tiene que dejar rastro en el registro: {:?}",
            buffer.texto()
        );
    }

    /// Y ese descarte, aguas abajo, tampoco publica: una señal con cuerpo inesperado
    /// no se convierte en evento en el canal.
    #[tokio::test]
    async fn una_senal_de_job_con_cuerpo_inesperado_no_publica_nada() {
        let (trabajos, _) = broadcast::channel(8);
        let mut suscriptor = trabajos.subscribe();

        let inesperada = zbus::Message::signal(
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "JobNew",
        )
        .expect("la cabecera de la señal es válida")
        .build(&(7, ruta_del_job(7), "centinela-db.service", "start"))
        .expect("el cuerpo de la señal se serializa");

        publicar_con_tipo(vec![inesperada], Some("start"), &trabajos).await;

        assert!(
            suscriptor.try_recv().is_err(),
            "un cuerpo que no encaja no puede inventarse un evento"
        );
    }

    /// La decisión de [`super::suscribir`] sobre el error del gestor, medida sin bus:
    /// `AlreadySubscribed` se toma por bueno —la conexión ya recibe señales, que es
    /// lo que se iba a pedir— y cualquier otro error no.
    ///
    /// El viaje de ida y vuelta —que systemd conteste eso de verdad cuando la otra
    /// tarea de la misma conexión pidió antes la suscripción— sólo lo ejerce el test
    /// de bus real (`tests/systemd_real.rs`); lo que se fija aquí es la traducción.
    #[test]
    fn solo_el_error_de_ya_suscrito_se_toma_por_bueno() {
        assert!(
            es_ya_suscrito(&error_de_metodo(ERROR_YA_SUSCRITO)),
            "`AlreadySubscribed` significa que la conexión ya recibe señales"
        );
        assert!(
            !es_ya_suscrito(&error_de_metodo(ERROR_NO_SUCH_UNIT)),
            "cualquier otro error del gestor no es una suscripción ya hecha"
        );
        assert!(
            !es_ya_suscrito(&zbus::Error::Unsupported),
            "lo que no es un error de método no lo dijo el gestor"
        );
    }

    /// El puerto del progreso de trabajos se ejerce a través del cliente real: la
    /// suscripción existe antes de que llegue ninguna señal del bus.
    #[test]
    fn el_cliente_real_expone_la_suscripcion_al_progreso() {
        fn acepta<T: super::ProgresoDeTrabajos>() {}

        acepta::<super::DbusSystemdClient>();
    }

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

    /// La regla del vigilante selecciona los cambios de propiedades de **cualquier**
    /// unidad: señal, interfaz estándar de propiedades, miembro
    /// `PropertiesChanged` y el espacio de rutas de las unidades.
    ///
    /// Se comprueba aquí, sin bus, porque es la única parte del vigilante que no
    /// depende de que haya señales: con la ruta equivocada, el panel no se
    /// refrescaría nunca y nada lo delataría.
    #[test]
    fn la_regla_escucha_los_cambios_de_propiedades_de_toda_unidad() {
        let regla = regla_de_propiedades().expect("los nombres de la regla son constantes válidas");

        assert_eq!(regla.msg_type(), Some(Type::Signal));
        assert_eq!(
            regla.interface().map(|interfaz| interfaz.as_str()),
            Some(IFACE_PROPIEDADES)
        );
        assert_eq!(
            regla.member().map(|miembro| miembro.as_str()),
            Some(SENAL_PROPIEDADES)
        );

        match regla.path_spec() {
            Some(PathSpec::PathNamespace(ruta)) => assert_eq!(ruta.as_str(), ESPACIO_DE_UNIDADES),
            otro => panic!("se esperaba el espacio de rutas de las unidades: {otro:?}"),
        }
    }

    /// Escenario «Notificaciones coalescidas»: varias señales seguidas producen
    /// **un** aviso, no uno por señal.
    ///
    /// El reloj va parado (`start_paused`), así que el *debounce* se mide de
    /// verdad —avanza sólo cuando no queda nada que hacer— y la prueba no depende
    /// de lo que tarde la máquina.
    #[tokio::test(start_paused = true)]
    async fn una_rafaga_de_senales_produce_un_solo_aviso() {
        let (crudo, entrantes) = mpsc::channel(CAPACIDAD_DE_SENALES);
        let (avisos, _) = broadcast::channel(8);
        let mut suscriptor = avisos.subscribe();

        let coalescedor = tokio::spawn(coalescer(entrantes, avisos.clone()));

        // Tres señales seguidas: es una ráfaga, no tres cambios.
        reenviar(stream::iter([(), (), ()]), crudo).await;

        tokio::time::timeout(Duration::from_secs(60), suscriptor.recv())
            .await
            .expect("la ráfaga tiene que producir un aviso")
            .expect("el canal sigue vivo mientras el coalescedor exista");

        // Y no hay un segundo: el silencio siguiente no emite nada.
        assert!(
            tokio::time::timeout(DEBOUNCE * 4, suscriptor.recv())
                .await
                .is_err(),
            "la ráfaga se tenía que coalescer en un solo aviso"
        );

        coalescedor.abort();
    }

    /// Y dos ráfagas **separadas** por el silencio del *debounce* sí producen dos
    /// avisos: lo que se coalesce es la ráfaga, no el flujo entero.
    #[tokio::test(start_paused = true)]
    async fn dos_rafagas_separadas_producen_dos_avisos() {
        let (crudo, entrantes) = mpsc::channel(CAPACIDAD_DE_SENALES);
        let (avisos, _) = broadcast::channel(8);
        let mut suscriptor = avisos.subscribe();

        let coalescedor = tokio::spawn(coalescer(entrantes, avisos.clone()));

        // Primera ráfaga: un aviso.
        reenviar(stream::iter([()]), crudo.clone()).await;
        tokio::time::timeout(Duration::from_secs(60), suscriptor.recv())
            .await
            .expect("la primera ráfaga tiene que avisar")
            .expect("el canal sigue vivo");

        // Silencio de sobra —más que el *debounce*— y segunda ráfaga: otro aviso.
        tokio::time::sleep(DEBOUNCE * 2).await;
        reenviar(stream::iter([()]), crudo).await;

        tokio::time::timeout(Duration::from_secs(60), suscriptor.recv())
            .await
            .expect("la segunda ráfaga tiene que avisar")
            .expect("el canal sigue vivo");

        coalescedor.abort();
    }

    /// El retraso del reintento crece al doble y **no** pasa del tope.
    ///
    /// Es la parte del vigilante que no depende de ningún bus —ni de ningún reloj—
    /// así que se mide suelta: sin tope, un bus que no vuelve haría esperas cada vez
    /// más largas; con él, el coste de un bus muerto es un intento cada medio minuto.
    #[test]
    fn el_retraso_del_reintento_crece_hasta_el_tope() {
        assert_eq!(REINTENTO_INICIAL, Duration::from_secs(1));

        let mut retraso = REINTENTO_INICIAL;
        assert_eq!(siguiente_retraso(retraso), Duration::from_secs(2));

        retraso = siguiente_retraso(retraso);
        assert_eq!(siguiente_retraso(retraso), Duration::from_secs(4));

        // Por muchos intentos que pasen, nunca se pasa del tope…
        let mut retraso = REINTENTO_INICIAL;
        for _ in 0..64 {
            retraso = siguiente_retraso(retraso);

            assert!(
                retraso <= REINTENTO_MAXIMO,
                "el retraso se pasó del tope: {retraso:?}"
            );
        }

        // …y cuando lo alcanza, ahí se queda.
        assert_eq!(retraso, REINTENTO_MAXIMO);
        assert_eq!(siguiente_retraso(REINTENTO_MAXIMO), REINTENTO_MAXIMO);
    }

    /// Escenario «El bus cierra el stream de señales»: cuando el bus cierra la
    /// escucha —con un error o cerrando el stream—, el vigilante **no** se rinde:
    /// reintenta con una espera creciente. Y sólo el cierre del servidor termina el
    /// bucle, para no dejar una tarea zombi tras el apagado.
    ///
    /// La escucha entra por parámetro —y no se llama a `escuchar` de verdad— porque
    /// lo que se mide aquí es el bucle, no el bus: con un bus de verdad esta prueba
    /// dejaría de ser determinista. El reloj va parado, así que las esperas crecientes
    /// se consumen sin que la prueba espere de verdad.
    #[tokio::test(start_paused = true)]
    async fn el_vigilante_reintenta_cuando_el_bus_cierra_el_stream() {
        let intentos = Arc::new(AtomicUsize::new(0));
        let (disparador, apagado) = watch::channel(false);

        let contador = intentos.clone();
        let escucha = move |_apagado: watch::Receiver<bool>| {
            let contador = contador.clone();

            async move {
                // El primer desenlace es un error —el bus que se cayó— y el resto un
                // stream que se cierra: los dos tienen que reintentarse.
                if contador.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(SystemdError::Transport {
                        causa: Box::new(std::io::Error::other("el bus se cayó")),
                    })
                } else {
                    Ok(())
                }
            }
        };

        let vigilante = tokio::spawn(vigilar_sin_descanso(escucha, apagado));

        // Con el reloj parado, los diez segundos se consumen de golpe y caben los
        // reintentos de 1 s, 2 s, 4 s y 8 s.
        tokio::time::sleep(Duration::from_secs(10)).await;

        let hechos = intentos.load(Ordering::SeqCst);
        assert!(
            hechos >= 3,
            "el vigilante tenía que reintentar tras cerrarse el bus: sólo {hechos} intento(s)"
        );

        // El cierre del servidor sí termina el bucle: nada de tareas zombi.
        disparador
            .send(true)
            .expect("el vigilante sigue suscrito al cierre");

        tokio::time::timeout(Duration::from_secs(5), vigilante)
            .await
            .expect("el cierre del servidor tiene que terminar el vigilante")
            .expect("el vigilante no puede entrar en pánico");
    }

    /// Una sesión de escucha lo bastante larga para contar como «el bus estaba
    /// sano», y no como un parpadeo.
    const SESION_LARGA: Duration = Duration::from_secs(40);

    /// El retraso del reintento **vuelve a empezar** tras una sesión que duró lo
    /// suficiente.
    ///
    /// Sin esto, una racha de parpadeos deja el retraso en el tope y una caída
    /// posterior —aunque el bus haya estado sano horas— tarda hasta medio minuto en
    /// recuperarse: el backoff crece para no martillear un bus caído, no para
    /// castigar a un bus que vuelve.
    ///
    /// Se mide con el reloj parado, así que la duración de la sesión y la espera
    /// siguiente son exactas y la prueba no depende de lo que tarde la máquina.
    #[tokio::test(start_paused = true)]
    async fn el_reintento_vuelve_a_empezar_tras_una_sesion_sana() {
        assert!(
            SESION_LARGA >= REINTENTO_MAXIMO,
            "la sesión de prueba tiene que contar como sana: {SESION_LARGA:?}"
        );

        // El instante —virtual— en el que empezó cada intento.
        let marcas = Arc::new(Mutex::new(Vec::new()));
        let (disparador, apagado) = watch::channel(false);

        let anotador = marcas.clone();
        let escucha = move |_apagado: watch::Receiver<bool>| {
            let anotador = anotador.clone();

            async move {
                let intento = {
                    let mut marcas = anotador.lock().expect("el candado de las marcas");

                    marcas.push(tokio::time::Instant::now());
                    marcas.len() - 1
                };

                // La segunda sesión es la sana: dura de sobra y luego se cae.
                if intento == 1 {
                    tokio::time::sleep(SESION_LARGA).await;
                }

                Ok(())
            }
        };

        let vigilante = tokio::spawn(vigilar_sin_descanso(escucha, apagado));

        // De sobra para las tres sesiones y las esperas de en medio.
        tokio::time::sleep(Duration::from_secs(60)).await;

        // La espera se calcula con el candado dentro de un bloque —y no se suelta a
        // mano— para que no pueda quedar vivo a través del `await` de más abajo.
        let espera = {
            let marcas = marcas.lock().expect("el candado de las marcas");
            let sana = *marcas
                .get(1)
                .expect("la sesión sana tiene que haber empezado");
            let siguiente = *marcas
                .get(2)
                .expect("tras la sesión sana tiene que haber otro intento");

            siguiente - sana - SESION_LARGA
        };

        assert_eq!(
            espera, REINTENTO_INICIAL,
            "tras una sesión sana, el reintento tiene que volver a empezar por el \
             principio: se esperó {espera:?}"
        );

        disparador
            .send(true)
            .expect("el vigilante sigue suscrito al cierre");

        tokio::time::timeout(Duration::from_secs(5), vigilante)
            .await
            .expect("el cierre del servidor tiene que terminar el vigilante")
            .expect("el vigilante no puede entrar en pánico");
    }
}
