//! La escritura de quadlets en el directorio canónico.
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

use std::path::Path;

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
pub async fn escribir(dir: &Path, file_name: &str, contenido: &str) -> Result<(), QuadletError> {
    let destino = dir.join(file_name);

    // `create_new` es lo que hace que la negativa a pisar no sea una carrera: un
    // `stat` que dijera «no hay nada» podría quedar obsoleto antes del `open`, y
    // el sistema de ficheros es quien mejor sabe si el nombre estaba libre. Si
    // el nombre lo ocupó otra creación entre la precomprobación y aquí, el
    // `AlreadyExists` se traduce a un duplicado —un `409`— y no a un fallo de
    // escritura —un `500`—.
    let mut fichero = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
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

    use super::{escribir, existe};

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
}
