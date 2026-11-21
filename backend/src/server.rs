//! La escucha y el apagado ordenado del servidor.
//!
//! Este módulo no sabe nada de las rutas: recibe el estado ya compuesto, un
//! `TcpListener` ya abierto y el futuro que anuncia el cierre. Que la señal entre
//! por parámetro —en vez de leerla aquí— es lo que permite medir la composición
//! sin mandar una señal de verdad.

use std::future::Future;

use tokio::net::TcpListener;

use crate::api;
use crate::state::AppState;

/// Sirve la aplicación hasta que `shutdown` resuelva.
///
/// Cuando `shutdown` resuelve, axum deja de aceptar conexiones nuevas y espera a
/// las que están en curso: para cuando esta función vuelve, nadie puede pedir
/// nada nuevo y el estado —con el cliente de D-Bus dentro— se puede soltar.
pub async fn serve(
    state: AppState,
    listener: TcpListener,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(shutdown)
        .await
}

/// El futuro que resuelve cuando llega una señal de terminación —`SIGTERM` o
/// `SIGINT`—.
///
/// Si una de las dos no se puede registrar, se deja de esperar por ella en vez de
/// apagar el servidor: un fallo al escuchar una señal no es una señal.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {}
            Err(error) => {
                tracing::error!(%error, "no se pudo atender SIGINT");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "no se pudo atender SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }

    tracing::info!("señal de terminación recibida: se deja de aceptar conexiones");
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use tokio::sync::oneshot;

    use crate::test_support::{entorno, peticion};
    use axum::Router;
    use axum::http::{Method, StatusCode};

    /// Escenario «Terminación limpia»: cuando el futuro de cierre resuelve, la
    /// escucha termina y `serve` vuelve —que es lo que deja al proceso liberar sus
    /// recursos antes de salir—.
    ///
    /// La señal de verdad (`SIGTERM`) la manda el proceso entero, y esa mitad la
    /// mide `tests/arranque.rs`; aquí se fija la composición, que es lo que una
    /// prueba unitaria puede recorrer sin señales.
    #[tokio::test]
    async fn el_servidor_deja_de_escuchar_cuando_el_cierre_resuelve() {
        let entorno = entorno().await;
        let app: Router = entorno.app();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("se tiene que poder abrir un puerto efímero");
        let direccion: SocketAddr = listener
            .local_addr()
            .expect("un listener abierto tiene dirección local");

        let (cierre, espera) = oneshot::channel::<()>();

        let servidor = tokio::spawn(crate::server::serve(
            entorno.state().clone(),
            listener,
            async move {
                let _ = espera.await;
            },
        ));

        // Mientras el cierre no llega, el puerto atiende.
        let viva = esperar_salud(direccion)
            .await
            .expect("la ruta de salud tiene que responder antes del cierre");
        assert_eq!(viva, StatusCode::OK);

        cierre
            .send(())
            .expect("nadie debería haber soltado el cierre");

        let resultado = tokio::time::timeout(std::time::Duration::from_secs(10), servidor)
            .await
            .expect("el servidor tiene que parar cuando el cierre resuelve")
            .expect("la tarea del servidor no puede entrar en pánico");

        assert!(
            resultado.is_ok(),
            "el apagado ordenado tiene que terminar sin error: {resultado:?}"
        );

        // Y ya no acepta: una segunda consulta no puede llegar a puerto.
        assert!(
            esperar_salud(direccion).await.is_err(),
            "el servidor no puede seguir atendiendo después del cierre"
        );

        // El router sigue siendo utilizable fuera del servidor: la composición no
        // se queda a medias por haberse apagado.
        let (status, _) = peticion(&app, Method::GET, "/api/v1/health").await;
        assert_eq!(status, StatusCode::OK);
    }

    /// Consulta la salud por HTTP contra la dirección dada.
    async fn esperar_salud(direccion: SocketAddr) -> Result<StatusCode, std::io::Error> {
        let respuesta =
            tokio::time::timeout(std::time::Duration::from_secs(5), salud_por_http(direccion))
                .await
                .map_err(|_| {
                    std::io::Error::other("la consulta de salud no respondió a tiempo")
                })??;

        Ok(respuesta)
    }

    /// Un `GET /api/v1/health` con el mínimo de HTTP escrito a mano.
    ///
    /// No se usa un cliente HTTP completo —ni `reqwest` ni `hyper` a mano— porque
    /// lo único que hace falta es saber si el puerto atiende y con qué código
    /// contesta, y eso cabe en una petición `HTTP/1.1` de una línea.
    async fn salud_por_http(direccion: SocketAddr) -> Result<StatusCode, std::io::Error> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut flujo = tokio::net::TcpStream::connect(direccion).await?;

        flujo
            .write_all(
                b"GET /api/v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            )
            .await?;

        let mut respuesta = Vec::new();
        flujo.read_to_end(&mut respuesta).await?;

        let texto = String::from_utf8_lossy(&respuesta);
        let linea = texto
            .lines()
            .next()
            .ok_or_else(|| std::io::Error::other("la respuesta no traía línea de estado"))?;

        // `HTTP/1.1 200 OK`
        let codigo: u16 = linea
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| std::io::Error::other("la línea de estado no traía código"))?
            .parse()
            .map_err(|_| std::io::Error::other("el código de estado no era un número"))?;

        StatusCode::from_u16(codigo)
            .map_err(|_| std::io::Error::other("el código de estado no era válido"))
    }
}
