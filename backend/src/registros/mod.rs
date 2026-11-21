//! Los registros del contenedor de una quadlet.
//!
//! Una quadlet **es** una unidad de systemd, pero sus logs no están en el journal
//! del usuario: están en el contenedor que Podman arrancó. La vía alineada con la
//! política del proyecto —hablar, no invocar binarios— es la **API de Podman por
//! su socket unix**, que ya es accesible desde el contenedor de podpot bajo
//! `/run/user/<uid>`.
//!
//! El módulo se parte en tres piezas con una responsabilidad cada una:
//!
//! * [`demux`] — el formato del stream de Podman, como pieza pura y probada con
//!   bytes fabricados: los marcos, las líneas partidas y el caso con TTY.
//! * [`podman`] — el cliente por socket: localizar el contenedor de la unidad por
//!   su etiqueta, mirar si usa TTY y seguir sus logs.
//! * el puerto [`FuenteDeRegistros`] — lo único que la capa API conoce, para que
//!   el canal SSE no dependa de que haya un socket ni un Podman en pie.
//!
//! El **mapeo unidad → contenedor** va por la etiqueta que systemd pone a los
//! contenedores que gestiona (`PODMAN_SYSTEMD_UNIT=<unidad>.service`), y no por
//! el nombre del fichero: la etiqueta es la fuente de verdad, y así no hay que
//! leer `ContainerName=` de la quadlet.

pub mod demux;
pub mod podman;

/// El doble de pruebas de la fuente de registros.
///
/// Sólo existe en las pruebas: a diferencia del doble de systemd —que también es
/// un modo de desarrollo para arrancar podpot sin bus—, aquí no hay nada que
/// fingir en producción, porque un registro inventado no sirve para diagnosticar
/// nada. En desarrollo se usa el socket de verdad o no se usan los logs.
#[cfg(test)]
pub mod fake;

use std::fmt;

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::systemd::UnitName;

pub use demux::{Demux, DemuxError, LineaDeRegistro, Origen};

/// Cuántas líneas caben en el canal antes de aplicar contrapresión.
///
/// No es un número pequeño porque una ráfaga —el arranque de un contenedor
/// verboso— no puede llenarlo antes de que el flujo SSE tenga ocasión de drenar;
/// y no es enorme porque el canal es la frontera donde se para al productor si el
/// consumidor se atasca. Lleno, el que lee espera: es contrapresión, no pérdida.
pub const CAPACIDAD_DE_REGISTROS: usize = 256;

/// Cuántas líneas de cola se piden al abrir el canal.
///
/// Es la «cola inicial» del contrato: quien abre el visor tiene que ver contexto
/// sin esperar a que el contenedor escriba. Doscientas líneas llenan una pantalla
/// y algo más sin convertir el primer evento en un volcado.
pub const COLA_INICIAL: usize = 200;

/// Por qué no se pueden leer los registros de una unidad.
///
/// Cada variante distingue un desenlace distinto porque la traducción a HTTP no
/// es la misma: una unidad sin contenedor es un `404`, un Podman que no contesta
/// un `503` y una respuesta que no se puede usar un `502`. El `Display` —que sí
/// puede nombrar la causa, porque acaba en el registro— no se publica nunca al
/// cliente: `src/error.rs` construye su propio mensaje para cada variante.
#[derive(Debug, Error)]
pub enum RegistrosError {
    /// La unidad no tiene contenedor del que leer.
    ///
    /// Es el caso de un `.volume`, un `.network` o una unidad que no es una
    /// quadlet de contenedor: la etiqueta no aparece en ningún contenedor. No es
    /// un `5xx` —no ha fallado nada— sino un recurso que no está.
    #[error("la unidad `{unidad}` no tiene contenedor del que leer registros")]
    SinContenedor {
        /// El nombre de la unidad.
        unidad: String,
    },

    /// No se pudo hablar con el socket de la API de Podman.
    ///
    /// La causa va detrás de un `Box<dyn Error>` para que el doble de pruebas
    /// pueda construirla sin un socket de verdad; su texto —una ruta, el `Display`
    /// de hyper— **no** sale al cliente.
    #[error("no se pudo hablar con la API de Podman por `{socket}`: {causa}")]
    Socket {
        /// La ruta del socket con el que se intentó hablar.
        socket: String,
        /// La causa, tal y como la informó la biblioteca.
        #[source]
        causa: Box<dyn std::error::Error + Send + Sync>,
    },

    /// La API de Podman contestó algo que no se puede usar.
    ///
    /// No es un fallo de transporte —la petición llegó y hubo respuesta—, sino
    /// una respuesta inesperada: un estado que no es `200` o un documento que no
    /// se puede leer. La culpa es del otro lado, y de ahí el `502`.
    #[error("la API de Podman contestó algo que no se puede usar: {causa}")]
    Podman {
        /// La causa, tal y como la informó la biblioteca.
        #[source]
        causa: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// El contrato de la fuente de registros.
///
/// Es un trait y no un tipo concreto por la misma razón que `SystemdClient`: las
/// pruebas no pueden depender de que la máquina que compila tenga un Podman con
/// el socket en pie. La capa API sólo conoce este puerto —no hyper, ni el
/// formato de los marcos—, así que el canal SSE se prueba entero sin socket.
///
/// Devuelve un **canal** y no un `Stream` porque lo que el flujo necesita es
/// contrapresión de verdad: si el cliente SSE no drena —un navegador en una
/// pestaña dormida—, el que lee del socket deja de leer y Podman deja de escribir,
/// en vez de acumular en memoria.
#[async_trait]
pub trait FuenteDeRegistros: Send + Sync + fmt::Debug {
    /// Empieza a seguir los registros del contenedor de una unidad.
    ///
    /// Devuelve el extremo de lectura del canal, que ya trae la **cola inicial**
    /// —las últimas `cola` líneas— y después cada línea nueva según se produce.
    /// Cuando el stream de Podman termina, el canal se cierra y el receptor
    /// devuelve `None`.
    ///
    /// Falla **antes** de devolver nada cuando la unidad no tiene contenedor o
    /// Podman no contesta: así quien abre el canal SSE puede contestar el error
    /// antes de haber anunciado `text/event-stream`.
    async fn seguir(
        &self,
        unidad: &UnitName,
        cola: usize,
    ) -> Result<mpsc::Receiver<LineaDeRegistro>, RegistrosError>;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::systemd::UnitName;

    use super::fake::FakeRegistros;
    use super::{
        CAPACIDAD_DE_REGISTROS, COLA_INICIAL, FuenteDeRegistros, LineaDeRegistro, Origen,
        RegistrosError,
    };

    /// El puerto vale como objeto: el estado de la aplicación guarda un
    /// `Arc<dyn FuenteDeRegistros>` y el doble se inyecta por ahí.
    #[tokio::test]
    async fn el_puerto_vale_detras_de_un_arco() {
        let fuente: Arc<dyn FuenteDeRegistros> = Arc::new(FakeRegistros::new());
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        let receptor = fuente
            .seguir(&unidad, COLA_INICIAL)
            .await
            .expect("el doble no falla");

        assert_eq!(
            receptor.len(),
            0,
            "el doble sin líneas no entrega nada, pero tampoco falla"
        );
    }

    /// Escenario «El doble entrega líneas»: la fuente de pruebas entrega líneas
    /// sin socket real —es lo que permite probar el canal SSE entero sin Podman—.
    #[tokio::test]
    async fn el_doble_entrega_lineas_sin_socket() {
        let doble = FakeRegistros::con([LineaDeRegistro::nueva("hola", Origen::Stdout)]);
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        let mut receptor = doble
            .seguir(&unidad, COLA_INICIAL)
            .await
            .expect("el doble no falla");

        // La cola inicial llega sin que nadie la pida.
        assert_eq!(
            receptor.recv().await,
            Some(LineaDeRegistro::nueva("hola", Origen::Stdout))
        );

        // Y una línea nueva, emitida después, también.
        doble.emitir(LineaDeRegistro::nueva("mundo", Origen::Stderr));

        assert_eq!(
            receptor.recv().await,
            Some(LineaDeRegistro::nueva("mundo", Origen::Stderr))
        );
    }

    /// Al cerrar el doble, el canal se cierra: es lo que el flujo traduce en el
    /// evento terminal.
    #[tokio::test]
    async fn cerrar_el_doble_cierra_el_canal() {
        let doble = FakeRegistros::new();
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        let mut receptor = doble
            .seguir(&unidad, COLA_INICIAL)
            .await
            .expect("el doble no falla");

        doble.cerrar();

        assert_eq!(
            receptor.recv().await,
            None,
            "sin emisores, el receptor tiene que ver el fin del stream"
        );
    }

    /// Y el doble puede fingir el caso de la unidad sin contenedor, que es el
    /// desenlace que la API tiene que contestar **antes** de abrir el flujo.
    #[tokio::test]
    async fn el_doble_puede_fingir_una_unidad_sin_contenedor() {
        let doble = FakeRegistros::sin_contenedor();
        let unidad = UnitName::parse("datos-volume.service").expect("nombre válido");

        let error = doble
            .seguir(&unidad, COLA_INICIAL)
            .await
            .expect_err("una unidad sin contenedor no puede dar registros");

        assert!(
            matches!(&error, RegistrosError::SinContenedor { unidad } if unidad == "datos-volume.service"),
            "se esperaba «sin contenedor» nombrando la unidad: {error}"
        );
    }

    /// Un canal lleno **no** da de baja al suscriptor: «no cabe» y «se fue» no son
    /// lo mismo, y confundirlos borraría al suscriptor —sus líneas siguientes se
    /// perderían para siempre—.
    #[tokio::test]
    async fn un_canal_lleno_no_borra_al_suscriptor() {
        let doble = FakeRegistros::new();
        let unidad = UnitName::parse("centinela-db.service").expect("nombre válido");

        let mut receptor = doble
            .seguir(&unidad, COLA_INICIAL)
            .await
            .expect("el doble no falla");

        // Se llena el canal sin que nadie lea.
        for indice in 0..CAPACIDAD_DE_REGISTROS {
            doble.emitir(LineaDeRegistro::nueva(
                format!("lleno {indice}"),
                Origen::Stdout,
            ));
        }

        // Y una más, con el canal lleno: la línea no cabe, pero el suscriptor sigue.
        doble.emitir(LineaDeRegistro::nueva("de sobra", Origen::Stdout));

        // Drenar una deja hueco, y lo que se emita después tiene que llegar: si el
        // suscriptor se hubiera borrado, el receptor estaría cerrado.
        let primera = receptor.recv().await.expect("el canal sigue vivo");
        assert_eq!(primera.line, "lleno 0");

        doble.emitir(LineaDeRegistro::nueva("tras drenar", Origen::Stderr));

        let mut ultima = None;
        for _ in 0..CAPACIDAD_DE_REGISTROS {
            ultima = Some(
                receptor
                    .recv()
                    .await
                    .expect("el suscriptor no se borró: el canal sigue vivo"),
            );
        }

        assert_eq!(
            ultima,
            Some(LineaDeRegistro::nueva("tras drenar", Origen::Stderr)),
            "la línea emitida después de drenar tiene que llegar"
        );
    }
}
