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
//! Y existen porque hay tres cosas que **no** se pueden medir sin bus:
//!
//! * que `ListUnits` conteste y devuelva el listado con los nombres de unidad que
//!   podpot espera;
//! * que el **escape** del nombre produzca una ruta que el gestor reconozca —una
//!   ruta inventada no falla: por una unidad que no existe, systemd contesta
//!   `inactive` igual que por una parada—;
//! * que el `JobRemoved` del job llegue y la acción vuelva cuando el job termina,
//!   en vez de esperar al plazo.

use podpot::systemd::dbus::DbusSystemdClient;
use podpot::systemd::{
    ActiveState, SystemdClient, UnitAction, UnitFileState, UnitName, UnitStatus,
};

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
