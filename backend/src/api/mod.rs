//! El servidor HTTP de podpot.
//!
//! Todo lo que cuelga de `/api/v1/` responde con la misma envoltura: los datos
//! dentro de `data` y los fallos dentro de `error`.
//!
//! Fuera de `/api/v1/` está el panel: si la configuración apunta a un build
//! estático, se sirve desde ahí y **cualquier** ruta que no sea de la API cae al
//! `index.html` —la reserva que necesita una SPA, que resuelve sus rutas en el
//! cliente—. Sin panel, lo que no es de la API es un `404` con la envoltura.

pub mod health;
pub mod response;
pub mod units;

use std::path::Path;

use axum::Router;
use axum::routing::{get, post};
use tower_http::services::{ServeDir, ServeFile};

use crate::error::AppError;
use crate::state::AppState;

/// El prefijo bajo el que vive toda la API.
///
/// Es lo que separa las dos mitades del router: por debajo, envoltura JSON; por
/// encima, el panel. Está aquí y no repetido en cada ruta porque es una sola
/// decisión.
const API_PREFIX: &str = "/api/v1";

/// El fichero al que se reserva una ruta de la SPA que no es un fichero.
const PANEL_INDEX: &str = "index.html";

/// Compone el router de la aplicación con su estado.
///
/// Las dos trampas que el router cierra a mano son las de la envoltura única:
/// una ruta que no existe y un método que no está declarado contestan por
/// defecto con un cuerpo vacío, y el contrato del §1 exige la envoltura **también
/// ahí**, así que las dos se sustituyen por manejadores propios —y por eso la API
/// va anidada: así su `404` no se confunde con la reserva del panel—.
///
/// La acción de una unidad va en la ruta —`…/{name}/{action}`— porque es lo que
/// la identifica: `start` sobre una unidad y `stop` sobre la misma unidad son
/// dos recursos distintos, y con la acción en el cuerpo cada petición tendría que
/// llevar uno para decir qué quiere.
pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health::health))
        .route("/units", get(units::listar))
        .route("/units/{name}/{action}", post(units::accionar))
        .fallback(ruta_inexistente)
        .method_not_allowed_fallback(metodo_no_permitido);

    let app = Router::new().nest(API_PREFIX, api);

    // El fallback del router es lo que atiende todo lo que no es de la API: con
    // panel, sus ficheros (y el `index.html` para lo que no sea fichero); sin
    // panel, el `404` de la casa.
    match state.config.static_dir.as_deref() {
        Some(dir) => app.fallback_service(panel(dir)),
        None => app.fallback(ruta_inexistente),
    }
    .with_state(state)
}

/// El servicio del panel: el build estático, con reserva a `index.html`.
///
/// `ServeDir` sirve el fichero que corresponde a la ruta —y el `index.html` del
/// directorio cuando la ruta es un directorio—; lo que **no** encuentra lo
/// contesta su servicio de reserva, que es el `index.html` de la raíz. Ésa es la
/// reserva SPA: una ruta como `/unidades/1` no es un fichero, y el panel la
/// resuelve en el cliente.
///
/// Se usa `fallback` y no `not_found_service` porque la reserva SPA es una
/// **respuesta buena** (el `index.html` con su `200`), no un `404` con cuerpo de
/// otro: `not_found_service` forzaría el `404` sobre la reserva y el navegador
/// trataría de cargar el `index.html` como si faltase.
fn panel(dir: &Path) -> ServeDir<ServeFile> {
    ServeDir::new(dir).fallback(ServeFile::new(dir.join(PANEL_INDEX)))
}

/// El `404` de la casa: una ruta que no existe, con la envoltura de error.
async fn ruta_inexistente() -> AppError {
    AppError::NotFound("la ruta solicitada no existe".to_owned())
}

/// El `405` de la casa: la ruta existe pero el método no está declarado.
async fn metodo_no_permitido() -> AppError {
    AppError::MethodNotAllowed("el método no está permitido en esta ruta".to_owned())
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, StatusCode};

    use crate::test_support::{entorno, entorno_con_panel, peticion, peticion_texto};

    /// Un directorio de panel con un `index.html` reconocible, para las pruebas
    /// que necesitan uno. Se devuelve el `TempDir` para que viva hasta el final:
    /// soltarlo borraría el directorio que se está sirviendo.
    fn panel_de_prueba() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");

        std::fs::write(
            dir.path().join("index.html"),
            "<!doctype html><title>podpot</title>",
        )
        .expect("el index del panel de prueba se tiene que poder escribir");

        dir
    }

    /// Escenario «Salud disponible»: sin autenticación, `200` y el modo de escucha.
    #[tokio::test]
    async fn la_ruta_de_salud_esta_viva_y_dice_donde_escucha() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/health").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cuerpo["data"]["status"], "ok");
        assert_eq!(cuerpo["data"]["bind_address"], "127.0.0.1:8080");
    }

    /// Escenario «Recurso desconocido»: también en los errores, la envoltura.
    #[tokio::test]
    async fn una_ruta_inexistente_da_404_con_la_envoltura() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/no-existe").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            cuerpo["error"]["code"].as_str().is_some(),
            "el 404 tiene que traer la envoltura de error: {cuerpo}"
        );
        assert!(
            cuerpo["error"]["message"].as_str().is_some(),
            "el 404 tiene que explicar el motivo: {cuerpo}"
        );
    }

    /// Escenario «Método no permitido»: `405` con la misma envoltura.
    #[tokio::test]
    async fn un_metodo_no_permitido_sobre_una_ruta_existente_da_405() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::POST, "/api/v1/health").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// Escenario «Panel en la raíz»: con panel configurado, `/` sirve el
    /// `index.html` del build.
    #[tokio::test]
    async fn la_raiz_sirve_el_index_del_panel() {
        let panel = panel_de_prueba();
        let entorno = entorno_con_panel(panel.path()).await;

        let (status, cuerpo) = peticion_texto(&entorno.app(), Method::GET, "/").await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            cuerpo.contains("podpot"),
            "la raíz tiene que servir el index del panel: {cuerpo}"
        );
    }

    /// Escenario «Ruta de la SPA»: una ruta que no es un fichero del build cae al
    /// `index.html`, que es lo que necesita una SPA que resuelve en el cliente.
    #[tokio::test]
    async fn una_ruta_de_la_spa_cae_al_index() {
        let panel = panel_de_prueba();
        let entorno = entorno_con_panel(panel.path()).await;

        let (status, cuerpo) = peticion_texto(&entorno.app(), Method::GET, "/unidades/1").await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            cuerpo.contains("podpot"),
            "una ruta desconocida tiene que caer al index del panel: {cuerpo}"
        );
    }

    /// El panel no se come a la API: con panel configurado, un recurso de la API
    /// que no existe sigue dando el `404` de la casa con su envoltura.
    #[tokio::test]
    async fn el_panel_no_se_come_el_404_de_la_api() {
        let panel = panel_de_prueba();
        let entorno = entorno_con_panel(panel.path()).await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/no-existe").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(cuerpo["error"]["code"], "not_found");
    }

    /// Y tampoco su `405`: el método no declarado de una ruta de la API sigue
    /// saliendo con la envoltura, no con el `index.html`.
    #[tokio::test]
    async fn el_panel_no_se_come_el_405_de_la_api() {
        let panel = panel_de_prueba();
        let entorno = entorno_con_panel(panel.path()).await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::POST, "/api/v1/health").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// Escenario «Sin panel»: sin `PODPOT_STATIC_DIR` la aplicación arranca sólo
    /// como API, y lo que no es de la API es el `404` de la casa.
    #[tokio::test]
    async fn sin_panel_lo_que_no_es_de_la_api_da_404() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(cuerpo["error"]["code"], "not_found");
    }
}
