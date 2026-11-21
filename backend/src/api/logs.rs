//! El canal de logs de una quadlet: `GET /api/v1/units/{name}/logs/stream`.
//!
//! Es el segundo flujo `text/event-stream` de la casa y comparte el patrón del
//! canal en vivo ([`crate::api::stream`]): no lleva la envoltura JSON de fuera
//! —una respuesta continua no tiene un `data` único—, pero el **contenido de cada
//! evento** sí la reutiliza, para que el panel analice el `data` con el mismo
//! tipo que el resto de la API.
//!
//! A diferencia del canal en vivo, aquí el flujo **termina**: los logs de un
//! contenedor que se para se acaban, así que el evento terminal y el cierre
//! ordenado son parte del contrato, no un caso raro.
//!
//! Lo que se lee del contenedor no lo decide este módulo: entra por el puerto
//! [`crate::registros::FuenteDeRegistros`], así que el manejador no conoce ni el
//! socket de Podman ni el formato de los marcos —y el canal entero se puede probar
//! con el doble.

use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::Sse;
use axum::response::sse::{Event, KeepAlive};
use futures_util::stream::{self, Stream};
use tokio::sync::{mpsc, watch};

use crate::api::response::ApiResponse;
use crate::error::AppError;
use crate::registros::{COLA_INICIAL, LineaDeRegistro};
use crate::state::AppState;
use crate::systemd::UnitName;

/// El nombre del evento que lleva cada línea.
///
/// El panel lo escucha por su nombre (`addEventListener("log", …)`): sin nombre,
/// el navegador lo entregaría como un `message` genérico y el cliente no podría
/// distinguirlo del evento terminal.
const EVENTO: &str = "log";

/// El nombre del evento que marca el fin del stream.
///
/// Va **sin `data`**: no lleva una línea, lleva el hecho de que ya no habrá más.
/// El panel lo usa para saber que el contenedor dejó de producir; el cuerpo del
/// evento no tiene nada que contar.
const EVENTO_TERMINAL: &str = "log-end";

/// Cuánto espera el cliente antes de reconectar si el flujo se corta.
///
/// Tres segundos es lo que el navegador usará entre reintentos tras una caída de
/// red: ni un martilleo contra el servidor ni una espera que deje el visor muerto.
const RETRY: Duration = Duration::from_secs(3);

/// Cada cuánto se manda un latido cuando no hay líneas nuevas.
///
/// Un proxy con la conexión ociosa la corta —algunos a los pocos segundos—, así
/// que el flujo manda un comentario de vez en cuando para que la conexión no
/// parezca muerta. Quince segundos es el valor por defecto de axum y el umbral que
/// usan la mayoría de los proxies.
const LATIDO: Duration = Duration::from_secs(15);

/// `GET /api/v1/units/{name}/logs/stream` — los logs del contenedor, en vivo.
///
/// Empieza a seguir el contenedor de la unidad y sirve su **cola inicial** —las
/// últimas [`COLA_INICIAL`] líneas— y después cada línea nueva.
///
/// Si la unidad no tiene contenedor —un `.volume`, un `.network`— o Podman no
/// contesta, la petición falla **antes** de abrir el flujo: un `404` o un `503`
/// con la envoltura de siempre, no un `text/event-stream` que nunca trae nada.
pub async fn stream(
    State(state): State<AppState>,
    Path(nombre): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::io::Error>> + Send + 'static>, AppError> {
    let unidad = UnitName::parse(&nombre)?;

    let lineas = state.registros.seguir(&unidad, COLA_INICIAL).await?;

    Ok(flujo(lineas, state.apagado.clone(), LATIDO))
}

/// Compone la respuesta SSE a partir del canal de líneas y el cierre.
///
/// El intervalo del latido entra por parámetro —y no se lee del módulo— para que
/// la prueba pueda pedir uno de milisegundos y medir el latido sin esperar quince
/// segundos.
fn flujo(
    lineas: mpsc::Receiver<LineaDeRegistro>,
    apagado: watch::Receiver<bool>,
    latido: Duration,
) -> Sse<impl Stream<Item = Result<Event, std::io::Error>> + Send + 'static> {
    Sse::new(registros(lineas, apagado))
        .keep_alive(KeepAlive::new().interval(latido).text("latido"))
}

/// El flujo de eventos: una línea por evento, y un evento terminal al final.
///
/// Termina de tres maneras, y las tres importan: cuando el stream de Podman se
/// acaba —el contenedor dejó de escribir—, cuando el proceso se apaga, y cuando el
/// cliente se va (axum suelta el flujo, el receptor se cierra y la tarea que lee
/// del socket termina). Las dos primeras se distinguen a propósito: el fin del
/// stream lleva evento terminal, porque el panel tiene que saber que el contenedor
/// dejó de producir; el apagado cierra sin más, porque el servidor se está yendo y
/// no hay a quién avisar.
///
/// El primer cuadro es **sólo el `retry`**, sin `data` ni nombre de evento: así el
/// cliente conoce cuánto esperar antes de reconectar aunque el contenedor no haya
/// escrito todavía una línea —y ese cuadro no dispara ningún evento en el panel—.
fn registros(
    lineas: mpsc::Receiver<LineaDeRegistro>,
    apagado: watch::Receiver<bool>,
) -> impl Stream<Item = Result<Event, std::io::Error>> + Send + 'static {
    stream::unfold(
        Estado {
            lineas,
            apagado,
            anunciado: false,
            terminado: false,
        },
        |mut estado| async move {
            if !estado.anunciado {
                estado.anunciado = true;

                return Some((Ok(Event::default().retry(RETRY)), estado));
            }

            // Tras el evento terminal ya no queda nada: sin esta guarda, el
            // receptor cerrado volvería a dar `None` en cada vuelta y el evento
            // terminal se repetiría sin fin.
            if estado.terminado {
                return None;
            }

            let evento = tokio::select! {
                biased;

                // El cierre se mira primero: apagándose no se espera a más líneas,
                // se termina el flujo para no colgar el apagado ordenado.
                cierre = estado.apagado.wait_for(|apagado| *apagado) => {
                    // Si el disparador se soltó (`Err`), tampoco hay a quién
                    // servir: el flujo acaba igual.
                    let _ = cierre;
                    return None;
                }
                linea = estado.lineas.recv() => match linea {
                    Some(linea) => evento_de_linea(linea),
                    // El contenedor dejó de producir: evento terminal y a cerrar.
                    None => {
                        estado.terminado = true;
                        evento_terminal()
                    }
                },
            };

            Some((evento, estado))
        },
    )
}

/// Por dónde va el flujo: lo que hay que recordar entre evento y evento.
struct Estado {
    /// El canal del que salen las líneas.
    lineas: mpsc::Receiver<LineaDeRegistro>,
    /// El cierre del proceso, que termina el flujo.
    apagado: watch::Receiver<bool>,
    /// Si ya se anunció el `retry` —lo lleva el primer cuadro—.
    anunciado: bool,
    /// Si ya se emitió el evento terminal.
    terminado: bool,
}

/// Traduce una línea de dominio a un evento SSE con la envoltura `{data}`.
///
/// El `data` lleva el mismo `{ "data": { "line": …, "stream": … } }` que el resto
/// de la API, así que el panel analiza el evento con el tipo `LineaDeRegistro` sin
/// un analizador aparte.
fn evento_de_linea(linea: LineaDeRegistro) -> Result<Event, std::io::Error> {
    let cuerpo = serde_json::to_string(&ApiResponse::new(linea)).map_err(|error| {
        tracing::error!(%error, "no se pudo serializar una línea de registros");
        std::io::Error::other(error)
    })?;

    Ok(Event::default().event(EVENTO).retry(RETRY).data(cuerpo))
}

/// El evento terminal: nombrado, con `retry` y **sin** `data`.
fn evento_terminal() -> Result<Event, std::io::Error> {
    Ok(Event::default().event(EVENTO_TERMINAL).retry(RETRY))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::BodyDataStream;
    use axum::http::{Method, StatusCode, header};
    use axum::response::IntoResponse;
    use futures_util::StreamExt;
    use tokio::sync::mpsc;

    use crate::registros::fake::FakeRegistros;
    use crate::registros::{CAPACIDAD_DE_REGISTROS, LineaDeRegistro, Origen};
    use crate::test_support::{
        body_json, cookie_de_sesion, entorno_con_registros, entorno_oidc_con_registros,
        header_texto, peticion, peticion_cruda, peticion_cruda_con_cookie,
    };

    use super::{EVENTO, EVENTO_TERMINAL, LATIDO, RETRY, flujo};

    /// El plazo de las esperas: de sobra para un flujo en memoria, corto para que
    /// una prueba rota no cuelgue la suite.
    const ESPERA: Duration = Duration::from_secs(5);

    /// Lee del cuerpo hasta tener un cuadro SSE completo —termina en línea en
    /// blanco—.
    async fn leer_cuadro(cuerpo: &mut BodyDataStream) -> String {
        let mut cuadro = String::new();

        loop {
            let trozo = tokio::time::timeout(ESPERA, cuerpo.next())
                .await
                .expect("el flujo tiene que mandar algo antes del plazo")
                .expect("el flujo sigue abierto")
                .expect("el cuerpo del flujo no puede fallar");

            cuadro.push_str(&String::from_utf8_lossy(&trozo));

            if cuadro.contains("\n\n") {
                return cuadro;
            }
        }
    }

    /// Lee el siguiente cuadro **con contenido**: salta el primer cuadro del
    /// `retry`, que no lleva ni `data` ni nombre de evento.
    async fn leer_evento(cuerpo: &mut BodyDataStream) -> String {
        loop {
            let cuadro = leer_cuadro(cuerpo).await;

            if cuadro.contains("data:") || cuadro.contains("event:") {
                return cuadro;
            }
        }
    }

    /// El JSON del campo `data` de un cuadro SSE.
    fn data_del_cuadro(cuadro: &str) -> serde_json::Value {
        let json = cuadro
            .lines()
            .find_map(|linea| linea.strip_prefix("data: "))
            .unwrap_or_else(|| panic!("el cuadro no traía campo `data`: {cuadro:?}"));

        serde_json::from_str(json).expect("el `data` del evento es JSON")
    }

    /// Escenario «Cola inicial al conectar»: al abrir el canal llegan primero las
    /// líneas que ya tenía el contenedor, sin esperar a que se produzcan otras.
    #[tokio::test]
    async fn al_conectar_llega_primero_la_cola_inicial() {
        let doble = Arc::new(FakeRegistros::con([
            LineaDeRegistro::nueva("una vieja", Origen::Stdout),
            LineaDeRegistro::nueva("otra vieja", Origen::Stderr),
        ]));
        let entorno = entorno_con_registros(doble).await;

        let respuesta = peticion_cruda(
            &entorno.app(),
            Method::GET,
            "/api/v1/units/centinela-db.service/logs/stream",
        )
        .await;

        assert_eq!(respuesta.status(), StatusCode::OK);
        assert_eq!(
            header_texto(&respuesta, header::CONTENT_TYPE).as_deref(),
            Some("text/event-stream"),
            "el canal de logs se anuncia como text/event-stream"
        );

        let mut cuerpo = respuesta.into_body().into_data_stream();

        let primero = leer_evento(&mut cuerpo).await;
        assert!(
            primero.contains(&format!("event: {EVENTO}")),
            "cada línea va en un evento `log` nombrado: {primero:?}"
        );
        assert_eq!(data_del_cuadro(&primero)["data"]["line"], "una vieja");
        assert_eq!(data_del_cuadro(&primero)["data"]["stream"], "stdout");

        let segundo = leer_evento(&mut cuerpo).await;
        assert_eq!(data_del_cuadro(&segundo)["data"]["line"], "otra vieja");
        assert_eq!(data_del_cuadro(&segundo)["data"]["stream"], "stderr");
    }

    /// Escenario «Líneas nuevas según se producen»: con la conexión abierta, una
    /// línea emitida por el contenedor llega como evento `log` con su origen.
    #[tokio::test]
    async fn una_linea_nueva_llega_como_evento_log() {
        let doble = Arc::new(FakeRegistros::new());
        let entorno = entorno_con_registros(Arc::clone(&doble)).await;

        let respuesta = peticion_cruda(
            &entorno.app(),
            Method::GET,
            "/api/v1/units/centinela-db.service/logs/stream",
        )
        .await;

        let mut cuerpo = respuesta.into_body().into_data_stream();

        doble.emitir(LineaDeRegistro::nueva("recién escrita", Origen::Stdout));

        let cuadro = leer_evento(&mut cuerpo).await;

        assert!(
            cuadro.contains(&format!("event: {EVENTO}")),
            "la línea nueva va en su evento `log`: {cuadro:?}"
        );
        assert_eq!(data_del_cuadro(&cuadro)["data"]["line"], "recién escrita");
    }

    /// Escenario «Fin del stream»: cuando el contenedor deja de producir, el flujo
    /// emite un evento terminal y termina de forma ordenada.
    #[tokio::test]
    async fn el_fin_del_stream_emite_un_evento_terminal_y_cierra() {
        let doble = Arc::new(FakeRegistros::new());
        let entorno = entorno_con_registros(Arc::clone(&doble)).await;

        let respuesta = peticion_cruda(
            &entorno.app(),
            Method::GET,
            "/api/v1/units/centinela-db.service/logs/stream",
        )
        .await;
        let mut cuerpo = respuesta.into_body().into_data_stream();

        doble.cerrar();

        let terminal = leer_evento(&mut cuerpo).await;
        assert!(
            terminal.contains(&format!("event: {EVENTO_TERMINAL}")),
            "el fin del stream tiene que ir marcado con su evento: {terminal:?}"
        );
        assert!(
            !terminal.contains("data:"),
            "el evento terminal no lleva una línea: {terminal:?}"
        );

        let fin = tokio::time::timeout(ESPERA, cuerpo.next())
            .await
            .expect("tras el evento terminal el flujo tiene que acabar");

        assert!(
            fin.is_none(),
            "el flujo tiene que cerrarse tras el evento terminal: {fin:?}"
        );
    }

    /// Escenario «Cierre con el servidor»: al dispararse el cierre, el flujo
    /// termina en vez de quedarse colgado y bloquear el apagado ordenado.
    #[tokio::test]
    async fn el_cierre_termina_el_flujo() {
        let doble = Arc::new(FakeRegistros::con([LineaDeRegistro::nueva(
            "algo",
            Origen::Stdout,
        )]));
        let entorno = entorno_con_registros(doble).await;

        let respuesta = peticion_cruda(
            &entorno.app(),
            Method::GET,
            "/api/v1/units/centinela-db.service/logs/stream",
        )
        .await;
        let mut cuerpo = respuesta.into_body().into_data_stream();

        // El flujo está vivo: manda su primer evento.
        let _ = leer_cuadro(&mut cuerpo).await;

        entorno.cerrar();

        let fin = tokio::time::timeout(ESPERA, cuerpo.next())
            .await
            .expect("el cierre tiene que terminar el flujo antes del plazo");

        assert!(
            fin.is_none(),
            "tras el cierre, el flujo tiene que acabar sin más eventos: {fin:?}"
        );
    }

    /// Escenario «Latido y reconexión»: el flujo indica un `retry` y, sin líneas,
    /// sigue mandando latidos.
    ///
    /// El latido se mide con un intervalo de milisegundos para no esperar los
    /// quince segundos de producción: es la razón por la que el intervalo entra por
    /// parámetro.
    #[tokio::test]
    async fn el_flujo_indica_retry_y_manda_latidos() {
        let entorno = entorno_con_registros(Arc::new(FakeRegistros::new())).await;
        let state = entorno.state().clone();

        let (_emisor, receptor) = mpsc::channel(CAPACIDAD_DE_REGISTROS);

        let respuesta =
            flujo(receptor, state.apagado.clone(), Duration::from_millis(1)).into_response();

        let mut cuerpo = respuesta.into_body().into_data_stream();

        let primero = leer_cuadro(&mut cuerpo).await;
        assert!(
            primero.contains(&format!("retry: {}", RETRY.as_millis())),
            "el flujo tiene que indicar el retry con el que reconectar: {primero:?}"
        );

        let latido = leer_cuadro(&mut cuerpo).await;
        assert!(
            latido.contains(": latido"),
            "sin líneas, el flujo tiene que mandar un latido: {latido:?}"
        );
    }

    /// Escenario «Unidad sin contenedor»: la petición falla con la envoltura de
    /// error **antes** de abrir el flujo, no con un `text/event-stream` vacío.
    #[tokio::test]
    async fn una_unidad_sin_contenedor_da_error_envuelto_y_no_abre_el_flujo() {
        let entorno = entorno_con_registros(Arc::new(FakeRegistros::sin_contenedor())).await;

        let respuesta = peticion_cruda(
            &entorno.app(),
            Method::GET,
            "/api/v1/units/datos-volume.service/logs/stream",
        )
        .await;

        assert_eq!(respuesta.status(), StatusCode::NOT_FOUND);

        let tipo = header_texto(&respuesta, header::CONTENT_TYPE).unwrap_or_default();
        assert!(
            !tipo.starts_with("text/event-stream"),
            "sin contenedor no puede abrirse el flujo: {tipo}"
        );

        let cuerpo = body_json(respuesta).await;
        assert_eq!(cuerpo["error"]["code"], "unit_without_container");
        assert!(
            cuerpo["error"]["message"]
                .as_str()
                .is_some_and(|mensaje| mensaje.contains("datos-volume.service")),
            "el error tiene que nombrar la unidad: {cuerpo}"
        );
    }

    /// Escenario «Método no permitido»: `GET` es el único admitido y cualquier otro
    /// método contesta `405` con la envoltura habitual.
    #[tokio::test]
    async fn un_metodo_distinto_de_get_da_405_con_la_envoltura() {
        let entorno = entorno_con_registros(Arc::new(FakeRegistros::new())).await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/centinela-db.service/logs/stream",
        )
        .await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// Escenario «Sin sesión en modo OIDC»: el canal de logs cuelga de la mitad
    /// protegida, así que sin cookie contesta `401` y **no** abre el flujo.
    #[tokio::test]
    async fn sin_sesion_el_canal_da_401_y_no_abre_el_flujo() {
        let entorno = entorno_oidc_con_registros(Arc::new(FakeRegistros::new())).await;

        let respuesta = peticion_cruda(
            &entorno.app(),
            Method::GET,
            "/api/v1/units/centinela-db.service/logs/stream",
        )
        .await;

        assert_eq!(respuesta.status(), StatusCode::UNAUTHORIZED);

        let tipo = header_texto(&respuesta, header::CONTENT_TYPE).unwrap_or_default();
        assert!(
            !tipo.starts_with("text/event-stream"),
            "sin sesión no puede abrirse el flujo: {tipo}"
        );

        let cuerpo = body_json(respuesta).await;
        assert_eq!(cuerpo["error"]["code"], "unauthorized");
    }

    /// Y con sesión válida el flujo se abre y las líneas del doble llegan por él.
    #[tokio::test]
    async fn con_sesion_el_canal_se_abre_y_entrega_lineas() {
        let doble = Arc::new(FakeRegistros::new());
        let entorno = entorno_oidc_con_registros(Arc::clone(&doble)).await;

        let respuesta = peticion_cruda_con_cookie(
            &entorno.app(),
            Method::GET,
            "/api/v1/units/centinela-db.service/logs/stream",
            &cookie_de_sesion(),
        )
        .await;

        assert_eq!(respuesta.status(), StatusCode::OK);

        let mut cuerpo = respuesta.into_body().into_data_stream();

        doble.emitir(LineaDeRegistro::nueva("con sesión", Origen::Stderr));

        let cuadro = leer_evento(&mut cuerpo).await;
        assert_eq!(data_del_cuadro(&cuadro)["data"]["line"], "con sesión");
    }

    /// El latido y el `retry` de producción son valores sensatos —ni un martilleo
    /// ni una conexión que un proxy pueda dar por muerta—.
    #[test]
    fn el_latido_y_el_retry_son_valores_sensatos() {
        assert!(
            LATIDO >= Duration::from_secs(5),
            "un latido muy corto es ruido"
        );
        assert!(
            LATIDO <= Duration::from_secs(60),
            "un latido muy largo deja que un proxy corte la conexión"
        );
        assert!(
            RETRY >= Duration::from_secs(1),
            "un retry muy corto martillea"
        );
        assert!(
            RETRY <= Duration::from_secs(30),
            "un retry muy largo deja el visor muerto tras una caída"
        );
    }
}
