//! El binario de podpot: sólo la composición.
//!
//! Aquí no hay lógica: se lee la configuración, se levanta el registro, se
//! compone el estado y se escucha hasta que llega una señal de terminación. Todo
//! lo demás vive en la biblioteca, donde las pruebas lo alcanzan.

use std::process::ExitCode;

use podpot::config::{Config, DEFAULT_LOG_FILTER};
use podpot::server;
use podpot::state::AppState;

#[tokio::main]
async fn main() -> ExitCode {
    // `.env` es una comodidad de desarrollo: si no está, no pasa nada.
    let _ = dotenvy::dotenv();

    // 1. Configuración. Un fallo no arranca el proceso, y el mensaje nombra la
    //    variable que falta y nunca su valor. Sale por `stderr` porque el registro
    //    todavía no está en pie.
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("podpot no puede arrancar: {error}");
            return ExitCode::FAILURE;
        }
    };

    // 2. Registro, con el filtro de la configuración.
    init_tracing(&config.log_filter);
    tracing::info!(config = %config.describe(), "configuración efectiva al arrancar");

    // 3. Escucha y apagado: el estado se compone desde la configuración ya resuelta.
    let estado = AppState::new(config);
    let direccion = estado.bind_address();

    let listener = match tokio::net::TcpListener::bind(direccion).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("podpot no puede arrancar: no se puede escuchar en {direccion}: {error}");
            return ExitCode::FAILURE;
        }
    };

    tracing::info!(%direccion, "escuchando");

    let resultado = server::serve(estado, listener, server::shutdown_signal()).await;

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
