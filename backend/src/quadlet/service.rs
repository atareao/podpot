//! La orquestación de la creación: escribir, recargar y registrar.
//!
//! Es el único sitio donde los cuatro pasos se ponen en fila, y el orden no es
//! negociable:
//!
//! 1. **validar** el nombre y el tipo (sin tocar el disco),
//! 2. comprobar la **unicidad** del nombre en el directorio,
//! 3. **escribir** el fichero (sin pisar),
//! 4. **recargar** systemd por D-Bus, que es lo que hace visible la quadlet,
//! 5. **registrar** la creación en la persistencia.
//!
//! El registro va **el último** a propósito: el requisito es que nunca quede
//! registrada una creación que no ocurrió, así que sólo se escribe cuando el
//! fichero está en disco y systemd lo ha recargado. Un fallo en cualquiera de los
//! pasos anteriores deja la tabla como estaba.
//!
//! Si systemd contesta que no —o no contesta— después de que el fichero esté
//! escrito, la creación se da por fallida y **no** se registra, pero el fichero
//! se queda en disco: borrarlo sería deshacer una escritura que sí ocurrió, y un
//! `daemon-reload` posterior lo haría visible igualmente. El fallo se registra en
//! el log para que quede la traza de por qué hay un fichero sin apunte.

use std::path::Path;

use sqlx::SqlitePool;

use crate::systemd::SystemdClient;

use super::{Quadlet, QuadletDraft, QuadletError, store};

/// Crea una quadlet y la deja visible para systemd.
///
/// `dir` es el directorio canónico ([`crate::config::Config::quadlet_dir`]) y
/// `systemd` el cliente del bus de usuario: las dos dependencias entran por
/// parámetro —y no se leen de dentro— para que las pruebas las fijen sin bus y
/// sin tocar el home del usuario.
pub async fn crear(
    dir: &Path,
    systemd: &dyn SystemdClient,
    pool: &SqlitePool,
    draft: QuadletDraft,
) -> Result<Quadlet, QuadletError> {
    // 1. Validar sin tocar el disco: nombre, tipo y unidad que le toca.
    let quadlet = draft.quadlet()?;
    // 2. Derivar el contenido de los campos tipados, nunca de un cuerpo.
    let contenido = draft.render()?;
    // 3. Unicidad antes de escribir: el destino no se toca si el nombre está en uso.
    if store::existe(dir, &quadlet.file_name).await? {
        return Err(QuadletError::Duplicate {
            file_name: quadlet.file_name,
        });
    }
    // 4. Escribir sin pisar. La vuelta a comprobar la da `create_new`.
    store::escribir(dir, &quadlet.file_name, &contenido).await?;
    // 5. Hacerla visible: sin recarga, la unidad de esta quadlet no existe. Si la
    //    recarga falla, el fichero se queda en disco: se deja la traza de qué
    //    fichero quedó huérfano para que el operador sepa por qué hay una quadlet
    //    sin apunte en el registro.
    if let Err(causa) = systemd.reload().await {
        tracing::warn!(
            fichero = %quadlet.file_name,
            error = %causa,
            "la quadlet quedó escrita pero systemd no recargó: hay un fichero sin registrar"
        );

        return Err(QuadletError::Systemd(causa));
    }
    // 6. Registrar. Si el fichero está en disco y systemd recargó, la creación
    //    ocurrió; si el registro falla, la petición falla, pero el hecho ocurrido
    //    no se deshace.
    registrar(pool, &quadlet).await?;

    Ok(quadlet)
}

/// Anota la creación en la tabla de registro.
async fn registrar(pool: &SqlitePool, quadlet: &Quadlet) -> Result<(), QuadletError> {
    sqlx::query("INSERT INTO quadlet_creations (file_name, unit_name, kind) VALUES (?, ?, ?)")
        .bind(&quadlet.file_name)
        .bind(quadlet.unit_name.as_str())
        .bind(quadlet.kind.extension())
        .execute(pool)
        .await
        .map_err(|causa| QuadletError::Persistencia {
            file_name: quadlet.file_name.clone(),
            causa: Box::new(causa),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sqlx::SqlitePool;

    use crate::db;
    use crate::quadlet::{QuadletDraft, QuadletError};
    use crate::systemd::SystemdError;
    use crate::systemd::fake::FakeSystemd;
    use crate::test_support::DobleQueFalla;

    use super::crear;

    /// Una base temporal ya migrada.
    async fn base() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let dsn = format!("sqlite://{}/podpot.db", dir.path().display());
        let pool = db::preparar(&dsn)
            .await
            .expect("la base temporal tiene que abrirse y migrarse");

        (dir, pool)
    }

    /// Un contenedor con lo mínimo.
    fn contenedor(name: &str) -> QuadletDraft {
        QuadletDraft {
            name: name.to_owned(),
            kind: "container".to_owned(),
            image: Some("docker.io/library/nginx:latest".to_owned()),
            ..QuadletDraft::default()
        }
    }

    /// Cuántas creaciones hay registradas.
    async fn registradas(pool: &SqlitePool) -> i64 {
        let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM quadlet_creations")
            .fetch_one(pool)
            .await
            .expect("la tabla de registro tiene que existir");

        total
    }

    /// Escenario «Escritura correcta» + «Recarga tras crear» + «Creación
    /// registrada»: el fichero queda escrito, systemd recarga y la creación
    /// queda apuntada.
    #[tokio::test]
    async fn crea_el_fichero_recarga_systemd_y_registra() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        let quadlet = crear(dir.path(), systemd.as_ref(), &pool, contenedor("web"))
            .await
            .expect("la creación tiene que salir bien");

        assert_eq!(quadlet.file_name, "web.container");
        assert_eq!(quadlet.unit_name.as_str(), "web.service");

        let contenido = std::fs::read_to_string(dir.path().join("web.container"))
            .expect("el fichero tiene que estar escrito");
        assert!(
            contenido.contains("Image=docker.io/library/nginx:latest"),
            "el fichero no lleva la imagen: {contenido}"
        );

        assert_eq!(systemd.recargas(), 1, "systemd tiene que recargar una vez");
        assert_eq!(registradas(&pool).await, 1, "la creación se registra");
    }

    /// Escenario «Nombre ya en uso»: la validación de unicidad va **antes** de
    /// escribir, así que el destino queda intacto, no se recarga y no se registra.
    #[tokio::test]
    async fn un_nombre_que_ya_esta_no_escribe_ni_recarga_ni_registra() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let destino = dir.path().join("web.container");
        std::fs::write(&destino, "lo de antes").expect("el fichero previo se escribe");

        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        let error = crear(dir.path(), systemd.as_ref(), &pool, contenedor("web"))
            .await
            .expect_err("un nombre ya en uso no puede crear nada");

        assert!(
            matches!(error, QuadletError::Duplicate { .. }),
            "se esperaba un duplicado: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(&destino).expect("el original sigue ahí"),
            "lo de antes",
            "se pisó el fichero que ya existía"
        );
        assert_eq!(
            systemd.recargas(),
            0,
            "no se recarga si no se escribió nada"
        );
        assert_eq!(
            registradas(&pool).await,
            0,
            "no se registra lo que no ocurrió"
        );
    }

    /// Un nombre inválido se rechaza antes de tocar el disco.
    #[tokio::test]
    async fn un_nombre_invalido_no_escribe_nada() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        let error = crear(dir.path(), systemd.as_ref(), &pool, contenedor("web malo"))
            .await
            .expect_err("un nombre inválido no puede crear nada");

        assert!(
            matches!(error, QuadletError::InvalidName { .. }),
            "se esperaba un nombre inválido: {error}"
        );
        assert_eq!(
            std::fs::read_dir(dir.path())
                .expect("el directorio se puede leer")
                .count(),
            0,
            "el directorio tiene que quedar vacío"
        );
        assert_eq!(registradas(&pool).await, 0);
    }

    /// Escenario «Creación fallida no registrada»: una recarga que falla deja la
    /// creación incompleta, así que no se apunta.
    #[tokio::test]
    async fn una_recarga_que_falla_no_registra_la_creacion() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let (_base, pool) = base().await;
        let systemd = DobleQueFalla::con(|| SystemdError::ReloadFailed {
            causa: Box::new(std::io::Error::other("el gestor dijo que no")),
        });

        let error = crear(dir.path(), &systemd, &pool, contenedor("web"))
            .await
            .expect_err("una recarga fallida no puede dar la creación por buena");

        assert!(
            matches!(
                error,
                QuadletError::Systemd(SystemdError::ReloadFailed { .. })
            ),
            "se esperaba un fallo de recarga: {error}"
        );
        assert_eq!(
            registradas(&pool).await,
            0,
            "no puede quedar registrada una creación que no se completó"
        );
    }
}
