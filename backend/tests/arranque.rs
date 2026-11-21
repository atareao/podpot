//! Prueba de proceso del apagado ordenado.
//!
//! Es la mitad que ninguna prueba de la biblioteca puede medir: `server::serve`
//! acepta el futuro de cierre por parámetro, así que su prueba fija la
//! composición, pero que el **proceso** atienda `SIGTERM`, termine con el código
//! de éxito y no se lleve nada por delante sólo se ve arrancando el binario.
//!
//! Se arranca con el doble de systemd y una base temporal —no hay bus de usuario
//! en todas las máquinas que compilan— y con `RUST_LOG=off`, para que lo que el
//! proceso escriba no se confunda con lo que escribe la prueba.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// El binario que Cargo compila para esta prueba.
const BINARIO: &str = env!("CARGO_BIN_EXE_podpot");

/// Lo que se espera, como mucho, a que el proceso arranque o se apague.
const PLAZO: Duration = Duration::from_secs(15);

/// El proceso de podpot, que se termina a la fuerza si la prueba falla antes de
/// haberlo apagado con la señal.
struct Proceso {
    hijo: Child,
}

impl Proceso {
    /// Lanza el binario con el entorno que se le dé.
    fn arrancar(direccion: SocketAddr, dsn: &str) -> Self {
        let hijo = Command::new(BINARIO)
            .env("BIND_ADDRESS", direccion.to_string())
            .env("DATABASE_URL", dsn)
            .env("PODPOT_SYSTEMD", "fake")
            // El socket se declara para que el arranque no dependa del entorno de
            // quien compila: nada se conecta a él —el proceso sólo escucha y se
            // apaga—, pero sin esta variable la configuración exigiría el
            // `XDG_RUNTIME_DIR` del usuario y el proceso no arrancaría en una
            // máquina que no lo tenga.
            .env("PODPOT_PODMAN_SOCKET", "/run/podpot-de-prueba/podman.sock")
            .env("RUST_LOG", "off")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("el binario de podpot tiene que estar compilado");

        Self { hijo }
    }

    /// Manda `SIGTERM` al proceso, como haría un orquestador al apagarlo.
    fn terminar(&self) -> std::io::Result<()> {
        let pid = self.hijo.id();

        // SAFETY: `kill` recibe un pid y una señal y no toca memoria propia. El pid
        // es el de un hijo que este proceso arrancó y todavía no ha recogido, así
        // que no puede referirse a otro proceso.
        let resultado = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };

        if resultado == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    /// Espera a que termine, hasta el plazo; `None` si sigue vivo al agotarse.
    fn esperar(&mut self, plazo: Duration) -> Option<ExitStatus> {
        let limite = Instant::now() + plazo;

        loop {
            match self.hijo.try_wait() {
                Ok(Some(salida)) => return Some(salida),
                Ok(None) if Instant::now() < limite => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Ok(None) | Err(_) => return None,
            }
        }
    }
}

impl Drop for Proceso {
    /// Si la prueba falla antes de apagarlo, no se deja un podpot vivo escuchando.
    fn drop(&mut self) {
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
}

/// Escenario «Terminación limpia»: el proceso arranca, atiende la salud y sale
/// con éxito cuando llega `SIGTERM`.
#[test]
fn el_proceso_arranca_atiende_la_salud_y_sale_limpio_con_sigterm() {
    let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
    let dsn = format!("sqlite://{}/podpot.db", dir.path().display());
    let direccion = puerto_libre();

    let mut proceso = Proceso::arrancar(direccion, &dsn);

    let respuesta = esperar_salud(direccion);

    assert!(
        respuesta.starts_with("HTTP/1.1 200"),
        "la ruta de salud no contestó 200: {respuesta}"
    );
    assert!(
        respuesta.contains("\"status\":\"ok\""),
        "la salud no traía el estado del proceso: {respuesta}"
    );

    // La base la abrió el proceso donde se le dijo, no en el directorio de trabajo.
    assert!(
        dir.path().join("podpot.db").exists(),
        "el proceso no creó la base del DSN que se le dio"
    );

    proceso
        .terminar()
        .expect("se tiene que poder mandar SIGTERM al proceso");

    let salida = proceso
        .esperar(PLAZO)
        .expect("el proceso tiene que terminar cuando llega SIGTERM");

    assert!(
        salida.success(),
        "la salida no fue limpia —un proceso que no atiende SIGTERM muere por la señal—: {salida:?}"
    );
    assert_eq!(salida.code(), Some(0), "se esperaba un código de salida 0");

    // Y el puerto queda libre: la escucha se cerró antes de salir.
    assert!(
        TcpStream::connect_timeout(&direccion, Duration::from_millis(500)).is_err(),
        "el proceso terminó pero su puerto sigue atendiendo"
    );
}

/// Un puerto libre de loopback: se pide al sistema y se suelta.
///
/// La carrera entre soltar el puerto y que el hijo lo pida existe y es pequeña: lo
/// que se pide es un puerto que nadie está usando, no una reserva.
fn puerto_libre() -> SocketAddr {
    let escucha = TcpListener::bind("127.0.0.1:0").expect("se tiene que poder abrir un puerto");
    let direccion = escucha
        .local_addr()
        .expect("un listener abierto tiene dirección local");

    drop(escucha);

    direccion
}

/// Pregunta por la salud hasta que el proceso contesta o se agota el plazo.
fn esperar_salud(direccion: SocketAddr) -> String {
    let limite = Instant::now() + PLAZO;
    let mut ultimo = String::new();

    while Instant::now() < limite {
        match salud(direccion) {
            Ok(respuesta) if respuesta.starts_with("HTTP/1.1 200") => return respuesta,
            Ok(respuesta) => ultimo = respuesta,
            Err(error) => ultimo = error.to_string(),
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    panic!("el proceso no atendió la salud en {PLAZO:?}: {ultimo}");
}

/// Un `GET /api/v1/health` con el mínimo de HTTP escrito a mano.
fn salud(direccion: SocketAddr) -> std::io::Result<String> {
    let mut flujo = TcpStream::connect_timeout(&direccion, Duration::from_millis(500))?;
    flujo.set_read_timeout(Some(Duration::from_secs(5)))?;

    flujo.write_all(
        b"GET /api/v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )?;

    let mut respuesta = String::new();
    flujo.read_to_string(&mut respuesta)?;

    Ok(respuesta)
}
