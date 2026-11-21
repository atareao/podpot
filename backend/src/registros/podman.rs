//! El cliente de la API de Podman, por su socket unix.
//!
//! Habla con la API compat de Podman —la que imita la de Docker— a través del
//! socket del usuario, sin invocar ningún binario: es la vía que eligió el
//! proyecto y la única que puede **seguir** el stream de un contenedor. El
//! cliente se escribe a mano sobre `hyper` porque sólo hacen falta tres llamadas
//! —listar, inspeccionar y seguir— y arrastrar un cliente entero de Docker no
//! estaría justificado.
//!
//! El **mapeo unidad → contenedor** va por la etiqueta que systemd pone a los
//! contenedores que gestiona (`PODMAN_SYSTEMD_UNIT=<unidad>`), no por el nombre
//! del fichero de la quadlet: la etiqueta es la fuente de verdad y así no hay que
//! leer `ContainerName=`.
//!
//! Nada de esto se publica a la capa API: lo que sale de aquí es el puerto
//! [`FuenteDeRegistros`], un canal de líneas ya demultiplexadas.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use hyper::body::{Bytes, Incoming};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixConnector, Uri};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use super::{CAPACIDAD_DE_REGISTROS, Demux, FuenteDeRegistros, LineaDeRegistro, RegistrosError};
use crate::systemd::UnitName;

/// La versión de la API con la que se habla.
///
/// Podman la mantiene estable y la anuncia en `/version`; fijarla evita depender
/// de que el servidor acepte la forma sin versión. `v1.40` es la que responde en
/// la máquina de referencia (podman 6.1.4) y la que usa la API compat de Docker.
const VERSION: &str = "/v1.40";

/// El nombre del cliente sobre el socket.
///
/// El cuerpo de una petición de lectura es siempre vacío; el tipo tiene que
/// nombrarse porque `hyper` lo lleva en el suyo.
type ClienteUnix = Client<UnixConnector, Full<Bytes>>;

/// El plazo de una petición suelta a la API de Podman.
///
/// Sin plazo, un Podman que acepta la conexión y no contesta deja la petición
/// colgada para siempre —y con ella el manejador del canal SSE y el apagado
/// ordenado—. Cinco segundos sobran para listar o inspeccionar en una máquina
/// sana, y son pocos para que una petición perdida se note.
const PLAZO_DE_PETICION: Duration = Duration::from_secs(5);

/// La fuente de registros **real**: la que habla con Podman.
pub struct PodmanRegistros {
    /// La ruta del socket de la API.
    socket: PathBuf,
    /// Cuánto se espera una petición antes de darla por perdida.
    plazo: Duration,
    /// El cliente HTTP sobre el socket, con su pool de conexiones.
    ///
    /// Se construye una vez y se reutiliza: el cliente guarda las conexiones y
    /// volver a construirlo por petición abriría una conexión por línea de log.
    cliente: ClienteUnix,
}

impl fmt::Debug for PodmanRegistros {
    /// Sólo la ruta del socket: el cliente de `hyper` no tiene nada que contar
    /// —y su `Debug` interno arrastraría el del conector—.
    fn fmt(&self, formateador: &mut fmt::Formatter<'_>) -> fmt::Result {
        formateador
            .debug_struct("PodmanRegistros")
            .field("socket", &self.socket)
            .finish_non_exhaustive()
    }
}

impl PodmanRegistros {
    /// Una fuente que lee del socket indicado.
    pub fn new(socket: PathBuf) -> Self {
        Self::construir(socket, PLAZO_DE_PETICION)
    }

    /// El constructor con el plazo explícito.
    ///
    /// Existe para las pruebas: miden el vencimiento con un plazo de milisegundos
    /// en vez de esperar los cinco segundos de producción.
    #[cfg(test)]
    fn con_plazo(socket: PathBuf, plazo: Duration) -> Self {
        Self::construir(socket, plazo)
    }

    /// El constructor de verdad: el socket, el plazo y el cliente.
    fn construir(socket: PathBuf, plazo: Duration) -> Self {
        Self {
            socket,
            plazo,
            cliente: Client::builder(TokioExecutor::new()).build(UnixConnector),
        }
    }

    /// La ruta del socket del que lee.
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Lanza una petición `GET` y devuelve el cuerpo **sin leer**.
    ///
    /// Devuelve el cuerpo y no los bytes porque el de los logs es un stream que
    /// hay que ir consumiendo: leerlo entero aquí sería esperar a que el
    /// contenedor termine.
    ///
    /// La petición va con plazo: un Podman que acepta la conexión y no contesta no
    /// puede colgar a quien pregunta —el manejador del canal SSE, que espera a
    /// esto antes de anunciar nada—.
    async fn obtener(&self, ruta_y_consulta: &str) -> Result<Incoming, RegistrosError> {
        let uri = Uri::new(&self.socket, ruta_y_consulta);

        let peticion = Request::get(uri)
            .body(Full::new(Bytes::new()))
            .map_err(|causa| self.fallo_de_socket(causa))?;

        let respuesta = tokio::time::timeout(self.plazo, self.cliente.request(peticion))
            .await
            .map_err(|_| self.fallo_de_plazo(ruta_y_consulta))?
            .map_err(|causa| self.fallo_de_socket(causa))?;

        let estado = respuesta.status();
        if !estado.is_success() {
            debug!(
                socket = %self.socket.display(),
                ruta = ruta_y_consulta,
                estado = %estado,
                "la API de Podman contestó con un estado que no es de éxito"
            );

            return Err(RegistrosError::Podman {
                causa: Box::new(std::io::Error::other(format!(
                    "la API de Podman contestó `{estado}`"
                ))),
            });
        }

        Ok(respuesta.into_body())
    }

    /// Lanza una petición `GET` y lee el cuerpo entero.
    ///
    /// Es para las respuestas cortas —el listado y la inspección—, no para los
    /// logs. La lectura también lleva plazo: un Podman que manda las cabeceras y
    /// luego se atasca dejaría la petición colgada igual que si no contestase.
    async fn obtener_entero(&self, ruta_y_consulta: &str) -> Result<Bytes, RegistrosError> {
        let cuerpo = self.obtener(ruta_y_consulta).await?;

        tokio::time::timeout(self.plazo, cuerpo.collect())
            .await
            .map_err(|_| self.fallo_de_plazo(ruta_y_consulta))?
            .map(|recolectado| recolectado.to_bytes())
            .map_err(|causa| RegistrosError::Podman {
                causa: Box::new(causa),
            })
    }

    /// El `id` del contenedor de una unidad, si lo hay.
    ///
    /// La búsqueda va por la etiqueta de systemd y con `all=true`: un contenedor
    /// parado conserva sus logs, y negarlos porque no está corriendo sería negar
    /// justo los que explican por qué se paró.
    async fn contenedor_de(&self, unidad: &UnitName) -> Result<String, RegistrosError> {
        let etiqueta = format!("PODMAN_SYSTEMD_UNIT={}", unidad.as_str());
        let filtro = serde_json::json!({ "label": [etiqueta] });

        // El filtro va como JSON **codificado**: sin codificar, las llaves y las
        // comillas romperían el URI —`Uri::new` lo construye parseándolo—. El
        // JSON se guarda en su propia variable porque `encode` lo presta.
        let filtro = filtro.to_string();
        let consulta = urlencoding::encode(&filtro);
        let ruta = format!("{VERSION}/containers/json?all=true&filters={consulta}");

        let cuerpo = self.obtener_entero(&ruta).await?;

        let resumenes: Vec<ResumenDeContenedor> =
            serde_json::from_slice(&cuerpo).map_err(|causa| RegistrosError::Podman {
                causa: Box::new(causa),
            })?;

        resumenes
            .into_iter()
            .next()
            .map(|resumen| resumen.id)
            .ok_or_else(|| RegistrosError::SinContenedor {
                unidad: unidad.as_str().to_owned(),
            })
    }

    /// Si el contenedor usa TTY, que es lo que decide el formato del stream.
    async fn usa_tty(&self, id: &str) -> Result<bool, RegistrosError> {
        let ruta = format!("{VERSION}/containers/{id}/json");

        let cuerpo = self.obtener_entero(&ruta).await?;

        let inspeccion: Inspeccion =
            serde_json::from_slice(&cuerpo).map_err(|causa| RegistrosError::Podman {
                causa: Box::new(causa),
            })?;

        Ok(inspeccion.config.tty)
    }

    /// Un fallo de transporte, con el socket que se intentó.
    fn fallo_de_socket(
        &self,
        causa: impl std::error::Error + Send + Sync + 'static,
    ) -> RegistrosError {
        RegistrosError::Socket {
            socket: self.socket.display().to_string(),
            causa: Box::new(causa),
        }
    }

    /// Un vencimiento del plazo: se habló con el socket, pero no contestó a tiempo.
    ///
    /// Se cuenta como fallo de transporte —y no como respuesta inservible— porque
    /// el desenlace es el mismo que el de un Podman caído: desde fuera, un Podman
    /// que no contesta es un Podman que no está. La causa lleva la ruta y el plazo,
    /// que son del operador: no se publican al cliente.
    fn fallo_de_plazo(&self, ruta_y_consulta: &str) -> RegistrosError {
        RegistrosError::Socket {
            socket: self.socket.display().to_string(),
            causa: Box::new(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "la API de Podman no contestó a `{ruta_y_consulta}` en {:?}",
                    self.plazo
                ),
            )),
        }
    }
}

/// Un contenedor del listado, con lo único que se necesita de él.
#[derive(Debug, serde::Deserialize)]
struct ResumenDeContenedor {
    /// El identificador con el que se piden los logs.
    #[serde(rename = "Id")]
    id: String,
}

/// La inspección de un contenedor, recortada a lo que se usa.
#[derive(Debug, serde::Deserialize)]
struct Inspeccion {
    /// La configuración del contenedor.
    #[serde(rename = "Config")]
    config: Configuracion,
}

/// La configuración del contenedor.
#[derive(Debug, serde::Deserialize)]
struct Configuracion {
    /// Si el contenedor usa TTY: con TTY el stream es texto crudo, sin marcos.
    #[serde(rename = "Tty")]
    tty: bool,
}

#[async_trait]
impl FuenteDeRegistros for PodmanRegistros {
    async fn seguir(
        &self,
        unidad: &UnitName,
        cola: usize,
    ) -> Result<mpsc::Receiver<LineaDeRegistro>, RegistrosError> {
        // Los tres pasos que pueden fallar van **antes** de devolver el canal:
        // así quien abre el flujo SSE puede contestar el error con su estado
        // propio —un `404` de unidad sin contenedor, un `503` de Podman caído—
        // en vez de haber anunciado ya `text/event-stream`.
        let id = self.contenedor_de(unidad).await?;
        let tty = self.usa_tty(&id).await?;

        let ruta = format!(
            "{VERSION}/containers/{id}/logs?follow=true&stdout=true&stderr=true&tail={cola}"
        );
        let cuerpo = self.obtener(&ruta).await?;

        debug!(
            unidad = unidad.as_str(),
            contenedor = %id,
            tty,
            cola,
            "siguiendo los registros del contenedor"
        );

        let (emisor, receptor) = mpsc::channel(CAPACIDAD_DE_REGISTROS);

        // El bombeo vive en su propia tarea: el canal es la frontera, y cuando
        // nadie lee —el cliente SSE se fue— el `send` falla y la tarea termina,
        // cerrando también la conexión con Podman.
        tokio::spawn(bombear(cuerpo, tty, emisor));

        Ok(receptor)
    }
}

/// Lee el cuerpo del stream y lo convierte en líneas por el canal.
///
/// Es lo que corre en la tarea de fondo: consume el cuerpo **según llega**, lo
/// pasa por el demultiplexador y entrega cada línea cerrada. Termina cuando el
/// cuerpo se acaba —el contenedor dejó de escribir o murió—, cuando el canal se
/// cierra, o cuando el cuerpo falla; en los tres casos cierra la última línea a
/// medias, si la hay, para que no se quede invisible.
///
/// El corte por el cliente se mira en **cada vuelta**, no al enviar: con
/// `follow=true` sobre un contenedor ocioso no llega ningún marco, y una tarea que
/// sólo viera el `send` fallido seguiría aparcada en el cuerpo —con la conexión
/// abierta— hasta la próxima línea, que puede no llegar nunca.
async fn bombear(cuerpo: Incoming, tty: bool, emisor: mpsc::Sender<LineaDeRegistro>) {
    let mut demux = Demux::new(tty);
    let mut cuerpo = cuerpo;

    loop {
        let cuadro = tokio::select! {
            biased;

            // El cliente se fue: se corta ya, sin esperar a más marcos.
            () = emisor.closed() => return,
            cuadro = cuerpo.frame() => cuadro,
        };

        let Some(cuadro) = cuadro else {
            break;
        };

        let Ok(cuadro) = cuadro else {
            // Un cuerpo que falla a medias no se puede reintentar aquí: lo que
            // llegó ya se entregó y lo que falta no volverá. El canal se cierra y
            // el flujo SSE emite su evento terminal.
            break;
        };

        let Some(datos) = cuadro.data_ref() else {
            continue;
        };

        let lineas = match demux.alimentar(datos) {
            Ok(lineas) => lineas,
            Err(causa) => {
                // Un stream con un marco imposible no se puede resincronizar: se
                // corta, se registra —la causa es del operador— y el flujo emite su
                // evento terminal.
                warn!(error = %causa, "el stream de logs del contenedor está roto");
                break;
            }
        };

        for linea in lineas {
            if emisor.send(linea).await.is_err() {
                return;
            }
        }
    }

    if let Some(ultima) = demux.terminar() {
        let _ = emisor.send(ultima).await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;
    use tokio::sync::mpsc;

    use crate::systemd::UnitName;

    use super::super::{
        CAPACIDAD_DE_REGISTROS, COLA_INICIAL, FuenteDeRegistros, LineaDeRegistro, Origen,
        RegistrosError,
    };
    use super::{PodmanRegistros, bombear};

    /// El plazo de las esperas de estas pruebas: de sobra para un socket efímero,
    /// corto para que una prueba rota no cuelgue la suite.
    const ESPERA: Duration = Duration::from_secs(5);

    /// El plazo con el que se mide el vencimiento: milisegundos, no los cinco
    /// segundos de producción.
    const PLAZO_DE_PRUEBA: Duration = Duration::from_millis(50);

    /// Un servidor HTTP/1.1 mínimo sobre un socket unix efímero.
    ///
    /// Es el arnés de estas pruebas: un Podman de mentira que contesta lo que se
    /// le diga. Se escribe a mano porque la feature `server` de `hyper` no está
    /// encendida —el servidor de verdad lo pone axum— y aquí sólo hace falta
    /// atender una petición y cerrar.
    struct Servidor {
        /// El directorio temporal que sostiene el socket mientras vive.
        _dir: tempfile::TempDir,
        /// La ruta del socket, para dársela al cliente.
        ruta: std::path::PathBuf,
        /// Las rutas que se han pedido, para poder comprobarlas.
        pedidas: Arc<Mutex<Vec<String>>>,
    }

    impl Servidor {
        /// Crea el socket efímero y deja la escucha lista.
        fn escuchar() -> (tempfile::TempDir, std::path::PathBuf, UnixListener) {
            let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
            let ruta = dir.path().join("podman.sock");
            let escucha = UnixListener::bind(&ruta).expect("el socket temporal se tiene que crear");

            (dir, ruta, escucha)
        }

        /// Arranca un servidor que contesta con lo que devuelva `responder`.
        ///
        /// `responder` recibe la ruta **con su consulta** y devuelve la respuesta
        /// cruda —línea de estado, cabeceras y cuerpo—.
        async fn arrancar<F>(responder: F) -> Self
        where
            F: Fn(&str) -> String + Send + Sync + 'static,
        {
            Self::arrancar_con(responder, false).await
        }

        /// Arranca un servidor que contesta y **sostiene** la conexión abierta.
        ///
        /// Es el contenedor ocioso con `follow=true`: la respuesta se queda abierta
        /// sin que lleguen más marcos. La conexión se sostiene hasta que el cliente
        /// se va —el `read` devuelve cero—, que es lo que permite medir que el
        /// cliente la cortó.
        async fn arrancar_sosteniendo<F>(responder: F) -> Self
        where
            F: Fn(&str) -> String + Send + Sync + 'static,
        {
            Self::arrancar_con(responder, true).await
        }

        /// Arranca un servidor que lee la petición y **no contesta**.
        ///
        /// Es el caso que hay que medir: un Podman que acepta la conexión y no
        /// responde. Sin plazo, la petición se quedaría colgada para siempre.
        async fn mudo() -> Self {
            Self::arrancar_con(|_| String::new(), true).await
        }

        /// El arnés de las tres: escucha, atiende cada conexión y decide si la
        /// cierra tras responder o la sostiene.
        async fn arrancar_con<F>(responder: F, sostener: bool) -> Self
        where
            F: Fn(&str) -> String + Send + Sync + 'static,
        {
            let (dir, ruta, escucha) = Self::escuchar();

            let pedidas = Arc::new(Mutex::new(Vec::new()));
            let responder = Arc::new(responder);
            let registro = Arc::clone(&pedidas);

            tokio::spawn(async move {
                loop {
                    let Ok((mut flujo, _)) = escucha.accept().await else {
                        break;
                    };

                    let responder = Arc::clone(&responder);
                    let registro = Arc::clone(&registro);

                    tokio::spawn(async move {
                        let Some(ruta_peticion) = leer_peticion(&mut flujo).await else {
                            return;
                        };

                        registro
                            .lock()
                            .unwrap_or_else(|envenenado| envenenado.into_inner())
                            .push(ruta_peticion.clone());

                        let respuesta = responder(&ruta_peticion);
                        let _ = flujo.write_all(respuesta.as_bytes()).await;
                        let _ = flujo.flush().await;

                        if !sostener {
                            let _ = flujo.shutdown().await;
                            return;
                        }

                        // Se sostiene la conexión hasta que el cliente la suelte:
                        // el `read` devuelve cero cuando el otro extremo se va.
                        let mut trozo = [0u8; 64];
                        while let Ok(leidos) = flujo.read(&mut trozo).await {
                            if leidos == 0 {
                                break;
                            }
                        }
                    });
                }
            });

            Self {
                _dir: dir,
                ruta,
                pedidas,
            }
        }

        /// La ruta del socket.
        fn ruta(&self) -> &std::path::Path {
            &self.ruta
        }

        /// Las rutas que se han pedido, en orden.
        fn pedidas(&self) -> Vec<String> {
            self.pedidas
                .lock()
                .unwrap_or_else(|envenenado| envenenado.into_inner())
                .clone()
        }
    }

    /// Lee una petición hasta el final de las cabeceras y devuelve su objetivo.
    async fn leer_peticion(flujo: &mut tokio::net::UnixStream) -> Option<String> {
        let mut recibido = Vec::new();
        let mut trozo = [0u8; 1024];

        loop {
            let leidos = flujo.read(&mut trozo).await.ok()?;
            if leidos == 0 {
                return None;
            }

            recibido.extend_from_slice(&trozo[..leidos]);

            if let Some(fin) = buscar(&recibido, b"\r\n\r\n") {
                let cabeceras = String::from_utf8_lossy(&recibido[..fin]);
                let primera = cabeceras.lines().next()?;
                let objetivo = primera.split_whitespace().nth(1)?;

                return Some(objetivo.to_owned());
            }
        }
    }

    /// Busca `aguja` dentro de `pajar` y devuelve su posición.
    fn buscar(pajar: &[u8], aguja: &[u8]) -> Option<usize> {
        pajar
            .windows(aguja.len())
            .position(|ventana| ventana == aguja)
    }

    /// Una respuesta HTTP/1.1 completa, con longitud y cierre de conexión.
    fn respuesta(estado: u16, motivo: &str, cuerpo: &[u8]) -> String {
        let cabeceras = format!(
            "HTTP/1.1 {estado} {motivo}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            cuerpo.len()
        );

        let mut respuesta = cabeceras.into_bytes();
        respuesta.extend_from_slice(cuerpo);

        String::from_utf8_lossy(&respuesta).into_owned()
    }

    /// Un marco multiplexado: origen, relleno, longitud y carga.
    fn marco(origen: u8, carga: &str) -> Vec<u8> {
        let mut bytes = vec![origen, 0, 0, 0];
        bytes.extend_from_slice(&(carga.len() as u32).to_be_bytes());
        bytes.extend_from_slice(carga.as_bytes());

        bytes
    }

    /// Las cabeceras de una respuesta cuyo cuerpo **no llega nunca**.
    ///
    /// Anuncia una longitud y no manda nada: es el contenedor ocioso en modo
    /// seguimiento, con la respuesta abierta y sin marcos.
    fn cabeceras_de_cuerpo_abierto() -> String {
        "HTTP/1.1 200 OK\r\nContent-Length: 4096\r\nConnection: keep-alive\r\n\r\n".to_owned()
    }

    /// Espera a que el canal tenga al menos `cuantas` líneas, o se vence el plazo.
    ///
    /// Sondea en vez de esperar un aviso porque lo que se mide es justo que **no**
    /// hay aviso: el productor está aparcado. Y sondea con un sueño, no con un
    /// `yield_now`, para dejar claro que el runtime sigue vivo: si el productor
    /// bloquease el hilo, este bucle no avanzaría.
    async fn esperar_a_que_llene(
        receptor: &mpsc::Receiver<LineaDeRegistro>,
        cuantas: usize,
    ) -> bool {
        tokio::time::timeout(ESPERA, async {
            while receptor.len() < cuantas {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .is_ok()
    }

    /// Un servidor que contesta el caso feliz: un contenedor sin TTY con dos
    /// líneas de logs.
    async fn servidor_feliz(tty: bool, cuerpo_logs: Vec<u8>) -> Servidor {
        Servidor::arrancar(move |ruta| {
            if ruta.starts_with("/v1.40/containers/json") {
                respuesta(200, "OK", br#"[{"Id":"abc123","Names":["/x"]}]"#)
            } else if ruta == "/v1.40/containers/abc123/json" {
                respuesta(
                    200,
                    "OK",
                    format!(r#"{{"Id":"abc123","Config":{{"Tty":{tty}}}}}"#).as_bytes(),
                )
            } else if ruta.starts_with("/v1.40/containers/abc123/logs") {
                respuesta(200, "OK", &cuerpo_logs)
            } else {
                respuesta(404, "Not Found", b"{}")
            }
        })
        .await
    }

    /// Escenario «Contenedor localizado por etiqueta»: el contenedor se busca por
    /// la etiqueta de systemd y su stream se demultiplexa en líneas.
    #[tokio::test]
    async fn un_contenedor_localizado_por_etiqueta_da_sus_lineas() {
        let servidor = servidor_feliz(
            false,
            [
                marco(1, "arrancando\n"),
                marco(2, "aviso\n"),
                marco(1, "listo\n"),
            ]
            .concat(),
        )
        .await;

        let fuente = PodmanRegistros::new(servidor.ruta().to_path_buf());
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        let mut receptor = fuente
            .seguir(&unidad, COLA_INICIAL)
            .await
            .expect("el contenedor existe y sus logs se pueden seguir");

        assert_eq!(
            receptor.recv().await,
            Some(LineaDeRegistro::nueva("arrancando", Origen::Stdout))
        );
        assert_eq!(
            receptor.recv().await,
            Some(LineaDeRegistro::nueva("aviso", Origen::Stderr))
        );
        assert_eq!(
            receptor.recv().await,
            Some(LineaDeRegistro::nueva("listo", Origen::Stdout))
        );

        // Al terminar el cuerpo, el canal se cierra: es el fin del stream.
        assert_eq!(receptor.recv().await, None);

        // Y las peticiones fueron las tres esperadas, con la etiqueta de systemd
        // y la cola pedida.
        let pedidas = servidor.pedidas();

        assert!(
            pedidas[0].contains("PODMAN_SYSTEMD_UNIT%3Dcentinela-db.service"),
            "el listado tiene que filtrar por la etiqueta de systemd: {}",
            pedidas[0]
        );
        assert_eq!(pedidas[1], "/v1.40/containers/abc123/json");
        assert!(
            pedidas[2].contains(&format!("tail={COLA_INICIAL}")),
            "la cola inicial tiene que viajar en la petición de logs: {}",
            pedidas[2]
        );
        assert!(
            pedidas[2].contains("follow=true"),
            "el stream tiene que pedirse en modo seguimiento: {}",
            pedidas[2]
        );
    }

    /// Escenario «Contenedor con TTY»: el stream es texto crudo y no se
    /// interpretan marcos.
    #[tokio::test]
    async fn un_contenedor_con_tty_lee_el_texto_crudo() {
        let servidor = servidor_feliz(true, b"2\0\0\0\0\0\0\0hola\n".to_vec()).await;

        let fuente = PodmanRegistros::new(servidor.ruta().to_path_buf());
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        let mut receptor = fuente
            .seguir(&unidad, COLA_INICIAL)
            .await
            .expect("el contenedor existe");

        assert_eq!(
            receptor.recv().await,
            Some(LineaDeRegistro::nueva(
                "2\0\0\0\0\0\0\0hola",
                Origen::Stdout
            )),
            "con TTY no hay marcos que interpretar: el stream es texto"
        );
    }

    /// Escenario «Unidad sin contenedor»: un listado vacío es un error explícito,
    /// y no se llega a pedir ningún stream.
    #[tokio::test]
    async fn un_listado_vacio_es_una_unidad_sin_contenedor() {
        let servidor = Servidor::arrancar(|ruta| {
            if ruta.starts_with("/v1.40/containers/json") {
                respuesta(200, "OK", b"[]")
            } else {
                respuesta(404, "Not Found", b"{}")
            }
        })
        .await;

        let fuente = PodmanRegistros::new(servidor.ruta().to_path_buf());
        let unidad = UnitName::parse("datos-volume.service").expect("nombre válido");

        let error = fuente
            .seguir(&unidad, COLA_INICIAL)
            .await
            .expect_err("sin contenedor no hay registros que leer");

        assert!(
            matches!(&error, RegistrosError::SinContenedor { unidad } if unidad == "datos-volume.service"),
            "se esperaba «sin contenedor» nombrando la unidad: {error}"
        );
        assert_eq!(
            servidor.pedidas().len(),
            1,
            "sin contenedor no se pide la inspección ni los logs"
        );
    }

    /// Un estado que no es de éxito es un fallo de la API, no una unidad sin
    /// contenedor: el desenlace no es el mismo y la traducción a HTTP tampoco.
    #[tokio::test]
    async fn un_estado_que_no_es_de_exito_es_un_fallo_de_la_api() {
        let servidor = Servidor::arrancar(|_| respuesta(500, "Internal Server Error", b"{}")).await;

        let fuente = PodmanRegistros::new(servidor.ruta().to_path_buf());
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        let error = fuente
            .seguir(&unidad, COLA_INICIAL)
            .await
            .expect_err("un 500 de la API no puede dar registros");

        assert!(
            matches!(error, RegistrosError::Podman { .. }),
            "se esperaba un fallo de la API: {error}"
        );
    }

    /// Sin socket no se puede hablar: es un fallo de transporte, y el error lleva
    /// la ruta que se intentó para que el registro diga qué arreglar.
    #[tokio::test]
    async fn sin_socket_el_fallo_es_de_transporte() {
        let fuente = PodmanRegistros::new("/no/existe/podman.sock".into());
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        let error = fuente
            .seguir(&unidad, COLA_INICIAL)
            .await
            .expect_err("sin socket no hay con quién hablar");

        assert!(
            matches!(&error, RegistrosError::Socket { socket, .. } if socket == "/no/existe/podman.sock"),
            "se esperaba un fallo de transporte nombrando el socket: {error}"
        );
    }

    /// Un Podman que acepta la conexión y **no contesta** no puede colgar la
    /// petición: el plazo la corta y el fallo es de transporte, como el de un
    /// Podman caído.
    #[tokio::test]
    async fn una_api_que_acepta_y_no_contesta_vence_el_plazo() {
        let servidor = Servidor::mudo().await;
        let fuente = PodmanRegistros::con_plazo(servidor.ruta().to_path_buf(), PLAZO_DE_PRUEBA);
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        let error = tokio::time::timeout(ESPERA, fuente.seguir(&unidad, COLA_INICIAL))
            .await
            .expect("la fuente tiene que cortar por plazo antes de la espera de la prueba")
            .expect_err("una API que no contesta no puede dar registros");

        assert!(
            matches!(&error, RegistrosError::Socket { socket, .. } if socket == &servidor.ruta().display().to_string()),
            "el vencimiento es un fallo de transporte: {error}"
        );
        assert!(
            error.to_string().contains("no contestó"),
            "el error tiene que decir que se venció el plazo: {error}"
        );
    }

    /// Escenario «El cliente se va»: al soltar el receptor, el bombeo termina
    /// **en cuanto se suelta**, sin esperar a un marco que puede no llegar.
    ///
    /// Es el caso del contenedor ocioso: con `follow=true` no llega ningún marco, y
    /// una tarea que sólo mirase el `send` fallido seguiría aparcada en el cuerpo
    /// —con la conexión con Podman abierta— hasta la próxima línea.
    #[tokio::test]
    async fn soltar_el_receptor_termina_el_bombeo_sin_esperar_a_mas_marcos() {
        let servidor = Servidor::arrancar_sosteniendo(|_| cabeceras_de_cuerpo_abierto()).await;
        let fuente = PodmanRegistros::new(servidor.ruta().to_path_buf());

        let cuerpo = fuente
            .obtener("/v1.40/containers/abc123/logs?follow=true")
            .await
            .expect("las cabeceras de la respuesta llegan");

        let (emisor, receptor) = mpsc::channel(CAPACIDAD_DE_REGISTROS);
        let tarea = tokio::spawn(bombear(cuerpo, false, emisor));

        // El cliente se va: nadie va a leer del canal.
        drop(receptor);

        let fin = tokio::time::timeout(ESPERA, tarea)
            .await
            .expect("el bombeo tiene que terminar en cuanto se suelta el receptor");

        assert!(fin.is_ok(), "la tarea de bombeo no puede fallar: {fin:?}");
    }

    /// Escenario «Contrapresión»: un consumidor lento **aparca** al productor en
    /// vez de perder líneas, y el runtime sigue corriendo mientras espera.
    #[tokio::test]
    async fn un_consumidor_lento_aparca_al_productor_sin_perder_lineas() {
        let total = CAPACIDAD_DE_REGISTROS + 5;

        let cuerpo_logs: Vec<u8> = (0..total)
            .map(|indice| marco(1, &format!("linea {indice}\n")))
            .collect::<Vec<_>>()
            .concat();
        let servidor = servidor_feliz(false, cuerpo_logs).await;
        let fuente = PodmanRegistros::new(servidor.ruta().to_path_buf());

        let cuerpo = fuente
            .obtener("/v1.40/containers/abc123/logs?follow=true")
            .await
            .expect("el stream de logs se abre");

        let (emisor, mut receptor) = mpsc::channel(CAPACIDAD_DE_REGISTROS);
        let tarea = tokio::spawn(bombear(cuerpo, false, emisor));

        // Nadie lee: el productor llena el canal y se aparca ahí.
        assert!(
            esperar_a_que_llene(&receptor, CAPACIDAD_DE_REGISTROS).await,
            "el canal tiene que llenarse hasta su capacidad"
        );

        // Drenar una línea reanuda al productor: estaba aparcado, no perdido.
        let primera = receptor
            .recv()
            .await
            .expect("la primera línea está en el canal");
        assert_eq!(primera.line, "linea 0");

        assert!(
            esperar_a_que_llene(&receptor, CAPACIDAD_DE_REGISTROS).await,
            "tras drenar, el productor tiene que reanudar y volver a llenar"
        );

        // Y se drena todo, en orden y sin perder ninguna.
        let mut lineas = vec![primera];
        while let Some(linea) = receptor.recv().await {
            lineas.push(linea);
        }

        assert_eq!(lineas.len(), total, "no se puede perder ninguna línea");
        for (indice, linea) in lineas.iter().enumerate() {
            assert_eq!(linea.line, format!("linea {indice}"), "el orden importa");
        }

        tarea
            .await
            .expect("el bombeo termina cuando el cuerpo se acaba");
    }
}
