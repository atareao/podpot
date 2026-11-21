//! Pruebas contra el bus de systemd del usuario de **verdad**.
//!
//! Van marcadas con `#[ignore]` porque necesitan una máquina con un bus de
//! usuario en pie —y con quadlets instaladas para mirar—: no es algo que se pueda
//! exigir a quien sólo quiere compilar el proyecto. Se ejecutan a mano:
//!
//! ```sh
//! cargo test --test systemd_real -- --ignored --nocapture
//! ```
//!
//! Y existen porque hay cuatro cosas que **no** se pueden medir sin bus:
//!
//! * que `ListUnits` conteste y devuelva el listado con los nombres de unidad que
//!   podpot espera;
//! * que el **escape** del nombre produzca una ruta que el gestor reconozca —una
//!   ruta inventada no falla: por una unidad que no existe, systemd contesta
//!   `inactive` igual que por una parada—;
//! * que el `JobRemoved` del job llegue y la acción vuelva cuando el job termina,
//!   en vez de esperar al plazo;
//! * que **arrancar o parar** una quadlet emita `PropertiesChanged` sobre su unidad
//!   y el vigilante lo convierta en una notificación —el camino que el panel usa
//!   para refrescarse solo—.

use podpot::systemd::dbus::DbusSystemdClient;
use podpot::systemd::{
    ActiveState, SystemdClient, UnitAction, UnitFileState, UnitName, UnitStatus, UnitWatcher,
};

/// El plazo con el que se esperan las notificaciones del vigilante.
///
/// Generoso a propósito: el *debounce* del vigilante tiene que cerrar la ráfaga
/// antes de avisar, así que la señal no llega al instante ni mucho menos.
const ESPERA_DE_AVISO: std::time::Duration = std::time::Duration::from_secs(5);

/// Se conecta al bus de usuario; si no hay, la prueba no tiene sentido y se dice.
async fn cliente() -> DbusSystemdClient {
    DbusSystemdClient::connect_user()
        .await
        .expect("estas pruebas necesitan el bus de systemd del usuario")
}

/// Las quadlets que el gestor conoce, o un aviso y nada si no hay ninguna.
///
/// No se falla por no tener quadlets: la prueba mide el bus, no el contenido de
/// la máquina, y una máquina limpia no prueba nada malo.
async fn quadlets_o_aviso(cliente: &DbusSystemdClient) -> Vec<UnitStatus> {
    let quadlets = cliente
        .quadlets()
        .await
        .expect("ListUnits tiene que contestar contra el bus de usuario");

    if quadlets.is_empty() {
        eprintln!(
            "no hay quadlets en el gestor de usuario: la prueba no tiene nada que mirar \
             (crea una con `podman quadlet` para ejercerla)"
        );
    }

    quadlets
}

/// El listado del bus es un listado de nombres de unidad, y el escape de cada uno
/// produce una ruta que systemd conoce.
#[tokio::test]
#[ignore = "necesita el bus de systemd del usuario"]
async fn las_quadlets_del_bus_se_listan_y_su_ruta_escapada_existe() {
    let cliente = cliente().await;

    for quadlet in quadlets_o_aviso(&cliente).await {
        let nombre = quadlet.name.as_str().to_owned();

        // El nombre que informa el gestor tiene que ser un nombre de unidad para
        // podpot: si no lo fuera, la API no podría aceptarlo de vuelta.
        let unidad = UnitName::parse(&nombre).unwrap_or_else(|error| {
            panic!("el gestor informó de `{nombre}`, que no es válido para podpot: {error}")
        });

        // El escape es lo que se pasa a D-Bus, así que no puede llevar ni un byte
        // que systemd no use en una ruta.
        assert!(
            unidad
                .escaped()
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
            "el escape de `{nombre}` dejó bytes que no son de una ruta de D-Bus: {}",
            unidad.escaped()
        );

        // Y la comprobación de verdad: pedir el estado por esa ruta responde por
        // **esta** unidad y no por una inventada.
        let estado = cliente.status(&unidad).await.unwrap_or_else(|error| {
            panic!("el gestor no reconoció la ruta escapada de `{nombre}`: {error}")
        });

        assert_eq!(estado.name.as_str(), nombre);
        assert!(
            estado.is_generated(),
            "`{nombre}` se listó como quadlet pero su UnitFileState no es generated"
        );
        assert!(
            !matches!(estado.active_state, ActiveState::Unknown(_)),
            "`{nombre}` informó de un ActiveState que podpot no conoce: {}",
            estado.active_state
        );
    }
}

/// El listado **no** trae unidades transitorias.
///
/// Es la comprobación contra el bus real del defecto que el doble mide en
/// pequeño: el listado enseñaba los `scope` de sesión
/// (`app-ghostty-surface-transient-13530.scope`), los `dbus-…@0.service` y los
/// `<hash>.timer` como si fueran contenedores. Todos tienen `SourcePath` vacío
/// —o `UnitFileState=transient`—, y una quadlet **siempre** sale de un fichero.
#[tokio::test]
#[ignore = "necesita el bus de systemd del usuario"]
async fn el_listado_no_trae_unidades_transitorias() {
    let cliente = cliente().await;

    for quadlet in quadlets_o_aviso(&cliente).await {
        let nombre = quadlet.name.as_str();
        let origen = quadlet.origin.as_str();

        assert!(
            !matches!(quadlet.unit_file_state, UnitFileState::Transient),
            "`{nombre}` se listó como quadlet siendo una unidad transitoria"
        );
        assert!(
            !quadlet.origin.es_desconocido(),
            "`{nombre}` se listó como quadlet sin fichero de origen (¿transitoria?)"
        );
        assert!(
            !nombre.ends_with(".scope"),
            "`{nombre}` se listó como quadlet y es un scope de sesión"
        );
        assert!(
            !origen.ends_with(".desktop"),
            "`{nombre}` se listó como quadlet y viene de un `.desktop` de escritorio: {origen}"
        );
    }
}

/// Un nombre con `.` viaja a D-Bus escapado, y el gestor lo reconoce.
///
/// El `_2e` no es decorativo: es la única forma de que una ruta que lleva un
/// punto en el nombre sea una ruta de D-Bus válida.
#[tokio::test]
#[ignore = "necesita el bus de systemd del usuario"]
async fn el_escape_del_nombre_es_el_que_usa_el_gestor() {
    let cliente = cliente().await;

    let Some(quadlet) = quadlets_o_aviso(&cliente)
        .await
        .into_iter()
        .find(|quadlet| quadlet.name.as_str().contains('.'))
    else {
        eprintln!("ninguna quadlet con un punto en el nombre: nada que escapar");
        return;
    };

    let unidad = UnitName::parse(quadlet.name.as_str()).expect("el nombre viene del gestor");

    assert!(
        unidad.object_path().contains("_2e"),
        "un nombre con punto tiene que escapar el punto: {}",
        unidad.object_path()
    );

    // Y con la ruta escapada, el gestor contesta: la unidad existe y es la misma.
    let estado = cliente
        .status(&unidad)
        .await
        .expect("el gestor tiene que reconocer la ruta escapada");

    assert_eq!(estado.name, unidad);
}

/// Escenario «Arrancar una unidad inactiva», medido sobre el bus: la operación
/// vuelve cuando el job termina, no cuando se pide.
///
/// Se arranca una unidad que **ya está activa**, así que la operación no cambia
/// nada de la máquina: lo que se mide es que `StartUnit` devuelva un job y que el
/// `JobRemoved` de ese job llegue —que es la mitad del requisito que ninguna
/// prueba con el doble puede tocar—.
#[tokio::test]
#[ignore = "necesita el bus de systemd del usuario"]
async fn arrancar_lo_que_ya_esta_activo_termina_cuando_termina_el_job() {
    let cliente = cliente().await;

    let Some(activa) = quadlets_o_aviso(&cliente)
        .await
        .into_iter()
        .find(|quadlet| quadlet.active_state == ActiveState::Active)
    else {
        eprintln!("ninguna quadlet activa: no se arranca nada para no cambiar la máquina");
        return;
    };

    let unidad = UnitName::parse(activa.name.as_str()).expect("el nombre viene del gestor");

    let estado = cliente
        .action(&unidad, UnitAction::Start)
        .await
        .expect("arrancar una unidad que ya está activa no puede fallar");

    assert_eq!(estado.name, unidad);
    assert_eq!(
        estado.active_state,
        ActiveState::Active,
        "la unidad estaba activa y tenía que seguir estándolo"
    );
}

/// Una unidad que no existe no se puede accionar, y el error nombra la unidad —lo
/// que de verdad contesta el bus por una ruta desconocida—.
#[tokio::test]
#[ignore = "necesita el bus de systemd del usuario"]
async fn una_unidad_que_no_existe_no_se_puede_accionar() {
    let cliente = cliente().await;

    // Un nombre con pinta de unidad y que no existe en ningún gestor.
    let fantasma = UnitName::parse("podpot-no-existe-de-verdad.service")
        .expect("es un nombre de unidad válido");

    let error = cliente
        .action(&fantasma, UnitAction::Start)
        .await
        .expect_err("arrancar una unidad que no existe no puede salir bien");

    assert!(
        matches!(&error, podpot::systemd::SystemdError::UnitNotFound { unit } if unit == fantasma.as_str()),
        "se esperaba «no existe» nombrando la unidad: {error}"
    );
}

/// Escenarios «Señal de cambio reenviada como notificación» y «Alta o baja de
/// unidad», medidos sobre el bus de verdad.
///
/// Un `daemon-reload` —que es `Reload`— emite `UnitFilesChanged` en el gestor, así
/// que es la señal más barata de provocar sin tocar ninguna unidad: no cambia el
/// estado de nada. Y mide las dos mitades del vigilante que ninguna prueba con el
/// doble puede tocar: que el `Subscribe()` previo hace que systemd emita, y que la
/// regla de la señal de propiedades y los `receive_signal` del gestor están bien
/// escritos.
///
/// La notificación se espera con un plazo generoso porque el *debounce* del
/// vigilante tiene que cerrar la ráfaga antes de avisar.
#[tokio::test]
#[ignore = "necesita el bus de systemd del usuario"]
async fn un_daemon_reload_llega_como_notificacion() {
    let cliente = cliente().await;
    let mut avisos = cliente.subscribe();

    // La suscripción tiene que estar hecha antes de provocar la señal: el canal no
    // guarda historia para quien llega tarde.
    cliente
        .reload()
        .await
        .expect("el gestor tiene que poder recargar su configuración");

    esperar_aviso(&mut avisos).await;
}

/// El camino que de verdad usa el panel: **arrancar y parar** una quadlet emite
/// `PropertiesChanged` sobre su unidad, y el vigilante lo reenvía.
///
/// Es lo que ninguna prueba con el doble puede tocar —«arranca / para / falla»— y
/// lo que mide que la regla del **espacio de rutas** de las unidades alcanza a una
/// unidad **generada**, que es como aparecen las quadlets.
///
/// La quadlet es un `.volume` de prueba —lo más inocuo que genera una quadlet: no
/// arranca ningún contenedor ni toca la red— y se borra al terminar, salga como
/// salga la prueba. No se usa ninguna quadlet del usuario a propósito: pararla
/// cambiaría la máquina.
#[tokio::test]
#[ignore = "necesita el bus de systemd del usuario"]
async fn arrancar_y_parar_una_quadlet_de_prueba_llega_como_notificacion() {
    let cliente = cliente().await;
    let prueba =
        QuadletDePrueba::nueva().expect("se tiene que poder escribir la quadlet de prueba");

    // La suscripción va **antes** de provocar cualquier cambio: el canal no guarda
    // historia, así que quien llega tarde no se entera de nada.
    let mut avisos = cliente.subscribe();

    // La unidad no existe para el gestor hasta que recarga su configuración.
    cliente
        .reload()
        .await
        .expect("el gestor tiene que poder recargar su configuración");

    // Y la recarga avisa por su cuenta: se drena su aviso **antes** de accionar
    // nada. Sin drenarlo, el `esperar_aviso` de más abajo se conformaría con él y la
    // prueba pasaría aunque arrancar la quadlet no emitiera ni una señal.
    esperar_aviso(&mut avisos).await;

    cliente
        .action(&prueba.unidad, UnitAction::Start)
        .await
        .expect("una quadlet de volumen tiene que poder arrancarse");

    esperar_aviso(&mut avisos).await;

    cliente
        .action(&prueba.unidad, UnitAction::Stop)
        .await
        .expect("la quadlet de volumen tiene que poder pararse");

    esperar_aviso(&mut avisos).await;

    // Se borra la quadlet y se recarga para no dejar la unidad cargada en el
    // gestor del usuario.
    prueba
        .borrar()
        .expect("se tiene que poder borrar la quadlet");
    cliente
        .reload()
        .await
        .expect("el gestor tiene que poder recargar su configuración");
}

/// Espera a que llegue una notificación de cambio, con el plazo de la casa.
async fn esperar_aviso(avisos: &mut tokio::sync::broadcast::Receiver<()>) {
    tokio::time::timeout(ESPERA_DE_AVISO, avisos.recv())
        .await
        .expect("el cambio tiene que llegar como notificación antes del plazo")
        .expect("el canal sigue vivo mientras el cliente exista");
}

/// Una quadlet de prueba —un `.volume` vacío— en el directorio de unidades del
/// usuario.
///
/// Se borra al soltarse, para no dejar basura en el directorio del usuario ni
/// unidades fantasma en el gestor.
struct QuadletDePrueba {
    /// El fichero de la quadlet.
    fichero: std::path::PathBuf,
    /// La unidad que el gestor genera a partir de ese fichero.
    unidad: UnitName,
}

impl QuadletDePrueba {
    /// Escribe la quadlet y devuelve su nombre de unidad.
    fn nueva() -> std::io::Result<Self> {
        let directorio = directorio_de_quadlets()?;
        std::fs::create_dir_all(&directorio)?;

        // El nombre lleva el pid para que dos ejecuciones —seguidas o a la vez— no
        // se pisen el fichero ni la unidad.
        let proceso = std::process::id();
        let fichero = directorio.join(format!("podpot-prueba-{proceso}.volume"));
        std::fs::write(&fichero, "[Volume]\n")?;

        // `<stem>.volume` genera la unidad `<stem>-volume.service`: el nombre de la
        // unidad sale del basename del fichero, no del contenido.
        let unidad = UnitName::parse(&format!("podpot-prueba-{proceso}-volume.service"))
            .expect("el nombre de la unidad de prueba es válido");

        Ok(Self { fichero, unidad })
    }

    /// Borra el fichero de la quadlet; borrarlo dos veces no es un error.
    fn borrar(&self) -> std::io::Result<()> {
        match std::fs::remove_file(&self.fichero) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Drop for QuadletDePrueba {
    /// La quadlet se borra al terminar la prueba, salga como salga.
    fn drop(&mut self) {
        let _ = self.borrar();
    }
}

/// El directorio en el que el usuario pone sus quadlets:
/// `$XDG_CONFIG_HOME/containers/systemd` o, sin la variable,
/// `$HOME/.config/containers/systemd`.
fn directorio_de_quadlets() -> std::io::Result<std::path::PathBuf> {
    let config = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(config) if !config.is_empty() => std::path::PathBuf::from(config),
        _ => {
            let home = std::env::var_os("HOME").ok_or_else(|| {
                std::io::Error::other("sin HOME no se puede saber dónde van las quadlets")
            })?;

            std::path::PathBuf::from(home).join(".config")
        }
    };

    Ok(config.join("containers/systemd"))
}
