//! El doble de la fuente de registros: líneas sin socket.
//!
//! Es lo que permite probar el canal SSE entero —cola inicial, líneas nuevas,
//! fin del stream, unidad sin contenedor— sin un Podman en pie y sin fabricar
//! marcos: el doble entrega `LineaDeRegistro` ya hechas, que es exactamente lo
//! que el puerto promete. Los marcos y el socket se prueban aparte, en
//! [`crate::registros::demux`] y [`crate::registros::podman`].
//!
//! Cada suscriptor recibe lo que se emita **a partir de su suscripción**, como el
//! stream de verdad: la prueba abre el canal, lee la cola inicial y después emite
//! para medir que la línea nueva llega.

use std::sync::Mutex;

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::{CAPACIDAD_DE_REGISTROS, FuenteDeRegistros, LineaDeRegistro, RegistrosError};
use crate::systemd::UnitName;

/// Lo que el doble hace cuando le piden seguir una unidad.
#[derive(Debug, Clone, Copy)]
enum Comportamiento {
    /// Entrega líneas: la cola inicial y las que se emitan.
    Entrega,
    /// Finge una unidad sin contenedor.
    SinContenedor,
}

/// Una fuente de registros que no habla con nadie.
#[derive(Debug)]
pub struct FakeRegistros {
    /// Las líneas que se entregan como cola inicial a cada suscriptor.
    iniciales: Vec<LineaDeRegistro>,
    /// Los suscriptores vivos, por su extremo de escritura.
    ///
    /// Se guardan para poder emitir a todos: un `mpsc` es de un productor y un
    /// consumidor, así que el doble hace de repartidor. Un suscriptor cuyo
    /// receptor se soltó —la prueba cerró el canal— se da de baja al emitir: el
    /// envío falla con «cerrado» y se descarta, que es lo mismo que hace el stream
    /// de verdad.
    suscriptores: Mutex<Vec<mpsc::Sender<LineaDeRegistro>>>,
    /// Qué hace al pedirle registros.
    comportamiento: Comportamiento,
}

impl Default for FakeRegistros {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeRegistros {
    /// Un doble que entrega lo que se le emita, sin cola inicial.
    pub fn new() -> Self {
        Self::con([])
    }

    /// Un doble con la cola inicial que se le dé.
    pub fn con(iniciales: impl IntoIterator<Item = LineaDeRegistro>) -> Self {
        Self {
            iniciales: iniciales.into_iter().collect(),
            suscriptores: Mutex::new(Vec::new()),
            comportamiento: Comportamiento::Entrega,
        }
    }

    /// Un doble que finge una unidad sin contenedor.
    ///
    /// Es la puerta del `404` envuelto: el desenlace que tiene que llegar
    /// **antes** de anunciar `text/event-stream`.
    pub fn sin_contenedor() -> Self {
        Self {
            iniciales: Vec::new(),
            suscriptores: Mutex::new(Vec::new()),
            comportamiento: Comportamiento::SinContenedor,
        }
    }

    /// Emite una línea a todos los suscriptores vivos.
    ///
    /// No bloquea: si el canal de un suscriptor está lleno, la línea se descarta
    /// **para él** —el suscriptor sigue apuntado, y lo que no cabe es la línea—. Es
    /// una prueba, y lo que mide es que la línea llega; esperar aquí colgaría la
    /// prueba por un motivo que no es el suyo.
    ///
    /// Un canal lleno **no** da de baja al suscriptor: `TrySendError::Full` dice
    /// que no cabe, no que el receptor se haya ido; confundir los dos borraría al
    /// suscriptor y sus líneas siguientes se perderían para siempre.
    pub fn emitir(&self, linea: LineaDeRegistro) {
        let mut suscriptores = self
            .suscriptores
            .lock()
            .unwrap_or_else(|envenenado| envenenado.into_inner());

        suscriptores.retain(|emisor| {
            matches!(
                emisor.try_send(linea.clone()),
                Ok(()) | Err(mpsc::error::TrySendError::Full(_))
            )
        });
    }

    /// Cierra todos los canales, como si el stream de Podman hubiera terminado.
    pub fn cerrar(&self) {
        let mut suscriptores = self
            .suscriptores
            .lock()
            .unwrap_or_else(|envenenado| envenenado.into_inner());

        suscriptores.clear();
    }
}

#[async_trait]
impl FuenteDeRegistros for FakeRegistros {
    async fn seguir(
        &self,
        unidad: &UnitName,
        _cola: usize,
    ) -> Result<mpsc::Receiver<LineaDeRegistro>, RegistrosError> {
        if let Comportamiento::SinContenedor = self.comportamiento {
            return Err(RegistrosError::SinContenedor {
                unidad: unidad.as_str().to_owned(),
            });
        }

        let (emisor, receptor) = mpsc::channel(CAPACIDAD_DE_REGISTROS);

        // La cola inicial se entrega **antes** de devolver el receptor: quien lee
        // la tiene esperando desde la primera vuelta del flujo, sin que haga
        // falta emitir nada. `try_send` y no `send` porque aquí no se puede
        // esperar —nadie está leyendo todavía—.
        for linea in &self.iniciales {
            let _ = emisor.try_send(linea.clone());
        }

        self.suscriptores
            .lock()
            .unwrap_or_else(|envenenado| envenenado.into_inner())
            .push(emisor);

        Ok(receptor)
    }
}
