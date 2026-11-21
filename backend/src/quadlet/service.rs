//! La orquestación de las operaciones sobre quadlets: leer, crear, editar y borrar.
//!
//! Es el único sitio donde los pasos de la creación se ponen en fila, y el orden
//! no es negociable:
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

/// Edita una quadlet existente: reescribe su fichero y recarga systemd.
///
/// El orden sigue el de la creación —validar, escribir, recargar— con dos
/// diferencias que son la esencia de la edición:
///
/// 1. **La identidad la fija el recurso**, no el cuerpo: se resuelve del nombre
///    de fichero `recurso` ([`Quadlet::from_file_name`]) y el borrador tiene que
///    describir exactamente ese recurso ([`QuadletDraft::describe_a`]); editar
///    nunca renombra.
/// 2. **La existencia se comprueba antes de renderizar**: editar una quadlet que
///    no existe es un [`QuadletError::NotFound`] y el disco no se toca.
///
/// Entre esa comprobación y el `rename` de [`store::reescribir`] cabe un borrado
/// concurrente —la carrera se acepta y está razonada allí—: si ocurre, el fichero
/// vuelve a estar, con el contenido nuevo, y la edición se da por buena. Es un
/// solape de dos peticiones sobre el mismo nombre, no una edición sobre algo que
/// no existe: la petición que llega aquí sí lo encontró.
///
/// La reescritura es atómica ([`store::reescribir`]) y el contenido sale siempre
/// del struct tipado ([`QuadletDraft::render`]), nunca de un cuerpo: un valor con
/// estructura se rechaza con [`QuadletError::InvalidValue`] **antes** de escribir
/// nada, así que el fichero original queda intacto.
pub async fn editar(
    dir: &Path,
    systemd: &dyn SystemdClient,
    pool: &SqlitePool,
    recurso: &str,
    draft: QuadletDraft,
) -> Result<Quadlet, QuadletError> {
    // 1. La identidad la fija el recurso: su nombre de fichero resuelve tipo y
    //    unidad. Un nombre que no vale se rechaza antes de tocar el disco.
    let objetivo = Quadlet::from_file_name(recurso)?;
    // 2. El borrador tiene que ser del tipo creable y describir **este** recurso:
    //    un intento de renombrar (o de cambiar el tipo) es `InvalidName`.
    draft.validate()?;
    draft.describe_a(&objetivo.file_name)?;
    // 3. La quadlet tiene que existir; si no, `NotFound` y el disco intacto.
    if !store::existe(dir, &objetivo.file_name).await? {
        return Err(QuadletError::NotFound {
            file_name: objetivo.file_name,
        });
    }
    // 4. Derivar el contenido de los campos tipados. La sanitización ocurre aquí,
    //    así que un valor con estructura no llega a tocar el fichero.
    let contenido = draft.render()?;
    // 5. Reescribir de forma atómica: el original sólo cambia cuando el temporal
    //    está entero.
    store::reescribir(dir, &objetivo.file_name, &contenido).await?;
    // 6. Hacer visible el cambio. Si la recarga falla, el fichero queda reescrito
    //    —deshacerlo sería deshacer una escritura que sí ocurrió— y se deja la
    //    traza para que el operador sepa que hay que recargar a mano.
    if let Err(causa) = systemd.reload().await {
        tracing::warn!(
            fichero = %objetivo.file_name,
            error = %causa,
            "la quadlet quedó reescrita pero systemd no recargó: hay que recargar a mano"
        );

        return Err(QuadletError::Systemd(causa));
    }
    // 7. Registrar. Va el último: sólo se apunta una operación que llegó al final,
    //    así que un fallo anterior deja la tabla como estaba.
    registrar_operacion(pool, "edit", &objetivo.file_name).await?;

    Ok(objetivo)
}

/// Lee una quadlet existente: devuelve su tipo y sus campos.
///
/// Es la operación más simple de las tres y la única que **no** toca systemd ni la
/// persistencia: leer no cambia nada, así que no hay recarga ni registro. La
/// identidad se resuelve como en la edición ([`Quadlet::from_file_name`]) —con su
/// misma validación de nombre y tipo—, el fichero se lee del directorio canónico y
/// el contenido se parsea de vuelta al struct tipado
/// ([`QuadletDraft::parse`]). Que el fichero exista lo decide la lectura: si no
/// está, es [`QuadletError::NotFound`].
pub async fn leer(dir: &Path, recurso: &str) -> Result<QuadletDraft, QuadletError> {
    let objetivo = Quadlet::from_file_name(recurso)?;
    let contenido = store::leer(dir, &objetivo.file_name).await?;

    Ok(QuadletDraft::parse(&objetivo, &contenido))
}

/// Borra una quadlet existente: elimina su fichero y recarga systemd.
///
/// La identidad es el nombre de fichero `recurso`. El borrado del fichero es lo
/// que decide si había algo: si no existía, [`store::borrar`] devuelve
/// [`QuadletError::NotFound`] y **no** se recarga. Si existía, se elimina y se
/// pide `daemon-reload` para que el gestor olvide la unidad generada.
///
/// No se para la unidad antes de borrarla: eso es una acción de control sobre la
/// unidad ([`crate::systemd::UnitAction`]) y es responsabilidad de quien la pide;
/// el borrado del fichero se limita a lo que el fichero representa.
pub async fn borrar(
    dir: &Path,
    systemd: &dyn SystemdClient,
    pool: &SqlitePool,
    recurso: &str,
) -> Result<Quadlet, QuadletError> {
    let objetivo = Quadlet::from_file_name(recurso)?;

    store::borrar(dir, &objetivo.file_name).await?;

    if let Err(causa) = systemd.reload().await {
        tracing::warn!(
            fichero = %objetivo.file_name,
            error = %causa,
            "la quadlet se borró pero systemd no recargó: hay que recargar a mano"
        );

        return Err(QuadletError::Systemd(causa));
    }

    registrar_operacion(pool, "delete", &objetivo.file_name).await?;

    Ok(objetivo)
}

/// Anota una operación sobre una quadlet existente en la tabla de registro.
///
/// `operacion` es el texto del tipo (`edit` o `delete`) y lo fija quien llama: son
/// dos constantes y no un valor que venga de fuera, así que la tabla nunca recibe
/// una operación que no esté declarada en su `CHECK`.
async fn registrar_operacion(
    pool: &SqlitePool,
    operacion: &str,
    file_name: &str,
) -> Result<(), QuadletError> {
    sqlx::query("INSERT INTO quadlet_operations (operation, file_name) VALUES (?, ?)")
        .bind(operacion)
        .bind(file_name)
        .execute(pool)
        .await
        .map_err(|causa| QuadletError::Persistencia {
            file_name: file_name.to_owned(),
            causa: Box::new(causa),
        })?;

    Ok(())
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

    use super::{borrar, crear, editar, leer};

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

    /// Un fichero de quadlet ya escrito, con el contenido que se le dé.
    fn fichero(dir: &std::path::Path, nombre: &str, contenido: &str) {
        std::fs::write(dir.join(nombre), contenido).expect("el fichero de prueba se escribe");
    }

    /// Escenario «Edición correcta»: el fichero se reescribe desde el struct
    /// tipado —reemplazando lo que hubiera— y systemd recarga.
    #[tokio::test]
    async fn edita_el_fichero_y_recarga_systemd() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        fichero(dir.path(), "web.container", "[Container]\nImage=vieja\n");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        let quadlet = editar(
            dir.path(),
            systemd.as_ref(),
            &pool,
            "web.container",
            contenedor("web"),
        )
        .await
        .expect("la edición tiene que salir bien");

        assert_eq!(quadlet.file_name, "web.container");
        assert_eq!(quadlet.unit_name.as_str(), "web.service");

        assert_eq!(
            std::fs::read_to_string(dir.path().join("web.container"))
                .expect("el fichero tiene que estar"),
            "[Container]\nImage=docker.io/library/nginx:latest\n",
            "el contenido tiene que salir del struct tipado, reemplazando el de antes"
        );
        assert_eq!(systemd.recargas(), 1, "systemd tiene que recargar una vez");
    }

    /// Escenario «Quadlet inexistente»: `NotFound`, el directorio queda intacto y
    /// **no** se recarga.
    #[tokio::test]
    async fn editar_algo_que_no_existe_es_not_found_y_no_recarga() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        let error = editar(
            dir.path(),
            systemd.as_ref(),
            &pool,
            "web.container",
            contenedor("web"),
        )
        .await
        .expect_err("no se puede editar una quadlet que no existe");

        assert!(
            matches!(error, QuadletError::NotFound { .. }),
            "se esperaba «no existe»: {error}"
        );
        assert_eq!(
            std::fs::read_dir(dir.path())
                .expect("el directorio se puede leer")
                .count(),
            0,
            "el directorio tiene que quedar intacto"
        );
        assert_eq!(
            systemd.recargas(),
            0,
            "no se recarga si no se escribió nada"
        );
    }

    /// Escenario «Intento de renombrado»: editar nunca renombra. `InvalidName`, el
    /// fichero original intacto y sin recargar.
    #[tokio::test]
    async fn editar_con_otro_nombre_es_invalid_name_y_no_toca_nada() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        fichero(dir.path(), "web.container", "lo de antes");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        // El recurso es `web.container`, pero el borrador pretente otro stem.
        let error = editar(
            dir.path(),
            systemd.as_ref(),
            &pool,
            "web.container",
            contenedor("otro"),
        )
        .await
        .expect_err("renombrar no está permitido");

        assert!(
            matches!(error, QuadletError::InvalidName { .. }),
            "se esperaba un nombre inválido: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("web.container"))
                .expect("el original sigue ahí"),
            "lo de antes",
            "un intento de renombrado no puede tocar el fichero"
        );
        assert_eq!(systemd.recargas(), 0);
    }

    /// Escenario «Valor con caracteres prohibidos»: `InvalidValue` y el fichero
    /// original **intacto** —la validación va antes de tocar el disco—.
    #[tokio::test]
    async fn editar_con_un_valor_invalido_es_invalid_value_y_no_toca_el_fichero() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        fichero(dir.path(), "web.container", "lo de antes");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        let inyectado = QuadletDraft {
            image: Some("nginx\nUser=root".to_owned()),
            ..contenedor("web")
        };

        let error = editar(
            dir.path(),
            systemd.as_ref(),
            &pool,
            "web.container",
            inyectado,
        )
        .await
        .expect_err("un valor con salto de línea no puede colarse");

        assert!(
            matches!(error, QuadletError::InvalidValue { field: "image" }),
            "se esperaba un valor inválido: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("web.container"))
                .expect("el original sigue ahí"),
            "lo de antes",
            "un valor inválido no puede modificar el fichero"
        );
        assert_eq!(systemd.recargas(), 0);
    }

    /// Escenario «Error de recarga»: la edición completa pero `daemon-reload`
    /// falla, así que la operación se da por fallida —con el fichero ya
    /// reescrito— y no se registra.
    #[tokio::test]
    async fn una_recarga_que_falla_da_la_edicion_por_fallida() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        fichero(dir.path(), "web.container", "lo de antes");
        let (_base, pool) = base().await;
        let systemd = DobleQueFalla::con(|| SystemdError::ReloadFailed {
            causa: Box::new(std::io::Error::other("el gestor dijo que no")),
        });

        let error = editar(
            dir.path(),
            &systemd,
            &pool,
            "web.container",
            contenedor("web"),
        )
        .await
        .expect_err("una recarga fallida no puede dar la edición por buena");

        assert!(
            matches!(
                error,
                QuadletError::Systemd(SystemdError::ReloadFailed { .. })
            ),
            "se esperaba un fallo de recarga: {error}"
        );
    }

    /// Escenario «Borrado correcto»: el fichero desaparece y systemd recarga.
    #[tokio::test]
    async fn borra_el_fichero_y_recarga_systemd() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        fichero(dir.path(), "web.container", "[Container]\nImage=nginx\n");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        let quadlet = borrar(dir.path(), systemd.as_ref(), &pool, "web.container")
            .await
            .expect("el borrado tiene que salir bien");

        assert_eq!(quadlet.file_name, "web.container");
        assert_eq!(quadlet.unit_name.as_str(), "web.service");
        assert!(
            !dir.path().join("web.container").exists(),
            "el fichero tenía que desaparecer"
        );
        assert_eq!(systemd.recargas(), 1, "systemd tiene que recargar una vez");
    }

    /// Escenario «Borrado de algo inexistente»: `NotFound` sin recargar y sin
    /// efectos.
    #[tokio::test]
    async fn borrar_algo_que_no_existe_es_not_found_y_no_recarga() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        let error = borrar(dir.path(), systemd.as_ref(), &pool, "web.container")
            .await
            .expect_err("no se puede borrar lo que no existe");

        assert!(
            matches!(error, QuadletError::NotFound { .. }),
            "se esperaba «no existe»: {error}"
        );
        assert_eq!(systemd.recargas(), 0, "no se recarga si no se borró nada");
    }

    /// Un fallo de transporte durante la recarga tras borrar se propaga tipado y
    /// sin registrar nada.
    #[tokio::test]
    async fn un_fallo_de_transporte_al_borrar_se_propaga() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        fichero(dir.path(), "web.container", "[Container]\nImage=nginx\n");
        let (_base, pool) = base().await;
        let systemd = DobleQueFalla::con(|| SystemdError::Transport {
            causa: Box::new(std::io::Error::other("no hay bus")),
        });

        let error = borrar(dir.path(), &systemd, &pool, "web.container")
            .await
            .expect_err("un bus que no contesta no puede dar el borrado por bueno");

        assert!(
            matches!(error, QuadletError::Systemd(SystemdError::Transport { .. })),
            "se esperaba un fallo de transporte: {error}"
        );
    }

    /// Cuántas operaciones (edición o borrado) hay registradas.
    async fn operaciones_registradas(pool: &SqlitePool) -> i64 {
        let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM quadlet_operations")
            .fetch_one(pool)
            .await
            .expect("la tabla de operaciones tiene que existir");

        total
    }

    /// El tipo y el fichero del único registro de operación que haya.
    async fn ultima_operacion(pool: &SqlitePool) -> (String, String) {
        sqlx::query_as("SELECT operation, file_name FROM quadlet_operations")
            .fetch_one(pool)
            .await
            .expect("tiene que haber un registro de operación")
    }

    /// Escenario «Edición registrada»: una edición con éxito queda apuntada con su
    /// tipo y su nombre de fichero.
    #[tokio::test]
    async fn una_edicion_con_exito_se_registra() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        fichero(dir.path(), "web.container", "lo de antes");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        editar(
            dir.path(),
            systemd.as_ref(),
            &pool,
            "web.container",
            contenedor("web"),
        )
        .await
        .expect("la edición tiene que salir bien");

        assert_eq!(
            operaciones_registradas(&pool).await,
            1,
            "la edición tiene que quedar registrada"
        );
        assert_eq!(
            ultima_operacion(&pool).await,
            ("edit".to_owned(), "web.container".to_owned())
        );
    }

    /// Escenario «Operación registrada tras un borrado»: un borrado con éxito
    /// queda apuntado como borrado.
    #[tokio::test]
    async fn un_borrado_con_exito_se_registra() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        fichero(dir.path(), "web.container", "[Container]\nImage=nginx\n");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        borrar(dir.path(), systemd.as_ref(), &pool, "web.container")
            .await
            .expect("el borrado tiene que salir bien");

        assert_eq!(
            operaciones_registradas(&pool).await,
            1,
            "el borrado tiene que quedar registrado"
        );
        assert_eq!(
            ultima_operacion(&pool).await,
            ("delete".to_owned(), "web.container".to_owned())
        );
    }

    /// Escenario «Operación fallida no registrada»: una edición cuya recarga falla
    /// no deja apunte aunque el fichero se haya reescrito.
    #[tokio::test]
    async fn una_operacion_fallida_no_se_registra() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        fichero(dir.path(), "web.container", "lo de antes");
        let (_base, pool) = base().await;
        let systemd = DobleQueFalla::con(|| SystemdError::ReloadFailed {
            causa: Box::new(std::io::Error::other("el gestor dijo que no")),
        });

        editar(
            dir.path(),
            &systemd,
            &pool,
            "web.container",
            contenedor("web"),
        )
        .await
        .expect_err("una recarga fallida no puede dar la edición por buena");

        assert_eq!(
            operaciones_registradas(&pool).await,
            0,
            "no puede quedar registrada una operación que no se completó"
        );
    }

    /// Escenario «El pool conserva WAL y foreign_keys»: registrar una operación a
    /// través del pool no relaja las dos garantías que fija el arranque.
    #[tokio::test]
    async fn el_pool_conserva_wal_y_claves_foraneas_al_registrar() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        fichero(dir.path(), "web.container", "lo de antes");
        let (_base, pool) = base().await;
        let systemd = Arc::new(FakeSystemd::new());

        editar(
            dir.path(),
            systemd.as_ref(),
            &pool,
            "web.container",
            contenedor("web"),
        )
        .await
        .expect("la edición tiene que salir bien");

        let (foraneas,): (i64,) = sqlx::query_as("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .expect("el PRAGMA tiene que responder");
        let (modo,): (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .expect("el PRAGMA tiene que responder");

        assert_eq!(foraneas, 1, "las claves foráneas tienen que seguir activas");
        assert_eq!(
            modo.to_lowercase(),
            "wal",
            "el modo WAL tiene que seguir en pie: {modo}"
        );
    }

    /// Escenario «Lectura correcta»: una quadlet existente devuelve su tipo y sus
    /// campos, sin tocar el fichero ni registrar nada. La lectura no necesita ni
    /// systemd ni el pool: por eso no los recibe.
    #[tokio::test]
    async fn leer_una_quadlet_devuelve_sus_campos() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let contenido = "[Container]\nImage=nginx\nContainerName=web\nPublishPort=8080:80\n";
        fichero(dir.path(), "web.container", contenido);

        let draft = leer(dir.path(), "web.container")
            .await
            .expect("la lectura tiene que salir bien");

        assert_eq!(draft.name, "web");
        assert_eq!(draft.kind, "container");
        assert_eq!(draft.image.as_deref(), Some("nginx"));
        assert_eq!(draft.container_name.as_deref(), Some("web"));
        assert_eq!(draft.publish_ports, ["8080:80"]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("web.container"))
                .expect("el fichero sigue ahí"),
            contenido,
            "leer no puede tocar el fichero"
        );
    }

    /// Escenario «Quadlet inexistente»: `NotFound`.
    #[tokio::test]
    async fn leer_algo_que_no_existe_es_not_found() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");

        let error = leer(dir.path(), "web.container")
            .await
            .expect_err("no se puede leer una quadlet que no existe");

        assert!(
            matches!(error, QuadletError::NotFound { .. }),
            "se esperaba «no existe»: {error}"
        );
    }

    /// Y el nombre se resuelve con la misma validación que la edición: un nombre
    /// no admisible o un tipo no creable no llegan a mirar el disco.
    #[tokio::test]
    async fn leer_un_nombre_no_admisible_es_invalid_name() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");

        let error = leer(dir.path(), "web")
            .await
            .expect_err("un nombre sin extensión no es una quadlet");

        assert!(
            matches!(error, QuadletError::InvalidName { .. }),
            "se esperaba un nombre inválido: {error}"
        );
    }
}
