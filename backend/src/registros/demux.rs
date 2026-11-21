//! El demultiplexado del stream de logs de Podman, como pieza **pura**.
//!
//! Cuando un contenedor **no** usa TTY, Podman contesta el stream en el formato
//! multiplexado de Docker: por cada trozo, una cabecera de ocho bytes —el byte 0
//! dice el origen (`0` entrada estándar, `1` salida estándar, `2` error
//! estándar), los bytes 1..4 son relleno y los bytes 4..8 la longitud de la carga
//! en **big-endian**— seguida de la carga. Cuando el contenedor **sí** usa TTY el
//! stream es crudo: texto tal cual, sin marcos.
//!
//! Aquí no hay ni red ni ficheros: entra un trozo de bytes y salen las líneas
//! completas que ese trozo permite cerrar. Es lo que hace que la parte difícil
//! —los marcos que se parten entre dos lecturas, las líneas que se parten entre
//! dos marcos— se pueda probar con bytes fabricados, sin socket y sin Podman.

use serde::Serialize;
use thiserror::Error;

/// La cabecera de un marco multiplexado: origen, relleno y longitud.
const CABECERA: usize = 8;

/// El mayor marco que se acepta como legítimo, en bytes.
///
/// Podman trocea los logs en marcos de como mucho decenas de kibibytes, así que un
/// mebibyte es un tope holgado para lo que el formato usa de verdad. Existe porque
/// una longitud **desmesurada** no se puede esperar: con la cabecera anunciando
/// gibibytes que nunca llegan, el búfer crecería sin freno hasta agotar la memoria.
/// Al superarlo, el stream se da por roto —no hay forma de resincronizarlo— y se
/// corta.
const TOPE_DE_MARCO: usize = 1024 * 1024;

/// Por qué el demultiplexador no puede seguir leyendo el stream.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DemuxError {
    /// La cabecera anuncia un marco más grande que cualquier marco razonable.
    ///
    /// No es una lectura corta —ésas se esperan—: es una longitud imposible, y
    /// esperarla sólo haría crecer el búfer. El stream está roto.
    #[error("la cabecera anuncia un marco de {longitud} bytes, por encima del tope de {tope}")]
    MarcoDesmesurado {
        /// La longitud que anunciaba la cabecera.
        longitud: usize,
        /// El tope que se aplicó.
        tope: usize,
    },
}

/// El origen de una línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origen {
    /// Salida estándar del contenedor.
    Stdout,
    /// Error estándar del contenedor.
    Stderr,
}

impl Origen {
    /// El origen como texto, tal y como viaja al panel.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// Una línea del contenedor, con el origen del que salió.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineaDeRegistro {
    /// El texto de la línea, sin el salto que la terminaba.
    pub line: String,
    /// De qué salida salió.
    pub stream: Origen,
}

impl LineaDeRegistro {
    /// Una línea nueva, sin salto.
    pub fn nueva(line: impl Into<String>, stream: Origen) -> Self {
        Self {
            line: line.into(),
            stream,
        }
    }
}

/// El demultiplexador **incremental** del stream de logs.
///
/// Es incremental y no una función de bytes a líneas porque un trozo de red no
/// respeta las fronteras del formato: un marco puede llegar partido —incluso
/// partida su cabecera— y una línea puede quedar a medias al final del trozo. El
/// estado vive aquí y [`Demux::alimentar`] devuelve **sólo** lo que ya se puede
/// cerrar; lo demás se queda pendiente hasta el trozo siguiente.
#[derive(Debug)]
pub struct Demux {
    /// Si el stream viene multiplexado (contenedor sin TTY).
    multiplexado: bool,
    /// Bytes recibidos que todavía no forman un marco completo.
    pendiente: Vec<u8>,
    /// Bytes de la línea en curso, aún sin salto.
    parcial: Vec<u8>,
    /// El origen de la línea en curso.
    origen: Origen,
}

impl Demux {
    /// Un demultiplexador para un contenedor con —o sin— TTY.
    ///
    /// Con TTY el stream es crudo: todo lo que llegue es texto de la salida
    /// estándar, sin cabeceras que interpretar.
    pub fn new(tty: bool) -> Self {
        Self {
            multiplexado: !tty,
            pendiente: Vec::new(),
            parcial: Vec::new(),
            origen: Origen::Stdout,
        }
    }

    /// Alimenta el demultiplexador con un trozo y devuelve las líneas cerradas.
    ///
    /// Sólo salen líneas **completas**: las que terminan en salto dentro de este
    /// trozo. La última línea a medias —y los marcos incompletos— se guardan para
    /// el trozo siguiente.
    ///
    /// Falla cuando el stream está roto de forma irrecuperable: una cabecera que
    /// anuncia un marco por encima de [`TOPE_DE_MARCO`]. Esperar esa carga sería
    /// crecer sin freno, así que se corta en vez de intentarlo.
    pub fn alimentar(&mut self, bytes: &[u8]) -> Result<Vec<LineaDeRegistro>, DemuxError> {
        let mut salida = Vec::new();

        if !self.multiplexado {
            self.trocear(bytes, Origen::Stdout, &mut salida);
            return Ok(salida);
        }

        self.pendiente.extend_from_slice(bytes);

        let mut cursor = 0;
        while self.pendiente.len() >= cursor + CABECERA {
            let mut cabecera = [0u8; CABECERA];
            cabecera.copy_from_slice(&self.pendiente[cursor..cursor + CABECERA]);

            let origen = match cabecera[0] {
                // El `2` es el error estándar; el `0` (entrada) y el `1` (salida)
                // se tratan igual: podpot no pide la entrada, así que lo único
                // que puede llegar por el `0` es ruido, y llamarlo salida estándar
                // es lo menos engañoso.
                2 => Origen::Stderr,
                _ => Origen::Stdout,
            };
            let longitud =
                u32::from_be_bytes([cabecera[4], cabecera[5], cabecera[6], cabecera[7]]) as usize;

            // Una longitud imposible se corta aquí: el búfer no puede crecer
            // esperando una carga que no cabe en ningún marco legítimo.
            if longitud > TOPE_DE_MARCO {
                return Err(DemuxError::MarcoDesmesurado {
                    longitud,
                    tope: TOPE_DE_MARCO,
                });
            }

            // El marco aún no está entero: se espera al trozo siguiente sin
            // consumir nada. La carga se copia antes de tocar `self` para no
            // prestar `pendiente` mientras se muta el estado.
            if self.pendiente.len() < cursor + CABECERA + longitud {
                break;
            }

            let carga = self.pendiente[cursor + CABECERA..cursor + CABECERA + longitud].to_vec();
            cursor += CABECERA + longitud;

            self.trocear(&carga, origen, &mut salida);
        }

        if cursor > 0 {
            self.pendiente.drain(..cursor);
        }

        Ok(salida)
    }

    /// Cierra lo que quede a medias, si es que queda algo.
    ///
    /// Es lo que se llama al terminar el stream: un contenedor que muere sin
    /// escribir el último salto dejaría su última línea invisible para siempre.
    /// Un marco incompleto —cabecera sin carga— **no** se inventa: no hay línea
    /// que sacar de ahí.
    pub fn terminar(&mut self) -> Option<LineaDeRegistro> {
        if self.parcial.is_empty() {
            return None;
        }

        let mut salida = Vec::new();
        self.cerrar(&mut salida);

        salida.pop()
    }

    /// Reparte un bloque de carga en líneas, cambiando de origen si hace falta.
    fn trocear(&mut self, bytes: &[u8], origen: Origen, salida: &mut Vec<LineaDeRegistro>) {
        if bytes.is_empty() {
            return;
        }

        // Un cambio de origen cierra la línea en curso aunque no traiga salto: sin
        // esto, media línea de la salida estándar se pegaría con media del error
        // estándar y el panel mentiría sobre de dónde salió.
        if origen != self.origen && !self.parcial.is_empty() {
            self.cerrar(salida);
        }

        self.origen = origen;

        for &byte in bytes {
            if byte == b'\n' {
                self.cerrar(salida);
            } else {
                self.parcial.push(byte);
            }
        }
    }

    /// Cierra la línea en curso y la añade a la salida.
    fn cerrar(&mut self, salida: &mut Vec<LineaDeRegistro>) {
        let mut bytes = std::mem::take(&mut self.parcial);

        // Un stream de terminal termina las líneas en `\r\n`: el `\r` es del
        // terminal, no del contenido, y dejarlo metería un carácter invisible en
        // cada línea del panel.
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }

        salida.push(LineaDeRegistro {
            line: String::from_utf8_lossy(&bytes).into_owned(),
            stream: self.origen,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{Demux, DemuxError, LineaDeRegistro, Origen, TOPE_DE_MARCO};

    /// Fabrica un marco multiplexado: origen, relleno, longitud y carga.
    fn marco(origen: u8, carga: &str) -> Vec<u8> {
        let mut bytes = vec![origen, 0, 0, 0];
        bytes.extend_from_slice(&(carga.len() as u32).to_be_bytes());
        bytes.extend_from_slice(carga.as_bytes());

        bytes
    }

    /// Alimenta el demultiplexador con un trozo que se sabe sano.
    ///
    /// Los trozos de estas pruebas son marcos bien formados, así que el único
    /// fallo posible —una longitud desmesurada— no puede ocurrir aquí: se
    /// desempaqueta sin más para que las aserciones hablen de líneas.
    fn alimentar(demux: &mut Demux, bytes: &[u8]) -> Vec<LineaDeRegistro> {
        demux
            .alimentar(bytes)
            .expect("un trozo de prueba no trae marcos desmesurados")
    }

    /// Une varios marcos en un solo trozo.
    fn marcos(partes: &[Vec<u8>]) -> Vec<u8> {
        partes.iter().flatten().copied().collect()
    }

    fn linea(texto: &str, stream: Origen) -> LineaDeRegistro {
        LineaDeRegistro::nueva(texto, stream)
    }

    /// Escenario «Contenedor sin TTY»: los marcos de la salida estándar se
    /// convierten en líneas con su origen.
    #[test]
    fn un_marco_de_stdout_da_una_linea_de_stdout() {
        let mut demux = Demux::new(false);

        let salida = alimentar(&mut demux, &marco(1, "arrancando\n"));

        assert_eq!(salida, [linea("arrancando", Origen::Stdout)]);
    }

    /// Escenario «Contenedor sin TTY»: el byte `2` delata el error estándar.
    #[test]
    fn un_marco_de_stderr_da_una_linea_de_stderr() {
        let mut demux = Demux::new(false);

        let salida = alimentar(&mut demux, &marco(2, "no se pudo abrir el fichero\n"));

        assert_eq!(
            salida,
            [linea("no se pudo abrir el fichero", Origen::Stderr)]
        );
    }

    /// Un marco puede traer **varias** líneas dentro.
    #[test]
    fn un_marco_con_varias_lineas_las_entrega_todas() {
        let mut demux = Demux::new(false);

        let salida = alimentar(&mut demux, &marco(1, "una\ndos\ntres\n"));

        assert_eq!(
            salida,
            [
                linea("una", Origen::Stdout),
                linea("dos", Origen::Stdout),
                linea("tres", Origen::Stdout),
            ]
        );
    }

    /// Y un trozo puede traer **varios** marcos, cada uno con su origen.
    #[test]
    fn varios_marcos_en_un_trozo_conservan_su_origen() {
        let mut demux = Demux::new(false);

        let salida = alimentar(
            &mut demux,
            &marcos(&[marco(1, "listo\n"), marco(2, "aviso\n"), marco(1, "sigo\n")]),
        );

        assert_eq!(
            salida,
            [
                linea("listo", Origen::Stdout),
                linea("aviso", Origen::Stderr),
                linea("sigo", Origen::Stdout),
            ]
        );
    }

    /// Un marco partido entre dos lecturas **no** se pierde: hasta que no llega
    /// entero no sale nada, y cuando llega sale la línea completa.
    #[test]
    fn un_marco_partido_entre_dos_trozos_espera_al_segundo() {
        let mut demux = Demux::new(false);
        let entero = marco(1, "partida\n");
        let (primera, segunda) = entero.split_at(5);

        assert!(
            alimentar(&mut demux, primera).is_empty(),
            "con medio marco no hay línea que dar"
        );

        let salida = alimentar(&mut demux, segunda);

        assert_eq!(salida, [linea("partida", Origen::Stdout)]);
    }

    /// La cabecera también puede llegar partida —incluso byte a byte—, que es el
    /// caso real de un socket.
    #[test]
    fn una_cabecera_partida_byte_a_byte_se_recompone() {
        let mut demux = Demux::new(false);
        let entero = marco(2, "de uno en uno\n");

        let mut salida = Vec::new();
        for (indice, byte) in entero.iter().enumerate() {
            let trozo = alimentar(&mut demux, &[*byte]);

            if indice + 1 < entero.len() {
                assert!(trozo.is_empty(), "aún no había marco completo en {indice}");
            } else {
                salida = trozo;
            }
        }

        assert_eq!(salida, [linea("de uno en uno", Origen::Stderr)]);
    }

    /// Una línea partida entre **dos marcos** del mismo origen se recompone: el
    /// salto es lo único que la cierra.
    #[test]
    fn una_linea_partida_entre_dos_marcos_se_recompone() {
        let mut demux = Demux::new(false);

        assert!(alimentar(&mut demux, &marco(1, "media ")).is_empty());

        let salida = alimentar(&mut demux, &marco(1, "linea\n"));

        assert_eq!(salida, [linea("media linea", Origen::Stdout)]);
    }

    /// Un cambio de origen cierra la línea en curso: media línea de la salida
    /// estándar no se pega con media del error estándar.
    #[test]
    fn un_cambio_de_origen_cierra_la_linea_en_curso() {
        let mut demux = Demux::new(false);

        assert!(alimentar(&mut demux, &marco(1, "sin salto")).is_empty());

        let salida = alimentar(&mut demux, &marco(2, "del error\n"));

        assert_eq!(
            salida,
            [
                linea("sin salto", Origen::Stdout),
                linea("del error", Origen::Stderr),
            ]
        );
    }

    /// Al terminar el stream, la última línea sin salto no se queda invisible.
    #[test]
    fn terminar_cierra_la_ultima_linea_sin_salto() {
        let mut demux = Demux::new(false);

        assert!(alimentar(&mut demux, &marco(2, "ultima sin salto")).is_empty());
        assert_eq!(
            demux.terminar(),
            Some(linea("ultima sin salto", Origen::Stderr))
        );

        // Y sólo una vez: después ya no queda nada que cerrar.
        assert_eq!(demux.terminar(), None);
    }

    /// Un `\r` final es del terminal, no del contenido.
    #[test]
    fn un_salto_de_terminal_no_deja_el_retorno_de_carro() {
        let mut demux = Demux::new(false);

        let salida = alimentar(&mut demux, &marco(1, "con retorno\r\n"));

        assert_eq!(salida, [linea("con retorno", Origen::Stdout)]);
    }

    /// Escenario «Contenedor con TTY»: el stream es crudo —texto sin marcos— y
    /// todo sale como salida estándar, sin interpretar ningún byte.
    #[test]
    fn un_contenedor_con_tty_no_demultiplexa() {
        let mut demux = Demux::new(true);

        // El primer byte es un `2`: si se interpretara como marco, el stream se
        // leería como basura. Con TTY, es texto y punto.
        let salida = alimentar(&mut demux, b"2\0\0\0\0\0\0\0hola\n");

        assert_eq!(salida, [linea("2\0\0\0\0\0\0\0hola", Origen::Stdout)]);
    }

    /// Y con TTY el `\r\n` de terminal también se limpia.
    #[test]
    fn con_tty_el_salto_de_terminal_tambien_se_limpia() {
        let mut demux = Demux::new(true);

        let salida = alimentar(&mut demux, b"una\r\ndos\r\n");

        assert_eq!(
            salida,
            [linea("una", Origen::Stdout), linea("dos", Origen::Stdout),]
        );
    }

    /// Un byte no UTF-8 no rompe la lectura: se sustituye, como en cualquier
    /// texto que viene de fuera.
    #[test]
    fn un_byte_que_no_es_utf8_no_rompe_la_lectura() {
        let mut demux = Demux::new(true);

        let salida = alimentar(&mut demux, b"a\xffb\n");

        assert_eq!(salida.len(), 1);
        assert_eq!(salida[0].stream, Origen::Stdout);
        assert!(
            salida[0].line.starts_with('a') && salida[0].line.ends_with('b'),
            "el byte inválido se sustituye y el resto se conserva: {:?}",
            salida[0].line
        );
    }

    /// Un marco de longitud cero no aporta línea, pero tampoco atasca el
    /// demultiplexador.
    #[test]
    fn un_marco_vacio_no_rompe_la_secuencia() {
        let mut demux = Demux::new(false);

        let salida = alimentar(&mut demux, &marcos(&[marco(1, ""), marco(1, "sigo\n")]));

        assert_eq!(salida, [linea("sigo", Origen::Stdout)]);
    }

    /// Una longitud que no puede ser la de un marco de logs es un stream **roto**,
    /// no una lectura corta: esperarla sólo haría crecer el búfer sin freno.
    #[test]
    fn un_marco_desmesurado_es_un_error() {
        let mut demux = Demux::new(false);

        // Una cabecera de salida estándar que anuncia cuatro gibibytes de carga.
        let mut cabecera = vec![1, 0, 0, 0];
        cabecera.extend_from_slice(&u32::MAX.to_be_bytes());

        let error = demux
            .alimentar(&cabecera)
            .expect_err("un marco de cuatro gibibytes no puede ser de logs");

        assert_eq!(
            error,
            DemuxError::MarcoDesmesurado {
                longitud: u32::MAX as usize,
                tope: TOPE_DE_MARCO,
            }
        );
    }
}
