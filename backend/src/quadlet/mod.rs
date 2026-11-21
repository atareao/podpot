//! La creación de quadlets nuevas: el vocabulario y las reglas del dominio.
//!
//! Una quadlet **es** un fichero de Podman del que el generador de systemd
//! deriva una unidad, así que crearla son cuatro pasos y en este módulo viven
//! los dos que **no** tocan nada:
//!
//! 1. validar el nombre y el tipo ([`QuadletDraft::validate`]),
//! 2. derivar el contenido del fichero de la plantilla tipada
//!    ([`QuadletDraft::render`]) —nunca de un cuerpo que mande el cliente—,
//! 3. escribir en el directorio canónico ([`store`]) y
//! 4. recargar systemd y registrar la creación ([`service`]).
//!
//! El tipo es un enumerado cerrado y no la extensión en crudo porque las que se
//! pueden **crear** son sólo cuatro: `.kube`, `.image` y `.build` son quadlets
//! que podpot sabe **ver**, pero no fabricar, y `.desktop` sale del mismo
//! generador sin ser una quadlet. La extensión que llega se lee con
//! [`QuadletKind::from_extension`], que es la única puerta a un [`QuadletKind`].

pub mod service;
pub mod store;

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::systemd::{SystemdError, UnitName};

/// La longitud máxima del **stem** de una quadlet.
///
/// Acota el nombre del fichero sin su extensión, para no crear un nombre que
/// ningún sistema de ficheros quiera guardar.
const MAX_STEM: usize = 200;

/// Los tipos de quadlet que podpot sabe **crear**.
///
/// No es la lista de los que sabe **leer** ([`crate::systemd::QUADLET_EXTENSIONS`]
/// los conoce todos): `.kube`, `.image` y `.build` se listan si están, pero su
/// creación no está en el alcance de esta entrega, así que aquí no están.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum QuadletKind {
    /// Un contenedor: genera `<stem>.service`.
    Container,
    /// Un volumen: genera `<stem>-volume.service`.
    Volume,
    /// Una red: genera `<stem>-network.service`.
    Network,
    /// Un pod: genera `<stem>-pod.service`.
    Pod,
}

impl QuadletKind {
    /// Todos los tipos que se pueden crear.
    pub const TODAS: [Self; 4] = [Self::Container, Self::Volume, Self::Network, Self::Pod];

    /// Lee el tipo a partir de la extensión del fichero, sin el punto.
    ///
    /// Cualquier cosa que no sea una de las cuatro extensiones creables es un
    /// [`QuadletError::InvalidKind`] y **no** un valor por defecto: aceptar una
    /// extensión desconocida y escribirla sería escribir un fichero que el
    /// generador no sabe leer.
    pub fn from_extension(extension: &str) -> Result<Self, QuadletError> {
        match extension {
            "container" => Ok(Self::Container),
            "volume" => Ok(Self::Volume),
            "network" => Ok(Self::Network),
            "pod" => Ok(Self::Pod),
            otra => Err(QuadletError::InvalidKind {
                kind: otra.to_owned(),
            }),
        }
    }

    /// La extensión del fichero, sin el punto.
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::Volume => "volume",
            Self::Network => "network",
            Self::Pod => "pod",
        }
    }

    /// El nombre de unidad que systemd genera para `<stem>.<ext>`.
    ///
    /// El mapeo no es el mismo para todos: un `.container` da `<stem>.service`
    /// —el generador no añade sufijo— y los tipos con sufijo (`.volume`,
    /// `.network`, `.pod`) dan `<stem>-<ext>.service`. El nombre no se construye
    /// con `format!` y se suelta: pasa por [`UnitName::parse`], que es lo que
    /// garantiza que lo que se devuelve es un nombre de unidad de verdad.
    pub fn unit_name(&self, stem: &str) -> Result<UnitName, QuadletError> {
        let nombre = match self {
            Self::Container => format!("{stem}.service"),
            Self::Volume => format!("{stem}-volume.service"),
            Self::Network => format!("{stem}-network.service"),
            Self::Pod => format!("{stem}-pod.service"),
        };

        // El nombre se construye aquí pero se **valida** al envolverlo: si el
        // `format!` hubiera producido algo que systemd no acepta, lo dice
        // `UnitName::parse` y no lo descubre luego una ruta de D-Bus.
        UnitName::parse(&nombre).map_err(QuadletError::from)
    }
}

impl fmt::Display for QuadletKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.extension())
    }
}

/// Por qué no se pudo crear una quadlet.
///
/// Cada variante distingue un desenlace que no se traduce igual a HTTP: una
/// colisión es un `409`, un nombre o un tipo inválidos son un `400`, un fallo de
/// escritura es un `500` y los fallos de systemd se dejan tal y como los clasifica
/// [`crate::systemd::SystemdError`] —un `502` para la recarga o la unidad
/// rechazada, un `503` para un bus que no contesta—.
#[derive(Debug, Error)]
pub enum QuadletError {
    /// Ya existe una quadlet con ese nombre en el directorio de quadlets.
    ///
    /// El fichero destino **no se toca**: la unicidad se comprueba antes de
    /// escribir, y la escritura vuelve a negarse si algo se cuela entre medias.
    #[error("ya existe una quadlet con el fichero `{file_name}` en el directorio de quadlets")]
    Duplicate {
        /// El nombre de fichero que ya estaba en uso.
        file_name: String,
    },

    /// El stem no es un nombre de fichero admisible.
    #[error("`{stem}` no es un nombre de quadlet válido")]
    InvalidName {
        /// El stem tal y como llegó.
        stem: String,
    },

    /// La extensión no es la de un tipo de quadlet que se pueda crear.
    #[error("`{kind}` no es un tipo de quadlet que se pueda crear")]
    InvalidKind {
        /// La extensión tal y como llegó.
        kind: String,
    },

    /// El tipo exige un campo que la petición no traía.
    #[error("el tipo `{kind}` exige el campo `{field}`")]
    MissingField {
        /// El tipo que exige el campo.
        kind: &'static str,
        /// El campo que falta.
        field: &'static str,
    },

    /// El valor de un campo traía estructura donde sólo cabe un valor.
    ///
    /// Un valor acaba en una línea `Clave=valor` del fichero, así que un salto de
    /// línea —o cualquier carácter de control— no sería parte del valor: sería
    /// una directiva o una sección nueva. La plantilla tipada sólo garantiza
    /// estructura si el valor no puede contener estructura, y por eso se rechaza.
    ///
    /// El rechazo nombra el **campo** y nunca el valor, como cualquier `4xx` de la
    /// casa: el valor que llegó se queda en el registro.
    #[error("el campo `{field}` tiene un valor no válido")]
    InvalidValue {
        /// El campo cuyo valor no vale.
        field: &'static str,
    },

    /// El fichero no se pudo escribir.
    ///
    /// El mensaje **no** publica la ruta: el fichero vive en el home del usuario
    /// y la traducción a HTTP responde con un mensaje genérico.
    #[error("no se pudo escribir la quadlet `{file_name}`: {causa}")]
    WriteFailed {
        /// El nombre de fichero destino.
        file_name: String,
        /// El motivo, tal y como lo informó el sistema de ficheros.
        #[source]
        causa: std::io::Error,
    },

    /// La creación se completó, pero no se pudo registrar.
    ///
    /// Es un fallo del propio podpot, no de la petición: se traduce a un `500`
    /// genérico y el detalle se queda en el registro.
    #[error("no se pudo registrar la creación de `{file_name}`: {causa}")]
    Persistencia {
        /// El nombre de fichero de la quadlet creada.
        file_name: String,
        /// El motivo, tal y como lo informó SQLite.
        #[source]
        causa: Box<sqlx::Error>,
    },

    /// El paso por systemd falló.
    ///
    /// Se reutiliza tal cual —`UnitRejected` y `Transport` incluidos— porque la
    /// traducción a HTTP de cada variante ya está decidida en un solo sitio.
    #[error(transparent)]
    Systemd(#[from] SystemdError),
}

/// Lo que el cliente envía para crear una quadlet: la **entrada tipada**.
///
/// Es un struct con campos y no un cuerpo de fichero: el contrato no admite
/// contenido libre —`deny_unknown_fields` cierra la puerta a un campo `content`
/// o a cualquier otro que no esté declarado—, así que el fichero se **deriva**
/// de estos campos y nunca se copia de lo que llegue.
///
/// `name` es el stem (sin extensión) y `kind` la extensión, sin el punto. Un
/// `name` que ya traiga la extensión (`web.container`) se acepta y se recorta
/// para no acabar con un `web.container.container`.
///
/// Los campos por tipo van todos en el mismo struct porque el cuerpo es un solo
/// objeto JSON; los que exige cada tipo se validan al renderizar
/// ([`QuadletDraft::render`]), no aquí: un `Option` que no se usa en ese tipo se
/// ignora en silencio, que es lo que se espera de un campo de otro tipo.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuadletDraft {
    /// El stem del fichero, sin la extensión.
    pub name: String,
    /// La extensión, sin el punto (`container`, `volume`, `network`, `pod`).
    pub kind: String,
    /// La imagen del contenedor (`.container`, obligatoria).
    #[serde(default)]
    pub image: Option<String>,
    /// El `ContainerName=` del contenedor.
    #[serde(default)]
    pub container_name: Option<String>,
    /// Los `PublishPort=` del contenedor o del pod, en orden.
    #[serde(default)]
    pub publish_ports: Vec<String>,
    /// Los `Volume=` del contenedor, en orden.
    #[serde(default)]
    pub volumes: Vec<String>,
    /// Los `Environment=` del contenedor, en orden.
    #[serde(default)]
    pub environments: Vec<String>,
    /// El `VolumeName=` del volumen.
    #[serde(default)]
    pub volume_name: Option<String>,
    /// El `NetworkName=` de la red.
    #[serde(default)]
    pub network_name: Option<String>,
    /// El `Subnet=` de la red.
    #[serde(default)]
    pub subnet: Option<String>,
    /// El `PodName=` del pod.
    #[serde(default)]
    pub pod_name: Option<String>,
}

impl QuadletDraft {
    /// El stem, ya sin la extensión aunque el nombre la traiga puesta.
    fn stem(&self) -> &str {
        let sufijo = format!(".{}", self.kind);
        self.name.strip_suffix(&sufijo).unwrap_or(&self.name)
    }

    /// El nombre de fichero que se va a escribir: `<stem>.<ext>`.
    pub fn file_name(&self) -> String {
        format!("{}.{}", self.stem(), self.kind)
    }

    /// El tipo, leído de la extensión.
    pub fn kind(&self) -> Result<QuadletKind, QuadletError> {
        QuadletKind::from_extension(&self.kind)
    }

    /// Comprueba el nombre y el tipo, sin tocar el disco.
    pub fn validate(&self) -> Result<QuadletKind, QuadletError> {
        let stem = self.stem();

        if !stem_valido(stem) {
            return Err(QuadletError::InvalidName {
                stem: stem.to_owned(),
            });
        }

        self.kind()
    }

    /// Renderiza el contenido del fichero desde la plantilla de su tipo.
    ///
    /// El orden de las líneas es fijo —el del propio tipo y, dentro de él, el de
    /// los vectores, que conservan el orden en que llegaron— para que dos
    /// peticiones iguales den un fichero idéntico. No hay ninguna línea que venga
    /// de fuera: cada sección y cada clave las pone esta función, y de la petición
    /// sólo salen los **valores** de los campos declarados, que además pasan por
    /// [`valor_de_campo`]: un valor no puede traer estructura, así que no puede
    /// colar una directiva ni una sección nueva.
    pub fn render(&self) -> Result<String, QuadletError> {
        let kind = self.kind()?;

        // Una cabecera de sección y sus líneas `Clave=valor`, en el orden en que
        // se van añadiendo. Se compone con `push_str` en vez de con `format!` de
        // una pieza entera para que el orden quede a la vista, línea a línea.
        let mut fichero = String::new();

        match kind {
            QuadletKind::Container => {
                fichero.push_str("[Container]\n");

                let imagen = self.image.as_deref().ok_or(QuadletError::MissingField {
                    kind: "container",
                    field: "image",
                })?;
                fichero.push_str(&format!("Image={}\n", valor_de_campo("image", imagen)?));

                if let Some(nombre) = &self.container_name {
                    fichero.push_str(&format!(
                        "ContainerName={}\n",
                        valor_de_campo("container_name", nombre)?
                    ));
                }
                for puerto in &self.publish_ports {
                    fichero.push_str(&format!(
                        "PublishPort={}\n",
                        valor_de_campo("publish_ports", puerto)?
                    ));
                }
                for volumen in &self.volumes {
                    fichero.push_str(&format!("Volume={}\n", valor_de_campo("volumes", volumen)?));
                }
                for entorno in &self.environments {
                    fichero.push_str(&format!(
                        "Environment={}\n",
                        valor_de_campo("environments", entorno)?
                    ));
                }
            }
            QuadletKind::Volume => {
                fichero.push_str("[Volume]\n");

                if let Some(nombre) = &self.volume_name {
                    fichero.push_str(&format!(
                        "VolumeName={}\n",
                        valor_de_campo("volume_name", nombre)?
                    ));
                }
            }
            QuadletKind::Network => {
                fichero.push_str("[Network]\n");

                if let Some(nombre) = &self.network_name {
                    fichero.push_str(&format!(
                        "NetworkName={}\n",
                        valor_de_campo("network_name", nombre)?
                    ));
                }
                if let Some(subnet) = &self.subnet {
                    fichero.push_str(&format!("Subnet={}\n", valor_de_campo("subnet", subnet)?));
                }
            }
            QuadletKind::Pod => {
                fichero.push_str("[Pod]\n");

                if let Some(nombre) = &self.pod_name {
                    fichero.push_str(&format!(
                        "PodName={}\n",
                        valor_de_campo("pod_name", nombre)?
                    ));
                }
                for puerto in &self.publish_ports {
                    fichero.push_str(&format!(
                        "PublishPort={}\n",
                        valor_de_campo("publish_ports", puerto)?
                    ));
                }
            }
        }

        Ok(fichero)
    }

    /// Resuelve el nombre de fichero y el de unidad generado.
    pub fn quadlet(&self) -> Result<Quadlet, QuadletError> {
        let kind = self.validate()?;
        let stem = self.stem();

        Ok(Quadlet {
            file_name: self.file_name(),
            unit_name: kind.unit_name(stem)?,
            kind,
        })
    }
}

/// El resultado de crear una quadlet.
///
/// Lleva lo que la API puede publicar y nada más: **nunca** la ruta del fichero,
/// que es un detalle del sistema de ficheros del usuario y no se publica —igual
/// que [`crate::systemd::UnitOrigin`] no se publica en el listado—.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quadlet {
    /// El nombre del fichero escrito (`web.container`).
    pub file_name: String,
    /// El nombre de la unidad que genera systemd (`web.service`).
    pub unit_name: UnitName,
    /// El tipo de quadlet creada.
    pub kind: QuadletKind,
}

/// El valor de un campo, ya comprobado y recortado, o el rechazo que le toca.
///
/// Un valor de la plantilla acaba en una línea `Clave=valor`, así que **no** puede
/// traer estructura: un salto de línea, un retorno o cualquier carácter de control
/// convertirían el valor en una directiva o en una sección nueva, y eso es
/// exactamente lo que el contrato prohíbe. Se recorta —un valor vacío o con sólo
/// espacios no es un valor— y se rechaza con [`QuadletError::InvalidValue`], que
/// nombra el campo y nunca el valor.
fn valor_de_campo<'a>(campo: &'static str, valor: &'a str) -> Result<&'a str, QuadletError> {
    let limpio = valor.trim();

    if limpio.is_empty() || limpio.chars().any(char::is_control) {
        return Err(QuadletError::InvalidValue { field: campo });
    }

    Ok(limpio)
}

/// ¿El stem puede ser el nombre de un fichero de quadlet?
///
/// No es una lista blanca de lo que systemd admite, sino de lo que podpot está
/// dispuesto a **crear**: un nombre que empiece por letra o dígito y siga con
/// letras, dígitos, `-`, `_` o `.`. Eso deja fuera lo que convertiría el nombre
/// en una ruta (`/`, `..`), lo que systemd no sabría leer (espacios, controles) y
/// los ficheros ocultos (un stem que empieza por `.`). No se admiten acentos: el
/// nombre de fichero viaja a una ruta de unidad de systemd y ahí no pinta bien
/// nada que no sea ASCII.
fn stem_valido(stem: &str) -> bool {
    if stem.is_empty() || stem.len() > MAX_STEM {
        return false;
    }

    let mut caracteres = stem.chars();

    let Some(primero) = caracteres.next() else {
        return false;
    };

    if !primero.is_ascii_alphanumeric() {
        return false;
    }

    caracteres
        .all(|caracter| caracter.is_ascii_alphanumeric() || matches!(caracter, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::systemd::UnitName;

    use super::{QuadletDraft, QuadletError, QuadletKind};

    /// Un borrador con sólo el nombre y el tipo.
    fn borrador(name: &str, kind: &str) -> QuadletDraft {
        QuadletDraft {
            name: name.to_owned(),
            kind: kind.to_owned(),
            ..QuadletDraft::default()
        }
    }

    /// Un contenedor con lo mínimo que exige su tipo.
    fn contenedor(name: &str) -> QuadletDraft {
        QuadletDraft {
            image: Some("docker.io/library/nginx:latest".to_owned()),
            ..borrador(name, "container")
        }
    }

    /// Escenario «Contenido derivado del tipo»: el tipo se lee de la extensión.
    #[test]
    fn el_tipo_se_lee_de_la_extension() {
        assert_eq!(
            QuadletKind::from_extension("container").expect("es un tipo creable"),
            QuadletKind::Container
        );
        assert_eq!(
            QuadletKind::from_extension("volume").expect("es un tipo creable"),
            QuadletKind::Volume
        );
        assert_eq!(
            QuadletKind::from_extension("network").expect("es un tipo creable"),
            QuadletKind::Network
        );
        assert_eq!(
            QuadletKind::from_extension("pod").expect("es un tipo creable"),
            QuadletKind::Pod
        );

        for tipo in QuadletKind::TODAS {
            assert_eq!(
                QuadletKind::from_extension(tipo.extension()).expect("ida y vuelta"),
                tipo
            );
        }
    }

    /// Escenario «Extensión no permitida» y «`.desktop` excluida»: lo que no sea
    /// una extensión creable es `InvalidKind`, también una que podpot sepa leer.
    #[test]
    fn una_extension_no_creable_es_invalid_kind() {
        for extension in [
            "desktop",
            "kube",
            "image",
            "build",
            "service",
            "socket",
            "container ",
            "",
        ] {
            let error =
                QuadletKind::from_extension(extension).expect_err("no puede ser un tipo creable");

            assert!(
                matches!(error, QuadletError::InvalidKind { .. }),
                "se esperaba un tipo inválido para {extension:?}: {error}"
            );
        }
    }

    /// Escenario «Nombre de unidad de contenedor»: sin sufijo de tipo.
    #[test]
    fn un_contenedor_genera_la_unidad_sin_sufijo() {
        assert_eq!(
            QuadletKind::Container
                .unit_name("web")
                .expect("es un nombre de unidad válido")
                .as_str(),
            "web.service"
        );
    }

    /// Escenario «Nombre de unidad de recurso»: `<stem>-<ext>.service`, y un
    /// stem con punto no se confunde con una extensión.
    #[test]
    fn un_recurso_genera_la_unidad_con_su_extension() {
        assert_eq!(
            QuadletKind::Volume
                .unit_name("web-data")
                .expect("válido")
                .as_str(),
            "web-data-volume.service"
        );
        assert_eq!(
            QuadletKind::Network
                .unit_name("web")
                .expect("válido")
                .as_str(),
            "web-network.service"
        );
        assert_eq!(
            QuadletKind::Pod.unit_name("web").expect("válido").as_str(),
            "web-pod.service"
        );
        assert_eq!(
            QuadletKind::Container
                .unit_name("voicebox.prod")
                .expect("válido")
                .as_str(),
            "voicebox.prod.service"
        );
    }

    /// Escenario «Nombre libre»: un stem con letras, dígitos y separadores vale.
    #[test]
    fn los_stems_admisibles_pasan_la_validacion() {
        for bueno in ["web", "voicebox.prod", "ata-CT2000_1", "a1", "web2"] {
            borrador(bueno, "container")
                .validate()
                .unwrap_or_else(|error| panic!("{bueno:?} tendría que valer: {error}"));
        }
    }

    /// Escenario «Caracteres no válidos en el stem»: lo que convertiría el nombre
    /// en una ruta o en un nombre que systemd no lee es `InvalidName`.
    #[test]
    fn un_stem_con_caracteres_no_admisibles_es_invalid_name() {
        for malo in [
            "",
            "with space",
            "foo/bar",
            "..",
            ".hidden",
            "-foo",
            "web;rm",
            "weñ",
            "web\n",
            "web\t.container",
        ] {
            let error = borrador(malo, "container")
                .validate()
                .expect_err("no puede colarse como nombre de quadlet");

            assert!(
                matches!(error, QuadletError::InvalidName { .. }),
                "se esperaba un nombre inválido para {malo:?}: {error}"
            );
        }
    }

    /// El tipo se valida en el mismo paso: `.desktop` no es un tipo creable.
    #[test]
    fn un_tipo_no_creable_no_pasa_la_validacion() {
        let error = borrador("web", "desktop")
            .validate()
            .expect_err("no puede colarse como tipo creable");

        assert!(matches!(error, QuadletError::InvalidKind { .. }), "{error}");
    }

    /// Escenario «Contenido derivado del tipo»: el fichero de un contenedor sale
    /// de sus campos, en un orden estable.
    #[test]
    fn el_contenedor_se_renderiza_desde_sus_campos() {
        let draft = QuadletDraft {
            image: Some("docker.io/library/nginx:latest".to_owned()),
            container_name: Some("web".to_owned()),
            publish_ports: vec!["8080:80".to_owned(), "8443:443".to_owned()],
            volumes: vec!["web-data:/data:Z".to_owned()],
            environments: vec!["TZ=Europe/Madrid".to_owned()],
            ..borrador("web", "container")
        };

        assert_eq!(
            draft
                .render()
                .expect("un contenedor con imagen se renderiza"),
            "[Container]\n\
             Image=docker.io/library/nginx:latest\n\
             ContainerName=web\n\
             PublishPort=8080:80\n\
             PublishPort=8443:443\n\
             Volume=web-data:/data:Z\n\
             Environment=TZ=Europe/Madrid\n"
        );
    }

    /// Un contenedor sin campos opcionales lleva sólo la imagen.
    #[test]
    fn el_contenedor_minimo_lleva_solo_la_imagen() {
        assert_eq!(
            contenedor("web").render().expect("se renderiza"),
            "[Container]\nImage=docker.io/library/nginx:latest\n"
        );
    }

    /// Y el render es determinista: dos veces, lo mismo.
    #[test]
    fn el_render_es_determinista() {
        let draft = contenedor("web");

        assert_eq!(
            draft.render().expect("se renderiza"),
            draft.render().expect("se renderiza")
        );
    }

    #[test]
    fn el_volumen_se_renderiza_desde_sus_campos() {
        let con_nombre = QuadletDraft {
            volume_name: Some("web-data".to_owned()),
            ..borrador("web-data", "volume")
        };

        assert_eq!(
            con_nombre.render().expect("se renderiza"),
            "[Volume]\nVolumeName=web-data\n"
        );
        assert_eq!(
            borrador("web-data", "volume")
                .render()
                .expect("se renderiza"),
            "[Volume]\n"
        );
    }

    #[test]
    fn la_red_se_renderiza_desde_sus_campos() {
        let completa = QuadletDraft {
            network_name: Some("web-net".to_owned()),
            subnet: Some("10.88.0.0/24".to_owned()),
            ..borrador("web-net", "network")
        };

        assert_eq!(
            completa.render().expect("se renderiza"),
            "[Network]\nNetworkName=web-net\nSubnet=10.88.0.0/24\n"
        );
        assert_eq!(
            borrador("web-net", "network")
                .render()
                .expect("se renderiza"),
            "[Network]\n"
        );
    }

    #[test]
    fn el_pod_se_renderiza_desde_sus_campos() {
        let completo = QuadletDraft {
            pod_name: Some("web".to_owned()),
            publish_ports: vec!["8080:80".to_owned()],
            ..borrador("web", "pod")
        };

        assert_eq!(
            completo.render().expect("se renderiza"),
            "[Pod]\nPodName=web\nPublishPort=8080:80\n"
        );
        assert_eq!(
            borrador("web", "pod").render().expect("se renderiza"),
            "[Pod]\n"
        );
    }

    /// Un contenedor sin imagen no se puede renderizar: la plantilla pide el
    /// campo y su ausencia es un rechazo de la petición, no un fichero a medias.
    #[test]
    fn un_contenedor_sin_imagen_deja_un_campo_que_falta() {
        let error = borrador("web", "container")
            .render()
            .expect_err("un contenedor sin imagen no se puede escribir");

        assert!(
            matches!(
                error,
                QuadletError::MissingField {
                    kind: "container",
                    field: "image"
                }
            ),
            "se esperaba que faltara el campo image: {error}"
        );
    }

    /// Escenario «Contenido libre rechazado»: un campo `content` (o cualquier
    /// otro que no esté declarado) no encaja en el contrato, así que la petición
    /// se rechaza antes de llegar a tocar nada.
    #[test]
    fn el_contrato_no_admite_un_cuerpo_de_fichero_arbitrario() {
        let cuerpo = json!({
            "name": "web",
            "kind": "container",
            "content": "[Container]\nImage=lo-que-sea\n",
        });

        let error = serde_json::from_value::<QuadletDraft>(cuerpo)
            .expect_err("un cuerpo de fichero libre no puede colarse en el contrato");

        assert!(
            error.to_string().contains("content"),
            "el rechazo tiene que nombrar el campo que no encaja: {error}"
        );
    }

    /// Y el contenido sale del tipo, no de lo que llegue: dos borradores del
    /// mismo nombre y distinto tipo dan ficheros distintos.
    #[test]
    fn el_contenido_sale_del_tipo_y_no_de_otro_sitio() {
        let contenedor = contenedor("web").render().expect("se renderiza");
        let volumen = borrador("web", "volume").render().expect("se renderiza");

        assert_ne!(contenedor, volumen);
        assert!(contenedor.starts_with("[Container]"), "{contenedor}");
        assert!(volumen.starts_with("[Volume]"), "{volumen}");
    }

    /// Escenario «Valor con estructura»: un valor no puede traer un salto de
    /// línea —ni ningún carácter de control—, porque acabaría en una línea
    /// `Clave=valor` del fichero y un `\n` sería una directiva nueva colada por la
    /// puerta del valor. La plantilla tipada sólo garantiza estructura si el
    /// valor no puede contener estructura.
    #[test]
    fn un_valor_con_salto_de_linea_no_inyecta_directivas() {
        let draft = QuadletDraft {
            image: Some("nginx\nUser=root".to_owned()),
            ..borrador("web", "container")
        };

        let error = draft
            .render()
            .expect_err("un valor con un salto de línea no puede colarse");

        assert!(
            matches!(error, QuadletError::InvalidValue { field: "image" }),
            "se esperaba un valor inválido en `image`: {error}"
        );
    }

    /// El rechazo vale para cualquier campo de texto y para cualquier carácter de
    /// control, no sólo para el `image` de un contenedor.
    #[test]
    fn ningun_campo_admite_controles_ni_valores_vacios() {
        let casos: [(&str, QuadletDraft); 5] = [
            (
                "container_name",
                QuadletDraft {
                    container_name: Some("web\r\nUser=root".to_owned()),
                    ..contenedor("web")
                },
            ),
            (
                "volumes",
                QuadletDraft {
                    volumes: vec!["/data\0:/data".to_owned()],
                    ..contenedor("web")
                },
            ),
            (
                "environments",
                QuadletDraft {
                    environments: vec!["TZ=Europe/Madrid\tX".to_owned()],
                    ..contenedor("web")
                },
            ),
            (
                "volume_name",
                QuadletDraft {
                    volume_name: Some("   ".to_owned()),
                    ..borrador("web-data", "volume")
                },
            ),
            (
                "subnet",
                QuadletDraft {
                    subnet: Some("10.88.0.0/24\nSubnet=1.2.3.4/32".to_owned()),
                    ..borrador("web-net", "network")
                },
            ),
        ];

        for (campo, draft) in casos {
            match draft.render() {
                Ok(fichero) => panic!("el valor de `{campo}` se coló en el fichero: {fichero}"),
                Err(error) => assert!(
                    matches!(&error, QuadletError::InvalidValue { field } if *field == campo),
                    "se esperaba un valor inválido en `{campo}`: {error}"
                ),
            }
        }
    }

    /// Documenta el comportamiento actual de una colisión de stem entre tipos
    /// distintos: la unicidad es por `file_name` completo y no por stem, así que
    /// `web.container` y `web.volume` conviven —es el ajuste del spec a la
    /// decisión real de directorio canónico único—.
    #[test]
    fn dos_tipos_con_el_mismo_stem_conviven() {
        let contenedor = contenedor("web").quadlet().expect("un contenedor válido");
        let volumen = borrador("web", "volume")
            .quadlet()
            .expect("un volumen válido");

        assert_eq!(contenedor.file_name, "web.container");
        assert_eq!(volumen.file_name, "web.volume");
        assert_ne!(
            contenedor.unit_name, volumen.unit_name,
            "cada tipo genera su propia unidad"
        );
    }

    /// El borrador resuelve el fichero y la unidad que le toca.
    #[test]
    fn el_borrador_resuelve_fichero_y_unidad() {
        let quadlet = contenedor("web").quadlet().expect("un contenedor válido");

        assert_eq!(quadlet.file_name, "web.container");
        assert_eq!(
            quadlet.unit_name,
            UnitName::parse("web.service").expect("es un nombre válido")
        );
        assert_eq!(quadlet.kind, QuadletKind::Container);
    }

    /// Un nombre que ya trae la extensión se recorta: no se acaba con
    /// `web.container.container`.
    #[test]
    fn un_nombre_que_ya_trae_la_extension_no_la_duplica() {
        let draft = QuadletDraft {
            image: Some("nginx".to_owned()),
            ..borrador("web.container", "container")
        };

        assert_eq!(draft.file_name(), "web.container");
        assert_eq!(
            draft.quadlet().expect("se resuelve").file_name,
            "web.container"
        );
    }

    /// Un recurso resuelve su nombre de unidad con su sufijo.
    #[test]
    fn un_recurso_resuelve_la_unidad_con_su_sufijo() {
        let quadlet = borrador("web-data", "volume")
            .quadlet()
            .expect("un volumen válido");

        assert_eq!(quadlet.file_name, "web-data.volume");
        assert_eq!(quadlet.unit_name.as_str(), "web-data-volume.service");
        assert_eq!(quadlet.kind, QuadletKind::Volume);
    }
}
