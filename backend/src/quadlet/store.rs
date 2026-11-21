//! La escritura y la lectura de quadlets en el directorio canónico.
//!
//! Todo lo que este módulo toca el disco vive aquí y en ningún otro sitio, para
//! que la parte que decide qué se escribe ([`crate::quadlet`]) sea pura y se
//! pueda probar sin un directorio temporal.
//!
//! Dos reglas gobiernan la escritura:
//!
//! * **Sólo el primer nivel** del directorio configurado: una quadlet se escribe
//!   donde el generador la busca, y no en un subdirectorio —el descubrimiento de
//!   podpot tampoco mira más abajo—.
//! * **No pisar**: la apertura es `create_new`, así que un fichero que ya exista
//!   en el destino **no se abre siquiera**. Comprobar y escribir en dos pasos
//!   dejaría una carrera entre el `stat` y el `open`; con `create_new` la negativa
//!   la da el propio sistema de ficheros y no hay hueco para colarse.
//!
//! Y una tercera que vale para las cuatro operaciones: **ninguna sigue enlaces
//! simbólicos**. Un enlace en el directorio de quadlets ocupa su nombre, pero no
//! es una definición: leerlo sería leer un fichero que no está en el primer nivel
//! —quizá uno de fuera del directorio— y el nombre de la quadlet acabaría
//! apuntando a donde no debe.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::io::AsyncWriteExt;

use super::QuadletError;

/// ¿Ya hay algo con ese nombre en el directorio?
///
/// Se mira con `symlink_metadata` —y no con `metadata`— porque un **symlink**
/// roto también ocupa el nombre: preguntar por el destino del enlace diría que no
/// y la escritura se negaría después, con un error peor.
pub async fn existe(dir: &Path, file_name: &str) -> Result<bool, QuadletError> {
    match tokio::fs::symlink_metadata(dir.join(file_name)).await {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(causa) => Err(fallo_de_escritura(file_name, causa)),
    }
}

/// Escribe el fichero en el primer nivel de `dir`, sin pisar lo que haya.
///
/// No devuelve la ruta: el único consumidor no la usa, y publicarla sería abrir la
/// puerta a que se filtrara. Lo que importa del resultado es que el fichero quedó
/// escrito entero, o el fallo que lo impidió.
///
/// El fichero **no** se deja a medias: si el contenido no se puede volcar entero,
/// se borra lo que se hubiera escrito y se devuelve el fallo, para que un error de
/// escritura no deje una definición truncada que systemd intentaría leer.
///
/// **Permisos.** Nace `0600` —sólo su dueño— por la misma razón que el temporal de
/// [`reescribir`]: lo que hay dentro es una definición que puede incluir
/// `Environment=…PASSWORD=…`, y no tiene por qué ser legible por nadie más. El modo
/// se pide al **abrir** y no se corrige después con un `chmod`: no puede existir un
/// instante en el que el fichero esté escrito y accesible a terceros.
pub async fn escribir(dir: &Path, file_name: &str, contenido: &str) -> Result<(), QuadletError> {
    let destino = dir.join(file_name);

    let mut opciones = tokio::fs::OpenOptions::new();
    opciones.write(true).create_new(true);

    // El modo con el que nace: sólo su dueño —ver «Permisos» en el docstring—.
    #[cfg(unix)]
    opciones.mode(0o600);

    // `create_new` es lo que hace que la negativa a pisar no sea una carrera: un
    // `stat` que dijera «no hay nada» podría quedar obsoleto antes del `open`, y
    // el sistema de ficheros es quien mejor sabe si el nombre estaba libre. Si
    // el nombre lo ocupó otra creación entre la precomprobación y aquí, el
    // `AlreadyExists` se traduce a un duplicado —un `409`— y no a un fallo de
    // escritura —un `500`—.
    let mut fichero = opciones
        .open(&destino)
        .await
        .map_err(|causa| fallo_de_apertura(file_name, causa))?;

    if let Err(causa) = fichero.write_all(contenido.as_bytes()).await {
        // Se cierra y se borra lo que hubiera quedado: una definición a medias es
        // peor que ninguna, porque el generador la leería en la próxima recarga.
        drop(fichero);
        let _ = tokio::fs::remove_file(&destino).await;

        return Err(fallo_de_escritura(file_name, causa));
    }

    fichero
        .flush()
        .await
        .map_err(|causa| fallo_de_escritura(file_name, causa))?;

    Ok(())
}

/// Reescribe el fichero en el primer nivel de `dir` **de forma atómica**.
///
/// Es la escritura de la edición y no la de la creación: aquí el destino **sí**
/// se reemplaza. La atomicidad no es un adorno —una quadlet a medias es una
/// definición truncada que el generador leería en la próxima recarga—, así que el
/// contenido se escribe en un **fichero temporal del mismo directorio** y, sólo
/// cuando está entero en disco, se hace `rename` sobre el destino. `rename` es
/// atómico dentro del mismo sistema de ficheros: quien lea el destino ve el
/// contenido de antes o el de después, nunca una mezcla, y un fallo antes del
/// `rename` deja el original intacto.
///
/// **Permisos.** El temporal nace `0600` —sólo su dueño— porque lo que lleva
/// dentro es una definición que puede incluir `Environment=…PASSWORD=…`, y no
/// puede haber un instante en el que cualquiera la lea. Cuando el destino ya
/// existía, sus permisos se copian al temporal **antes** del `rename`: así editar
/// no degrada un `0600` a un `0644` ni endurece lo que el dueño eligió. Si el
/// destino no existía —el caso de carrera de abajo—, el fichero se queda con el
/// `0600` con el que nació.
///
/// **Durabilidad.** El temporal se vacía al disco (`fsync`) antes del `rename`,
/// para que lo que ya se ve no dependa de buffers que un corte de luz se llevaría
/// por delante.
///
/// Si algo falla, el temporal se retira y el destino no se toca. El nombre del
/// temporal no termina en extensión de quadlet, así que el descubrimiento no lo
/// confundiría con una quadlet ni en el hueco entre el fallo y su limpieza.
///
/// La existencia del destino **no** se comprueba aquí: la comprueba quien llama
/// antes de renderizar ([`crate::quadlet::service::editar`]). Queda una carrera
/// **benigna** y aceptada a conciencia: si el fichero se borra entre esa
/// comprobación y el `rename`, el `rename` lo **resucita** con el contenido nuevo
/// —el borrado concurrente no se deshace, se solapa— y la respuesta dice que la
/// edición salió bien, que es lo que pasó desde el punto de vista de esta
/// petición. Añadir un `stat` no cerraría la ventana —entre el `stat` y el
/// `rename` sigue habiendo tiempo—, así que se prefiere no gastar la llamada: no
/// hay ningún desenlace en el que el `stat` cambie la decisión.
pub async fn reescribir(dir: &Path, file_name: &str, contenido: &str) -> Result<(), QuadletError> {
    let destino = dir.join(file_name);

    // El temporal vive en el **mismo** directorio: `rename` sólo es atómico dentro
    // del mismo sistema de ficheros, y `/tmp` podría ser otro.
    let temporal = dir.join(nombre_temporal(file_name));

    let mut opciones = tokio::fs::OpenOptions::new();
    opciones.write(true).create_new(true);

    // El temporal nace sólo para su dueño —ver «Permisos» en el docstring—: si se
    // creara con el modo por defecto del proceso, el contenido estaría legible por
    // cualquiera durante el hueco entre la escritura y el `rename`.
    #[cfg(unix)]
    opciones.mode(0o600);

    let mut fichero = opciones
        .open(&temporal)
        .await
        .map_err(|causa| fallo_de_escritura(file_name, causa))?;

    if let Err(causa) = fichero.write_all(contenido.as_bytes()).await {
        drop(fichero);
        let _ = tokio::fs::remove_file(&temporal).await;

        return Err(fallo_de_escritura(file_name, causa));
    }

    if let Err(causa) = fichero.flush().await {
        drop(fichero);
        let _ = tokio::fs::remove_file(&temporal).await;

        return Err(fallo_de_escritura(file_name, causa));
    }

    // Los permisos del destino, si existía, se copian al temporal: el `rename` los
    // hereda del fichero temporal, así que es aquí —y no después— donde hay que
    // dejar dicho qué modo tendrá el fichero final.
    #[cfg(unix)]
    if let Ok(anterior) = tokio::fs::metadata(&destino).await
        && let Err(causa) = tokio::fs::set_permissions(&temporal, anterior.permissions()).await
    {
        drop(fichero);
        let _ = tokio::fs::remove_file(&temporal).await;

        return Err(fallo_de_escritura(file_name, causa));
    }

    // Al disco, no al búfer: el `rename` publica un contenido que ya está escrito
    // de verdad, no uno que el sistema de ficheros todavía tiene en memoria.
    if let Err(causa) = fichero.sync_all().await {
        drop(fichero);
        let _ = tokio::fs::remove_file(&temporal).await;

        return Err(fallo_de_escritura(file_name, causa));
    }

    // El descriptor tiene que estar cerrado antes del `rename` en Windows; en
    // Unix no importa, pero cerrarlo aquí deja el paso siguiente sin depender de
    // que el sistema de ficheros sea el de esta máquina.
    drop(fichero);

    if let Err(causa) = tokio::fs::rename(&temporal, &destino).await {
        let _ = tokio::fs::remove_file(&temporal).await;

        return Err(fallo_de_escritura(file_name, causa));
    }

    Ok(())
}

/// Lee el fichero del primer nivel de `dir`.
///
/// Es el otro extremo de [`escribir`]: la identidad es el nombre de fichero, así
/// que leer algo que no existe es un [`QuadletError::NotFound`] y **no** un fallo
/// de lectura. Servir un contenido truncado o vacío por un fichero que falta sería
/// decir que la quadlet está y no está a la vez.
///
/// Un directorio de quadlets que no está se ve igual: para la identidad del
/// recurso, en los dos casos no hay nada con ese nombre. Otro fallo del sistema de
/// ficheros —un permiso, un directorio donde se esperaba un fichero, un contenido
/// que no es UTF-8— es un [`QuadletError::ReadFailed`]: lo que hay no se pudo
/// leer, y eso es un `500` y no un `404`.
///
/// La lectura **no sigue enlaces simbólicos**, igual que no los siguen
/// [`escribir`], [`reescribir`] ni [`borrar`]: el nombre lo ocupa el enlace, no su
/// destino. Seguirlo sería leer un fichero que no está en el primer nivel —y, con
/// un enlace plantado en el directorio, uno de fuera: un `/etc/shadow` o el
/// `podpot.db` de al lado—. Por eso se mira con `symlink_metadata` y se exige que
/// lo que hay sea un **fichero regular**: un enlace —roto o no—, un directorio o
/// un fifo que ocupen ese nombre son `ReadFailed`, porque lo que hay no se puede
/// leer como una definición. Un fifo, además, no se puede ni abrir sin esperar a
/// que alguien escriba; y un enlace roto no es `NotFound`, que sería contradecir a
/// [`existe`], que sí lo ve.
pub async fn leer(dir: &Path, file_name: &str) -> Result<String, QuadletError> {
    let destino = dir.join(file_name);

    match tokio::fs::symlink_metadata(&destino).await {
        Ok(metadatos) => {
            let tipo = metadatos.file_type();

            if !tipo.is_file() {
                let que_es = if tipo.is_symlink() {
                    "un enlace simbólico"
                } else if tipo.is_dir() {
                    "un directorio"
                } else {
                    "algo que no es un fichero regular"
                };

                return Err(QuadletError::ReadFailed {
                    file_name: file_name.to_owned(),
                    causa: std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("el destino es {que_es}"),
                    ),
                });
            }
        }
        Err(causa) if causa.kind() == std::io::ErrorKind::NotFound => {
            return Err(QuadletError::NotFound {
                file_name: file_name.to_owned(),
            });
        }
        Err(causa) => {
            return Err(QuadletError::ReadFailed {
                file_name: file_name.to_owned(),
                causa,
            });
        }
    }

    // El contenido se lee del mismo nombre: lo que había era un fichero regular, y
    // el `open` de aquí no puede ser el de antes con otro tipo de fichero detrás
    // —si alguien cambió el nombre en medio, lo que se lee es lo que hay ahora—.
    match tokio::fs::read_to_string(&destino).await {
        Ok(contenido) => Ok(contenido),
        Err(causa) if causa.kind() == std::io::ErrorKind::NotFound => Err(QuadletError::NotFound {
            file_name: file_name.to_owned(),
        }),
        Err(causa) => Err(QuadletError::ReadFailed {
            file_name: file_name.to_owned(),
            causa,
        }),
    }
}

/// Borra el fichero del primer nivel de `dir`.
///
/// La identidad es el nombre de fichero, así que borrar algo que no existe es un
/// [`QuadletError::NotFound`] y **no** un borrado sin más: el directorio queda
/// intacto y quien llama puede distinguir «lo borré» de «no había nada», que es
/// lo que la API necesita para contestar un `404`. Otro fallo del sistema de
/// ficheros —permisos, por ejemplo— es un [`QuadletError::WriteFailed`].
pub async fn borrar(dir: &Path, file_name: &str) -> Result<(), QuadletError> {
    match tokio::fs::remove_file(dir.join(file_name)).await {
        Ok(()) => Ok(()),
        Err(causa) if causa.kind() == std::io::ErrorKind::NotFound => Err(QuadletError::NotFound {
            file_name: file_name.to_owned(),
        }),
        Err(causa) => Err(fallo_de_escritura(file_name, causa)),
    }
}

/// Un nombre de fichero temporal, único, para el directorio de quadlets.
///
/// Lleva el `pid` y un contador para que dos procesos —o dos peticiones del mismo
/// proceso— no elijan el mismo nombre: no hay aleatoriedad que valga cuando el
/// sistema de ficheros ya sabe decir «ocupado» con `create_new`. Empieza por `.`
/// —un fichero oculto— y termina en `.tmp`, así que **no** tiene extensión de
/// quadlet ni un stem admisible: el descubrimiento nunca lo tomaría por una.
fn nombre_temporal(file_name: &str) -> String {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);

    format!(".{file_name}.{}.{n}.tmp", std::process::id())
}

/// El fallo al abrir el destino para escribirlo.
///
/// `AlreadyExists` es el nombre que el sistema de ficheros dice que ya estaba
/// ocupado: quien pierde la carrera de crear el mismo fichero recibe un
/// **duplicado** —un `409`—, no un fallo de escritura. Cualquier otro fallo de
/// apertura sí es de escritura.
fn fallo_de_apertura(file_name: &str, causa: std::io::Error) -> QuadletError {
    if causa.kind() == std::io::ErrorKind::AlreadyExists {
        return QuadletError::Duplicate {
            file_name: file_name.to_owned(),
        };
    }

    fallo_de_escritura(file_name, causa)
}

/// El fallo de escritura con el nombre del fichero ya puesto.
///
/// Se aísla aquí porque los tres sitios que pueden fallar —abrir, volcar y
/// vaciar— construyen el mismo error: nombre de fichero, nunca la ruta.
fn fallo_de_escritura(file_name: &str, causa: std::io::Error) -> QuadletError {
    QuadletError::WriteFailed {
        file_name: file_name.to_owned(),
        causa,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::quadlet::QuadletError;

    use super::{borrar, escribir, existe, leer, reescribir};

    #[tokio::test]
    async fn escribe_en_el_primer_nivel_del_directorio() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");

        escribir(dir.path(), "web.container", "[Container]\nImage=nginx\n")
            .await
            .expect("la escritura tiene que salir bien");

        let contenido = std::fs::read_to_string(dir.path().join("web.container"))
            .expect("el fichero tiene que estar en el primer nivel");

        assert_eq!(contenido, "[Container]\nImage=nginx\n");
    }

    /// Escenario «Permisos» (creación): el fichero que nace de [`escribir`] sale
    /// con permiso **sólo para su dueño**. Un contenedor recién creado puede
    /// llevar `Environment=…PASSWORD=…`, así que el fichero no puede nacer legible
    /// por cualquiera —el modo por defecto del proceso lo haría—.
    #[cfg(unix)]
    #[tokio::test]
    async fn escribir_nace_con_permisos_solo_para_el_dueno() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");

        escribir(dir.path(), "web.container", "[Container]\nImage=nginx\n")
            .await
            .expect("la escritura tiene que salir bien");

        assert_eq!(modo(&dir.path().join("web.container")), 0o600);
    }

    /// Escenario «Fichero existente»: el destino no se toca, y la negativa es un
    /// **duplicado** y no un fallo de escritura.
    ///
    /// Es lo que hace correcto el camino de carrera: dos creaciones del mismo
    /// nombre pueden pasar las dos la precomprobación `existe`, y la que pierde
    /// `create_new` recibe `AlreadyExists`; tratarlo como `WriteFailed` daría un
    /// `500` por lo que en realidad es un `409`.
    #[tokio::test]
    async fn no_pisa_un_fichero_que_ya_existe() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let destino = dir.path().join("web.container");
        std::fs::write(&destino, "lo de antes").expect("el fichero previo se escribe");

        let error = escribir(dir.path(), "web.container", "lo de ahora")
            .await
            .expect_err("no puede pisar un fichero que ya está");

        assert!(
            matches!(error, QuadletError::Duplicate { .. }),
            "un nombre ya ocupado es un duplicado, no un fallo de escritura: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(&destino).expect("el original sigue ahí"),
            "lo de antes",
            "el fichero que ya existía se pisó"
        );
    }

    /// Un directorio ausente es un fallo de escritura, y con el nombre del
    /// fichero en el error: quien pregunta no ve la ruta, pero el operador sí.
    #[tokio::test]
    async fn un_directorio_ausente_es_un_fallo_de_escritura() {
        let error = escribir(Path::new("/no/existe/podpot"), "web.container", "x")
            .await
            .expect_err("no se puede escribir en un directorio que no existe");

        match error {
            QuadletError::WriteFailed { file_name, .. } => {
                assert_eq!(file_name, "web.container");
            }
            otro => panic!("se esperaba un fallo de escritura: {otro}"),
        }
    }

    #[tokio::test]
    async fn existe_dice_lo_que_hay() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");

        assert!(
            !existe(dir.path(), "web.container")
                .await
                .expect("se puede mirar el directorio"),
            "en un directorio vacío no hay nada"
        );

        std::fs::write(dir.path().join("web.container"), "").expect("se escribe un fichero");

        assert!(
            existe(dir.path(), "web.container")
                .await
                .expect("se puede mirar el directorio"),
            "el fichero que se acaba de escribir tiene que verse"
        );
    }

    /// Los nombres de las entradas del primer nivel del directorio, ordenados:
    /// lo que hace falta para comprobar que no sobra un temporal.
    fn entradas(dir: &Path) -> Vec<String> {
        let mut nombres: Vec<String> = std::fs::read_dir(dir)
            .expect("el directorio se puede leer")
            .map(|entrada| {
                entrada
                    .expect("la entrada se puede leer")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        nombres.sort();
        nombres
    }

    /// Escenario «Edición correcta» (escritura): la reescritura reemplaza el
    /// contenido y **no** deja ningún fichero temporal en el directorio.
    #[tokio::test]
    async fn reescribe_el_fichero_sin_dejar_temporales() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let destino = dir.path().join("web.container");
        std::fs::write(&destino, "lo de antes").expect("el original se escribe");

        reescribir(dir.path(), "web.container", "[Container]\nImage=nginx\n")
            .await
            .expect("la reescritura tiene que salir bien");

        assert_eq!(
            std::fs::read_to_string(&destino).expect("el fichero tiene que estar"),
            "[Container]\nImage=nginx\n",
            "el contenido tiene que ser el nuevo"
        );
        assert_eq!(
            entradas(dir.path()),
            ["web.container"],
            "no puede quedar ningún fichero temporal"
        );
    }

    /// La reescritura crea el fichero si no existía: no es una creación «sin
    /// pisar» como [`escribir`], es un reemplazo. El destino que no estaba queda
    /// con el contenido nuevo y sin temporales.
    #[tokio::test]
    async fn reescribe_crea_el_fichero_si_no_estaba() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");

        reescribir(dir.path(), "web.container", "[Container]\nImage=nginx\n")
            .await
            .expect("la reescritura tiene que salir bien");

        assert_eq!(
            std::fs::read_to_string(dir.path().join("web.container")).expect("el fichero está"),
            "[Container]\nImage=nginx\n"
        );
        assert_eq!(entradas(dir.path()), ["web.container"]);
    }

    /// El modo de un fichero, sin los bits de tipo.
    #[cfg(unix)]
    fn modo(ruta: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;

        std::fs::metadata(ruta)
            .expect("el fichero tiene que existir")
            .permissions()
            .mode()
            & 0o777
    }

    /// Escenario «Permisos»: reescribir **no degrada** el fichero. Una quadlet que
    /// puede llevar `Environment=…PASSWORD=…` escrita para su dueño (`0600`) tiene
    /// que seguir siendo de su dueño después de editarla: el reemplazo es por un
    /// fichero que podpot crea, y con el modo por defecto del proceso el `rename`
    /// la dejaría legible por cualquiera.
    #[cfg(unix)]
    #[tokio::test]
    async fn reescribir_no_degrada_los_permisos_del_fichero() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let destino = dir.path().join("web.container");
        std::fs::write(&destino, "Environment=PASSWORD=secreta\n").expect("el original se escribe");
        std::fs::set_permissions(&destino, std::fs::Permissions::from_mode(0o600))
            .expect("los permisos del original se fijan");

        reescribir(dir.path(), "web.container", "[Container]\nImage=nginx\n")
            .await
            .expect("la reescritura tiene que salir bien");

        assert_eq!(
            modo(&destino),
            0o600,
            "la edición degradó los permisos del fichero"
        );
    }

    /// Y un fichero que nace de la reescritura —el destino no existía— nace con
    /// permiso **sólo para su dueño**: es el modo más prudente para un fichero que
    /// puede llevar secretos, y el temporal nunca pasa por otro.
    #[cfg(unix)]
    #[tokio::test]
    async fn reescribir_nace_con_permisos_solo_para_el_dueno() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");

        reescribir(dir.path(), "web.container", "[Container]\nImage=nginx\n")
            .await
            .expect("la reescritura tiene que salir bien");

        assert_eq!(modo(&dir.path().join("web.container")), 0o600);
    }

    /// Escenario «Fallo de escritura»: si el último paso —el `rename`— no puede
    /// completarse, el destino queda **intacto** y no sobra el temporal.
    ///
    /// Se fuerza de forma determinista —e independiente del uid— haciendo que el
    /// destino sea un directorio: `rename` de un fichero sobre un directorio falla
    /// con `EISDIR`, así que el fallo ocurre **después** de haber escrito el
    /// temporal, que es justo el estado que hay que comprobar.
    #[tokio::test]
    async fn una_escritura_que_no_se_puede_completar_no_corrompe_el_destino() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let destino = dir.path().join("web.container");
        std::fs::create_dir(&destino).expect("el destino de prueba es un directorio");
        std::fs::write(destino.join("dentro"), "lo de antes").expect("se escribe lo de dentro");

        let error = reescribir(dir.path(), "web.container", "lo de ahora")
            .await
            .expect_err("no se puede reemplazar un directorio con un fichero");

        assert!(
            matches!(error, QuadletError::WriteFailed { .. }),
            "se esperaba un fallo de escritura: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(destino.join("dentro")).expect("el destino sigue ahí"),
            "lo de antes",
            "el destino se tocó pese a que la escritura falló"
        );
        assert_eq!(
            entradas(dir.path()),
            ["web.container"],
            "quedó un temporal tras un fallo de escritura"
        );
    }

    /// Un directorio ausente es un fallo de escritura también al reescribir.
    #[tokio::test]
    async fn un_directorio_ausente_es_un_fallo_de_escritura_al_reescribir() {
        let error = reescribir(Path::new("/no/existe/podpot"), "web.container", "x")
            .await
            .expect_err("no se puede reescribir en un directorio que no existe");

        match error {
            QuadletError::WriteFailed { file_name, .. } => {
                assert_eq!(file_name, "web.container");
            }
            otro => panic!("se esperaba un fallo de escritura: {otro}"),
        }
    }

    /// Escenario «Borrado correcto» (escritura): el fichero desaparece del primer
    /// nivel del directorio.
    #[tokio::test]
    async fn borra_el_fichero_del_primer_nivel() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        std::fs::write(dir.path().join("web.container"), "[Container]\n")
            .expect("el fichero previo se escribe");

        borrar(dir.path(), "web.container")
            .await
            .expect("borrar un fichero que existe tiene que salir bien");

        assert_eq!(
            entradas(dir.path()),
            Vec::<String>::new(),
            "el fichero tenía que desaparecer y no queda nada más"
        );
    }

    /// Escenario «Borrado de algo inexistente»: `NotFound` y **sin efectos** —el
    /// directorio queda exactamente como estaba—.
    #[tokio::test]
    async fn borrar_algo_que_no_existe_es_not_found_sin_efectos() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        std::fs::write(dir.path().join("otra.container"), "lo de otra")
            .expect("el otro fichero se escribe");

        let error = borrar(dir.path(), "web.container")
            .await
            .expect_err("no se puede borrar lo que no existe");

        match error {
            QuadletError::NotFound { file_name } => assert_eq!(file_name, "web.container"),
            otro => panic!("se esperaba «no existe»: {otro}"),
        }
        assert_eq!(
            entradas(dir.path()),
            ["otra.container"],
            "el directorio quedó tocado pese a que el borrado falló"
        );
    }

    /// Escenario «Lectura de una quadlet» (disco): el contenido vuelve tal cual y
    /// el fichero **no** se toca —leer no escribe—.
    #[tokio::test]
    async fn lee_el_contenido_del_fichero() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let contenido = "[Container]\nImage=nginx\n";
        std::fs::write(dir.path().join("web.container"), contenido)
            .expect("el fichero de prueba se escribe");

        assert_eq!(
            leer(dir.path(), "web.container")
                .await
                .expect("el fichero tiene que leerse"),
            contenido
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("web.container"))
                .expect("el fichero sigue ahí"),
            contenido,
            "leer no puede modificar el fichero"
        );
    }

    /// Escenario «Quadlet inexistente»: `NotFound`, y el directorio queda intacto.
    #[tokio::test]
    async fn leer_algo_que_no_existe_es_not_found() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        std::fs::write(dir.path().join("otra.container"), "lo de otra")
            .expect("el otro fichero se escribe");

        let error = leer(dir.path(), "web.container")
            .await
            .expect_err("no se puede leer lo que no existe");

        match error {
            QuadletError::NotFound { file_name } => assert_eq!(file_name, "web.container"),
            otro => panic!("se esperaba «no existe»: {otro}"),
        }
        assert_eq!(entradas(dir.path()), ["otra.container"]);
    }

    /// Un directorio de quadlets que no está se ve como cualquier otro recurso que
    /// no existe: `NotFound`. La lectura no distingue «falta el directorio» de
    /// «falta el fichero», porque para la identidad del recurso en los dos casos no
    /// hay nada con ese nombre.
    #[tokio::test]
    async fn un_directorio_ausente_es_not_found_al_leer() {
        let error = leer(Path::new("/no/existe/podpot"), "web.container")
            .await
            .expect_err("no se puede leer en un directorio que no existe");

        assert!(
            matches!(error, QuadletError::NotFound { .. }),
            "un directorio ausente es «no existe» para el recurso: {error}"
        );
    }

    /// Un fallo de lectura de verdad —un directorio donde se esperaba un fichero—
    /// es `ReadFailed` y **no** `NotFound`: lo que hay no se pudo leer.
    #[tokio::test]
    async fn un_directorio_donde_se_espera_un_fichero_es_un_fallo_de_lectura() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        std::fs::create_dir(dir.path().join("web.container")).expect("el destino es un directorio");

        let error = leer(dir.path(), "web.container")
            .await
            .expect_err("un directorio no se puede leer como una quadlet");

        assert!(
            matches!(error, QuadletError::ReadFailed { .. }),
            "se esperaba un fallo de lectura: {error}"
        );
    }

    /// Y un fichero que no es texto —una quadlet es UTF-8— también es `ReadFailed`.
    #[tokio::test]
    async fn un_fichero_no_textual_es_un_fallo_de_lectura() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        std::fs::write(dir.path().join("web.container"), [0xff, 0xfe, 0x00])
            .expect("el fichero binario se escribe");

        let error = leer(dir.path(), "web.container")
            .await
            .expect_err("un fichero binario no es una quadlet");

        assert!(
            matches!(error, QuadletError::ReadFailed { .. }),
            "se esperaba un fallo de lectura: {error}"
        );
    }

    /// Escenario «Enlace simbólico»: leer **no** sigue enlaces. La escritura, la
    /// reescritura y el borrado no los siguen —el nombre lo ocupa el enlace, no su
    /// destino—, y la lectura tiene que medir lo mismo: seguirle el rastro sería
    /// leer un fichero que no está en el primer nivel, quizá uno de fuera del
    /// directorio de quadlets.
    #[cfg(unix)]
    #[tokio::test]
    async fn leer_no_sigue_un_enlace_simbolico() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let fuera = dir.path().join("fuera");
        std::fs::create_dir(&fuera).expect("el directorio de fuera se crea");
        std::fs::write(fuera.join("ajena.container"), "[Container]\nImage=ajena\n")
            .expect("el fichero de fuera se escribe");
        std::os::unix::fs::symlink(
            fuera.join("ajena.container"),
            dir.path().join("web.container"),
        )
        .expect("el enlace se crea");

        let error = leer(dir.path(), "web.container")
            .await
            .expect_err("la lectura no puede seguir un enlace");

        assert!(
            matches!(error, QuadletError::ReadFailed { .. }),
            "se esperaba un fallo de lectura: {error}"
        );
    }

    /// Y un enlace **roto** tampoco es `NotFound`: el nombre está ocupado —es lo
    /// mismo que mira [`existe`]—, así que lo que hay no se pudo leer, y decir «no
    /// existe» sería contradecir a la escritura, que sí lo vería.
    #[cfg(unix)]
    #[tokio::test]
    async fn un_enlace_roto_es_un_fallo_de_lectura() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        std::os::unix::fs::symlink(
            dir.path().join("no-esta.container"),
            dir.path().join("web.container"),
        )
        .expect("el enlace roto se crea");

        let error = leer(dir.path(), "web.container")
            .await
            .expect_err("un enlace roto no se puede leer");

        assert!(
            matches!(error, QuadletError::ReadFailed { .. }),
            "se esperaba un fallo de lectura: {error}"
        );
    }
}
