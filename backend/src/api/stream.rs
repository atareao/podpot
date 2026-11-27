//! El canal en vivo: `GET /api/v1/units/stream`.
//!
//! Es la única ruta de la API que no contesta un documento único: devuelve un
//! flujo `text/event-stream` que no termina hasta que el proceso se apaga. Por eso
//! **no** lleva la envoltura JSON de fuera —una respuesta continua no tiene un
//! `data` único—, pero el **contenido de cada evento** sí la reutiliza: cada
//! `data:` lleva el mismo `{ "data": [...] }` que el listado
//! (`GET /api/v1/units`), para que el panel no necesite un analizador distinto.
//!
//! Lo que viaja por el flujo son **instantáneas completas**, no deltas. Cada evento
//! sustituye el dato del cliente, así que un evento perdido no deja un estado
//! divergente: el siguiente lo corrige. El precio —mandar la lista entera cada
//! vez— es despreciable para las quadlets de un usuario.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::Sse;
use axum::response::sse::{Event, KeepAlive};
use futures_util::stream::{self, Stream};
use tokio::sync::{broadcast, watch};

use crate::api::response::ApiResponse;
use crate::api::units::{UnitSummary, from_con_grupos};
use crate::config::Config;
use crate::error::AppError;
use crate::quadlet::groups::load_groups_para;
use crate::state::AppState;
use crate::systemd::UnitStatus;

/// El nombre del evento que viaja por el flujo.
///
/// El panel lo escucha por su nombre (`addEventListener("units", …)`): sin nombre,
/// el navegador lo entregaría como un `message` genérico y el cliente no podría
/// distinguirlo de un latido mal formado.
const EVENTO: &str = "units";

/// Cuánto espera el cliente antes de reconectar si el flujo se corta.
///
/// Tres segundos es lo que el navegador usará entre reintentos tras una caída de
/// red: ni un martilleo contra el servidor ni una espera que deje el panel muerto.
const RETRY: Duration = Duration::from_secs(3);

/// Cada cuánto se manda un latido cuando no hay nada que contar.
///
/// Un proxy con la conexión ociosa la corta —algunos a los pocos segundos—, así que
/// el flujo manda un comentario de vez en cuando para que la conexión no parezca
/// muerta. Quince segundos es el valor por defecto de axum y el que usan la mayoría
/// de los proxies como umbral.
const LATIDO: Duration = Duration::from_secs(15);

/// `GET /api/v1/units/stream` — la instantánea de las quadlets, en vivo.
///
/// El primer evento se construye **aquí**, con el listado que ya existe, y no
/// espera a ningún cambio: un cliente que se conecta ve el panel pintado sin un
/// `GET` aparte. Después, el flujo se alimenta del canal de eventos que llena el
/// fan-out.
///
/// Si el listado inicial falla, la petición contesta el error **antes** de abrir el
/// flujo: un `503` con la envoltura de siempre, no un `text/event-stream` roto.
pub async fn stream(
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::io::Error>> + Send + 'static>, AppError> {
    // La suscripción va **antes** de calcular la instantánea, y el orden no es un
    // detalle: al revés, un cambio que ocurriera entre el listado y la suscripción
    // no llegaría nunca —el canal no guarda historia— y el panel se quedaría con el
    // dato viejo hasta el cambio siguiente. Suscribiéndose primero, ese cambio queda
    // encolado y sale justo después del primer evento.
    let eventos = state.eventos.subscribe();
    let inicial = state.systemd.quadlets().await?;

    Ok(flujo(
        inicial,
        eventos,
        state.apagado.clone(),
        LATIDO,
        state.config.clone(),
    ))
}

/// Compone la respuesta SSE a partir de la instantánea inicial y los canales.
///
/// El intervalo del latido entra por parámetro —y no se lee del módulo— para que la
/// prueba pueda pedir uno de milisegundos y medir el latido sin esperar quince
/// segundos.
fn flujo(
    inicial: Vec<UnitStatus>,
    eventos: broadcast::Receiver<Vec<UnitStatus>>,
    apagado: watch::Receiver<bool>,
    latido: Duration,
    config: Arc<Config>,
) -> Sse<impl Stream<Item = Result<Event, std::io::Error>> + Send + 'static> {
    Sse::new(instantaneas(inicial, eventos, apagado, config))
        .keep_alive(KeepAlive::new().interval(latido).text("latido"))
}

/// El flujo de eventos: primero la instantánea inicial, después una por publicación.
///
/// Termina cuando el proceso se apaga (el canal de cierre lo dice) o cuando el
/// fan-out desaparece. Terminar es lo que impide que el apagado ordenado se quede
/// colgado esperando a una conexión que nunca acabaría.
fn instantaneas(
    inicial: Vec<UnitStatus>,
    eventos: broadcast::Receiver<Vec<UnitStatus>>,
    apagado: watch::Receiver<bool>,
    config: Arc<Config>,
) -> impl Stream<Item = Result<Event, std::io::Error>> + Send + 'static {
    stream::unfold(
        (Some(inicial), eventos, apagado, config),
        |(mut pendiente, mut eventos, mut apagado, config)| async move {
            // La instantánea inicial sale en la primera vuelta, sin esperar a nada:
            // es lo que hace que conectar baste para pintar el panel.
            if let Some(instantanea) = pendiente.take() {
                return Some((
                    evento(instantanea, &config),
                    (pendiente, eventos, apagado, config),
                ));
            }

            loop {
                // El `select!` produce **un** valor y su ámbito se cierra antes de
                // tocar los canales: así el préstamo del `wait_for` no impide
                // devolver los receptores al estado del flujo.
                let siguiente = tokio::select! {
                    biased;

                    // El cierre se mira primero: apagándose no se espera a más
                    // eventos, se termina el flujo.
                    cierre = apagado.wait_for(|apagado| *apagado) => {
                        // Si el disparador se soltó (`Err`), tampoco hay a quién
                        // servir: el flujo acaba igual.
                        let _ = cierre;
                        None
                    }
                    aviso = eventos.recv() => match aviso {
                        Ok(instantanea) => Some(instantanea),
                        // El cliente se quedó atrás y el canal tiró instantáneas
                        // viejas: no hay nada que recuperar, porque cada evento
                        // lleva la lista entera y el siguiente la pone al día.
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        // El fan-out se fue: no habrá más eventos y el flujo acaba.
                        Err(broadcast::error::RecvError::Closed) => None,
                    },
                };

                let instantanea = siguiente?;

                return Some((
                    evento(instantanea, &config),
                    (pendiente, eventos, apagado, config),
                ));
            }
        },
    )
}

/// Traduce una instantánea de dominio a un evento SSE con la envoltura `{data}`.
///
/// Es el **mismo** recorte que hace el listado (`UnitStatus` → `UnitSummary`), así
/// que el `data` del evento y el cuerpo de `GET /api/v1/units` son idénticos por
/// construcción: el panel puede tratarlos igual.
fn evento(instantanea: Vec<UnitStatus>, config: &Config) -> Result<Event, std::io::Error> {
    // Los grupos se recargan **por instantánea**, no al conectar: el `GET` los
    // lee en cada petición y el flujo tiene que ver el mismo cambio en el
    // siguiente evento, sin obligar al cliente a reconectar.
    let grupos = load_groups_para(&instantanea, config);
    let resumen: Vec<UnitSummary> = instantanea
        .into_iter()
        .map(|u| from_con_grupos(u, &grupos))
        .collect();

    // Serializar una lista de cadenas y enumeraciones no puede fallar, pero si
    // fallara no se puede fingir que hay evento: se devuelve el fallo y el flujo
    // termina, que es lo único honesto.
    let cuerpo = serde_json::to_string(&ApiResponse::new(resumen)).map_err(|error| {
        tracing::error!(%error, "no se pudo serializar la instantánea del canal en vivo");
        std::io::Error::other(error)
    })?;

    Ok(Event::default().event(EVENTO).retry(RETRY).data(cuerpo))
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::BodyDataStream;
    use axum::http::{Method, StatusCode};
    use axum::response::IntoResponse;
    use futures_util::StreamExt;
    use tempfile::TempDir;

    use crate::config::Config;

    use crate::systemd::{
        ActiveState, SystemdError, UnitFileState, UnitName, UnitOrigin, UnitStatus,
    };
    use crate::test_support::{
        DobleQueFalla, DobleRetenido, entorno, entorno_con, peticion, peticion_cruda,
    };

    use super::{LATIDO, RETRY, flujo};

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

    /// El JSON del campo `data` de un cuadro SSE.
    fn data_del_cuadro(cuadro: &str) -> serde_json::Value {
        let json = cuadro
            .lines()
            .find_map(|linea| linea.strip_prefix("data: "))
            .unwrap_or_else(|| panic!("el cuadro no traía campo `data`: {cuadro:?}"));

        serde_json::from_str(json).expect("el `data` del evento es JSON")
    }

    /// Una instantánea de un solo elemento con el nombre que se pida, distinta de
    /// la del doble, para medir que el evento en vivo **sustituye** el dato y no
    /// repite el inicial.
    fn instantanea_de(nombre: &str) -> Vec<UnitStatus> {
        vec![UnitStatus {
            name: UnitName::sin_validar(nombre),
            active_state: ActiveState::Inactive,
            sub_state: "dead".to_owned(),
            unit_file_state: UnitFileState::Generated,
            origin: UnitOrigin::desde(
                "/home/lorenzo/.config/containers/systemd/solo-una.container",
            ),
        }]
    }

    /// La instantánea que publica una prueba por el canal de eventos.
    fn instantanea_distinta() -> Vec<UnitStatus> {
        instantanea_de("solo-una.service")
    }

    /// Escenario «Instantánea inicial al conectar»: el primer evento llega sin
    /// esperar a ningún cambio y trae la lista actual de quadlets.
    #[tokio::test]
    async fn el_primer_evento_lleva_la_instantanea_actual() {
        let entorno = entorno().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/units/stream").await;

        assert_eq!(respuesta.status(), StatusCode::OK);
        assert_eq!(
            respuesta
                .headers()
                .get("content-type")
                .and_then(|valor| valor.to_str().ok()),
            Some("text/event-stream"),
            "el stream tiene que anunciarse como text/event-stream"
        );

        let mut cuerpo = respuesta.into_body().into_data_stream();
        let cuadro = leer_cuadro(&mut cuerpo).await;

        assert!(
            cuadro.contains("event: units"),
            "el primer evento tiene que ir nombrado: {cuadro:?}"
        );

        let data = data_del_cuadro(&cuadro);
        let nombres: Vec<&str> = data["data"]
            .as_array()
            .expect("el `data` del evento es un array de unidades")
            .iter()
            .map(|unidad| unidad["name"].as_str().unwrap_or_default())
            .collect();

        assert_eq!(
            nombres,
            ["podpot-demo-db.service", "podpot-demo-web.service"],
            "el primer evento trae la instantánea actual: {cuadro:?}"
        );
    }

    /// Escenario «Empuje al cambiar el estado»: tras publicar una instantánea nueva
    /// en el canal, el flujo emite el siguiente evento con ella.
    #[tokio::test]
    async fn un_evento_nuevo_llega_al_publicar_en_el_canal() {
        let entorno = entorno().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/units/stream").await;
        let mut cuerpo = respuesta.into_body().into_data_stream();

        // El primero es la instantánea inicial; se consume para llegar al vivo.
        let _ = leer_cuadro(&mut cuerpo).await;

        entorno
            .state()
            .eventos
            .send(instantanea_distinta())
            .expect("el flujo está suscrito: el envío tiene receptor");

        let cuadro = leer_cuadro(&mut cuerpo).await;
        let data = data_del_cuadro(&cuadro);

        assert_eq!(
            data["data"][0]["name"], "solo-una.service",
            "el evento en vivo trae la instantánea publicada: {cuadro:?}"
        );
        assert_eq!(
            data["data"].as_array().map(Vec::len),
            Some(1),
            "el evento sustituye la lista entera, no la amplía: {cuadro:?}"
        );
    }

    /// Escenario «Recarga por instantánea»: editar `groups.toml` cambia el
    /// `stack` del siguiente evento sin reconectar el flujo.
    #[tokio::test]
    async fn el_stream_recarga_los_grupos_en_cada_instantanea() {
        let fuente = TempDir::new().expect("directorio fuente");
        let defend = fuente.path().to_str().expect("ruta UTF-8").to_owned();
        let config = Config::from_source(move |clave| match clave {
            "DATABASE_URL" => Some("sqlite::memory:".to_owned()),
            "HOME" => Some("/home/tester".to_owned()),
            "XDG_RUNTIME_DIR" => Some("/run/user/1000".to_owned()),
            "PODPOT_QUADLET_DIR" => Some(defend.clone()),
            _ => None,
        })
        .expect("la configuración de prueba es válida");

        let origen = fuente
            .path()
            .join("web.container")
            .to_string_lossy()
            .into_owned();
        let escribir = |valor: &str| {
            let mut fichero = std::fs::File::create(fuente.path().join("groups.toml"))
                .expect("crear groups.toml");
            writeln!(fichero, "[mappings]\n\"web.container\" = \"{valor}\"")
                .expect("escribir groups.toml");
        };
        let publicar = || {
            vec![UnitStatus {
                name: UnitName::sin_validar("web.service"),
                active_state: ActiveState::Active,
                sub_state: "running".to_owned(),
                unit_file_state: UnitFileState::Generated,
                origin: UnitOrigin::desde(&origen),
            }]
        };

        escribir("primero");
        let entorno = entorno().await;
        let state = entorno.state().clone();
        let respuesta = flujo(
            Vec::new(),
            state.eventos.subscribe(),
            state.apagado.clone(),
            Duration::from_millis(1),
            Arc::new(config),
        )
        .into_response();
        let mut cuerpo = respuesta.into_body().into_data_stream();

        // La instantánea inicial está vacía: se consume para llegar al vivo.
        let _ = leer_cuadro(&mut cuerpo).await;

        state.eventos.send(publicar()).expect("hay un suscriptor");
        let primero = data_del_cuadro(&leer_cuadro(&mut cuerpo).await);
        assert_eq!(primero["data"][0]["stack"], "primero");

        // El fichero cambia con el flujo ya abierto.
        escribir("segundo");
        state.eventos.send(publicar()).expect("hay un suscriptor");
        let segundo = data_del_cuadro(&leer_cuadro(&mut cuerpo).await);
        assert_eq!(
            segundo["data"][0]["stack"], "segundo",
            "el evento tiene que ver el groups.toml actual sin reconectar"
        );
    }

    /// La suscripción al canal va **antes** de calcular la instantánea inicial.
    ///
    /// Al revés hay una carrera de verdad: un cambio que ocurra entre el listado y
    /// la suscripción no llega nunca —el canal no guarda historia para quien llega
    /// tarde—, así que el panel se quedaría con el dato viejo hasta el cambio
    /// siguiente. Con el listado retenido, la prueba publica justo en ese hueco: si
    /// el manejador no estuviera suscrito, el envío no tendría receptor y la
    /// instantánea publicada se perdería sin que nada lo dijera.
    #[tokio::test]
    async fn un_cambio_entre_el_listado_y_la_suscripcion_no_se_pierde() {
        let doble = Arc::new(DobleRetenido::con(instantanea_de("retenida.service")));
        let dentro = doble.dentro();
        let seguir = doble.seguir();

        let entorno = entorno_con(doble).await;
        let app = entorno.app();

        let mut peticion = Box::pin(peticion_cruda(&app, Method::GET, "/api/v1/units/stream"));

        // El manejador ya está dentro del listado —y, por tanto, ya pasó por la
        // suscripción—: ahora se publica el cambio que tiene que llegar.
        tokio::select! {
            () = dentro.notified() => {}
            _ = &mut peticion => panic!("el listado no se retuvo: la prueba no mide nada"),
        }

        entorno
            .state()
            .eventos
            .send(instantanea_distinta())
            .expect("el manejador tiene que estar suscrito antes de listar");

        seguir.notify_one();

        let respuesta = peticion.await;
        let mut cuerpo = respuesta.into_body().into_data_stream();

        let primero = leer_cuadro(&mut cuerpo).await;
        assert_eq!(
            data_del_cuadro(&primero)["data"][0]["name"],
            "retenida.service",
            "el primer evento tiene que ser la instantánea del listado: {primero:?}"
        );

        let segundo = leer_cuadro(&mut cuerpo).await;
        assert_eq!(
            data_del_cuadro(&segundo)["data"][0]["name"],
            "solo-una.service",
            "el cambio publicado durante el listado no puede perderse: {segundo:?}"
        );
    }

    /// Escenario «Latido y reconexión»: el flujo indica un `retry` y, sin cambios,
    /// sigue mandando latidos.
    ///
    /// El latido se mide con un intervalo de milisegundos para no esperar los
    /// quince segundos de producción: es la razón por la que el intervalo entra por
    /// parámetro.
    #[tokio::test]
    async fn el_flujo_indica_retry_y_manda_latidos() {
        let entorno = entorno().await;
        let state = entorno.state().clone();

        let respuesta = flujo(
            Vec::new(),
            state.eventos.subscribe(),
            state.apagado.clone(),
            Duration::from_millis(1),
            state.config.clone(),
        )
        .into_response();

        let mut cuerpo = respuesta.into_body().into_data_stream();

        let primero = leer_cuadro(&mut cuerpo).await;
        assert!(
            primero.contains(&format!("retry: {}", RETRY.as_millis())),
            "el flujo tiene que indicar el retry con el que reconectar: {primero:?}"
        );

        let latido = leer_cuadro(&mut cuerpo).await;
        assert!(
            latido.contains(": latido"),
            "sin cambios, el flujo tiene que mandar un latido: {latido:?}"
        );
    }

    /// Escenario «El stream no usa la envoltura de documento único»: el `data` del
    /// evento es el mismo `{data: …}` que el listado, para que el panel reutilice
    /// el mismo tipo.
    #[tokio::test]
    async fn el_data_del_evento_es_el_mismo_que_el_del_listado() {
        let entorno = entorno().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/units/stream").await;
        let mut cuerpo = respuesta.into_body().into_data_stream();
        let cuadro = leer_cuadro(&mut cuerpo).await;
        let del_evento = data_del_cuadro(&cuadro);

        let (status, del_listado) = peticion(&entorno.app(), Method::GET, "/api/v1/units").await;
        assert_eq!(status, StatusCode::OK);

        assert_eq!(
            del_evento, del_listado,
            "el `data` del evento y el cuerpo del listado tienen que ser idénticos"
        );
    }

    /// Escenario «Apagado con un cliente conectado»: al dispararse el cierre, el
    /// flujo termina en vez de quedarse colgado y bloquear el apagado ordenado.
    #[tokio::test]
    async fn el_cierre_termina_el_flujo() {
        let entorno = entorno().await;

        let respuesta = peticion_cruda(&entorno.app(), Method::GET, "/api/v1/units/stream").await;
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

    /// Escenario «Método no permitido en el stream»: `GET` es el único admitido, y
    /// cualquier otro método contesta `405` con la envoltura habitual.
    #[tokio::test]
    async fn un_metodo_distinto_de_get_da_405_con_la_envoltura() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::POST, "/api/v1/units/stream").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// Un gestor que no contesta no abre un flujo roto: la petición falla **antes**
    /// del `200`, con el `503` de siempre.
    #[tokio::test]
    async fn un_gestor_que_no_contesta_da_503_antes_de_abrir_el_flujo() {
        let entorno = entorno_con(std::sync::Arc::new(DobleQueFalla::con(|| {
            SystemdError::Transport {
                causa: Box::new(std::io::Error::other("el bus se cayó")),
            }
        })))
        .await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/units/stream").await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(cuerpo["error"]["code"], "systemd_unavailable");
    }

    /// El latido de producción es un valor sensato —ni un martilleo ni una conexión
    /// que un proxy pueda dar por muerta—, y el `retry` también.
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
    }
}
