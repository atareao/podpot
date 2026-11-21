//! El binario de podpot: sólo la composición.
//!
//! Aquí no hay lógica: se lee la configuración, se levanta el registro, se abre
//! la base, se elige el cliente de systemd, se compone el estado y se escucha
//! hasta que llega una señal de terminación. Todo lo demás vive en la
//! biblioteca, donde las pruebas lo alcanzan.

use std::process::ExitCode;
use std::sync::Arc;

use tokio::sync::{broadcast, watch};

use podpot::config::{Config, DEFAULT_LOG_FILTER, SystemdBackend};
use podpot::state::{AppState, CAPACIDAD_DE_EVENTOS};
use podpot::systemd::dbus::DbusSystemdClient;
use podpot::systemd::fake::FakeSystemd;
use podpot::systemd::{SystemdClient, SystemdError, UnitWatcher};
use podpot::{db, fanout, server};

#[tokio::main]
async fn main() -> ExitCode {
    // `.env` es una comodidad de desarrollo: si no está, no pasa nada.
    let _ = dotenvy::dotenv();

    // 1. Configuración. Un fallo no arranca el proceso, y el mensaje nombra la
    //    variable que falta y nunca su valor. Sale por `stderr` porque el registro
    //    todavía no está en pie.
    let mut config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("podpot no puede arrancar: {error}");
            return ExitCode::FAILURE;
        }
    };

    // 2. Registro, con el filtro de la configuración.
    init_tracing(&config.log_filter);

    // 3. El panel, contra el disco: definido **y** existente, o nada. Va después
    //    de levantar el registro para que el aviso de un `PODPOT_STATIC_DIR` que
    //    no existe llegue a alguien; un panel ausente no impide el arranque.
    config.confirmar_panel();

    tracing::info!(config = %config.describe(), "configuración efectiva al arrancar");

    // 4. La base, ya migrada: cuando `preparar` vuelve, el esquema está al día.
    let pool = match db::preparar(&config.database_url).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("podpot no puede arrancar: {error}");
            return ExitCode::FAILURE;
        }
    };

    tracing::info!(systemd = %config.systemd_backend, "base lista");

    // 5. El cliente de systemd. La elección se toma aquí y en ningún otro sitio:
    //    el estado se queda con el objeto que se le dé, así que ninguna ruta tiene
    //    que saber si por debajo hay bus o doble. El mismo objeto se entrega como
    //    **dos** trazos: el listado que contesta preguntas y el vigilante que avisa
    //    de los cambios.
    let (systemd, vigilante) = match cliente_de_systemd(config.systemd_backend).await {
        Ok(par) => par,
        Err(error) => {
            eprintln!("podpot no puede arrancar: {error}");
            pool.close().await;
            return ExitCode::FAILURE;
        }
    };

    // 6. Los dos canales del canal en vivo. Sus extremos «de escritura» se quedan
    //    aquí: el de los eventos lo comparte el estado —el fan-out publica por él—
    //    y el del cierre lo dispara la señal de terminación.
    let (eventos, _) = broadcast::channel(CAPACIDAD_DE_EVENTOS);
    let (disparador, apagado) = watch::channel(false);

    // 7. Escucha y apagado: el estado se compone con las piezas ya resueltas.
    let estado = AppState::new(config, pool.clone(), systemd, eventos, apagado);
    let direccion = estado.bind_address();

    let listener = match tokio::net::TcpListener::bind(direccion).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("podpot no puede arrancar: no se puede escuchar en {direccion}: {error}");
            pool.close().await;
            return ExitCode::FAILURE;
        }
    };

    tracing::info!(%direccion, "escuchando");

    // 8. El fan-out: consume los avisos del vigilante, recalcula la instantánea y
    //    la publica en el canal que lee el stream. Se arranca antes de escuchar
    //    para que ningún cambio entre el arranque y la primera petición se pierda.
    let fanout = fanout::arrancar(estado.clone(), vigilante.as_ref());

    // 9. El cierre: la señal de terminación apaga el canal y con él los flujos SSE
    //    y el fan-out, **antes** de que el apagado ordenado espere a las conexiones
    //    en curso —si no, un stream perpetuo lo colgaría—.
    let cierre = async move {
        server::shutdown_signal().await;
        // Sin receptores el envío falla, y no es un error: nadie mira.
        let _ = disparador.send(true);
    };

    let resultado = server::serve(estado, listener, cierre).await;

    // El fan-out mira el mismo canal de cierre, así que termina solo; se espera
    // para no dejar una tarea a la espalda.
    let _ = fanout.await;

    // La escucha ha terminado sin nadie atendiendo: el pool se cierra antes de
    // salir, que es la mitad de «liberar los recursos» del apagado ordenado.
    pool.close().await;

    match resultado {
        Ok(()) => {
            tracing::info!("apagado ordenado: el proceso termina");
            ExitCode::SUCCESS
        }
        Err(error) => {
            // El registro puede estar apagado (`RUST_LOG=off`), así que el fallo
            // fatal también sale por `stderr`.
            eprintln!("el servidor se detuvo con error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// El cliente de systemd que pide la configuración, con sus **dos** trazos.
///
/// El doble de desarrollo no conecta con nada —sus unidades son de muestra— y el
/// modo de verdad es el bus de **usuario**: si no hay bus, el proceso no arranca,
/// porque un panel que enseñara el doble creyendo que enseña la máquina sería
/// peor que un panel que no arranca.
///
/// Devuelve el mismo objeto dos veces con dos formas distintas —el listado que
/// contesta preguntas y el vigilante que avisa de los cambios— porque tienen que
/// ser el **mismo** objeto: con dos distintos, un aviso no tendría nada que ver con
/// lo que se lista.
async fn cliente_de_systemd(
    backend: SystemdBackend,
) -> Result<(Arc<dyn SystemdClient>, Arc<dyn UnitWatcher>), SystemdError> {
    match backend {
        SystemdBackend::Fake => {
            let doble = Arc::new(FakeSystemd::demo());
            let cliente: Arc<dyn SystemdClient> = doble.clone();
            let vigilante: Arc<dyn UnitWatcher> = doble;

            Ok((cliente, vigilante))
        }
        SystemdBackend::Dbus => {
            let real = Arc::new(DbusSystemdClient::connect_user().await?);
            let cliente: Arc<dyn SystemdClient> = real.clone();
            let vigilante: Arc<dyn UnitWatcher> = real;

            Ok((cliente, vigilante))
        }
    }
}

/// Inicializa el registro con el filtro ya resuelto por la configuración.
///
/// El filtro entra por parámetro en vez de volver a leer el entorno: el
/// `EnvFilter` que se aplica y el que se vuelca en el arranque tienen que ser el
/// mismo, o el volcado miente.
fn init_tracing(filtro: &str) {
    let filter = match tracing_subscriber::EnvFilter::try_new(filtro) {
        Ok(filter) => filter,
        Err(error) => {
            eprintln!(
                "el filtro de registro {filtro:?} no es válido: {error}; se usa {DEFAULT_LOG_FILTER}"
            );
            tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER)
        }
    };

    if let Err(error) = tracing_subscriber::fmt().with_env_filter(filter).try_init() {
        eprintln!("no se pudo inicializar el registro: {error}");
    }
}
