//! Pruebas contra la API de Podman del usuario de **verdad**, por su socket.
//!
//! Van marcadas con `#[ignore]` porque necesitan una máquina con un Podman
//! rootless en pie y su socket en `$XDG_RUNTIME_DIR/podman/podman.sock`: no es algo
//! que se pueda exigir a quien sólo quiere compilar el proyecto. Se ejecutan a
//! mano:
//!
//! ```sh
//! cargo test --test registros_real -- --ignored --nocapture
//! ```
//!
//! Y existen porque hay tres cosas que **no** se pueden medir con un servidor
//! simulado:
//!
//! * que la petición de listado, con el filtro por la etiqueta de systemd, sea la
//!   que el Podman de verdad acepta y devuelva el contenedor de la unidad;
//! * que la inspección traiga `Config.Tty` como el cliente lo espera;
//! * que el stream de logs responda `200` y entregue marcos que el
//!   demultiplexador sepa leer.

use std::path::PathBuf;
use std::time::Duration;

use podpot::registros::podman::PodmanRegistros;
use podpot::registros::{COLA_INICIAL, FuenteDeRegistros, RegistrosError};
use podpot::systemd::SystemdClient;
use podpot::systemd::dbus::DbusSystemdClient;

/// El plazo con el que se espera la primera línea del contenedor.
///
/// Corto a propósito: no se está midiendo que el contenedor escriba —puede no
/// hacerlo—, sino que el stream se abrió. Lo que llegue dentro del plazo se
/// informa; lo que no, también.
const ESPERA: Duration = Duration::from_secs(2);

/// El socket de la API de Podman del usuario.
fn socket() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR")
        .expect("estas pruebas necesitan el XDG_RUNTIME_DIR del usuario");

    PathBuf::from(runtime).join("podman/podman.sock")
}

/// El listado de quadlets del gestor de usuario, para tener unidades que mirar.
async fn unidades() -> Vec<podpot::systemd::UnitName> {
    let cliente = DbusSystemdClient::connect_user()
        .await
        .expect("estas pruebas necesitan el bus de systemd del usuario");

    cliente
        .quadlets()
        .await
        .expect("ListUnits tiene que contestar contra el bus de usuario")
        .into_iter()
        .map(|unidad| unidad.name)
        .collect()
}

/// Los logs de una quadlet con contenedor se leen por el socket: listado por
/// etiqueta, inspección de TTY y stream de logs, los tres contra el Podman real.
#[tokio::test]
#[ignore = "necesita el socket de la API de Podman del usuario"]
async fn los_logs_de_un_contenedor_se_leen_por_el_socket() {
    let fuente = PodmanRegistros::new(socket());

    for unidad in unidades().await {
        match fuente.seguir(&unidad, COLA_INICIAL).await {
            Ok(mut receptor) => {
                eprintln!("el stream de `{unidad}` se abrió: el socket responde");

                match tokio::time::timeout(ESPERA, receptor.recv()).await {
                    Ok(Some(linea)) => eprintln!("primera línea: {linea:?}"),
                    Ok(None) => eprintln!("el stream terminó sin entregar ninguna línea"),
                    Err(_) => eprintln!("el contenedor no escribió nada en el plazo"),
                }

                return;
            }
            Err(RegistrosError::SinContenedor { .. }) => continue,
            Err(error) => panic!("el socket de Podman tiene que responder: {error}"),
        }
    }

    eprintln!(
        "ninguna quadlet de la máquina tiene contenedor: la prueba no tiene nada que mirar \
         (arranca una con `systemctl --user start <quadlet>` para ejercerla)"
    );
}

/// Una unidad que no tiene contenedor falla de forma explícita —y contra el
/// socket de verdad, que es donde se mide que el listado vacío se detecte.
#[tokio::test]
#[ignore = "necesita el socket de la API de Podman del usuario"]
async fn una_unidad_sin_contenedor_da_error_explicito() {
    let fuente = PodmanRegistros::new(socket());
    let unidad = podpot::systemd::UnitName::parse("podpot-no-existe-en-esta-maquina.service")
        .expect("el nombre de la unidad de prueba es válido");

    let error = fuente
        .seguir(&unidad, COLA_INICIAL)
        .await
        .expect_err("una unidad que no existe no puede tener contenedor");

    assert!(
        matches!(&error, RegistrosError::SinContenedor { unidad } if unidad == "podpot-no-existe-en-esta-maquina.service"),
        "se esperaba «sin contenedor» nombrando la unidad: {error}"
    );
}
