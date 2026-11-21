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
    ActiveState, SystemdClient, SystemdError, UnitAction, UnitFileState, UnitName, UnitOrigin,
    UnitStatus, UnitWatcher,
};

/// El servicio del gestor de systemd.
const DESTINO: &str = "org.freedesktop.systemd1";

/// La ruta del objeto del gestor.
const RUTA_MANAGER: &str = "/org/freedesktop/systemd1";

/// La interfaz del gestor.
const IFACE_MANAGER: &str = "org.freedesktop.systemd1.Manager";

/// La interfaz de una unidad.
const IFACE_UNIT: &str = "org.freedesktop.systemd1.Unit";

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
    /// El canal por el que el vigilante avisa de los cambios de unidades.
    ///
    /// Es `broadcast` y no `mpsc` porque los suscriptores son varios y ninguno
    /// manda a los demás: el fan-out escucha para recalcular y publicar, y una
    /// prueba de integración escucha para medir el camino entero.
    avisos: broadcast::Sender<()>,
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
    pub async fn connect_user() -> Result<Self, SystemdError> {
        let conexion = Connection::session().await.map_err(transporte)?;
        let (avisos, _) = broadcast::channel(CAPACIDAD_DE_AVISOS);
        let (apagado, _) = watch::channel(false);

        let cliente = Self {
            conexion,
            avisos,
            apagado,
        };
        cliente.vigilar();

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
    let manager = Proxy::new(conexion, DESTINO, RUTA_MANAGER, IFACE_MANAGER)
        .await
        .map_err(transporte)?;

    manager
        .call::<_, _, ()>("Subscribe", &())
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
        ERROR_UNKNOWN_PROPERTY, ESPACIO_DE_UNIDADES, IFACE_PROPIEDADES, REINTENTO_INICIAL,
        REINTENTO_MAXIMO, SENAL_PROPIEDADES, clase, clase_del_nombre, coalescer, recarga, reenviar,
        regla_de_propiedades, siguiente_retraso, vigilar_sin_descanso,
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
