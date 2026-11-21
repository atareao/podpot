//! La persistencia propia de podpot sobre SQLite.
//!
//! En esta entrega el estado de negocio vive en systemd y las quadlets en disco,
//! así que la base **no** guarda unidades: guarda lo que es de podpot y de nadie
//! más (los ajustes del panel y el registro de las acciones de control). Lo que
//! sí se monta de verdad aquí es la infraestructura —pool, WAL, claves foráneas
//! y migraciones embebidas—, para que la primera entidad de negocio no tenga que
//! estrenar nada de eso.

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::migrate::MigrateError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use thiserror::Error;

/// Las migraciones del esquema, embebidas en el binario.
///
/// Se leen de `backend/migrations/` **en tiempo de compilación**, así que el
/// binario lleva dentro el esquema que le corresponde: un despliegue no puede
/// arrancar con migraciones que no son las suyas. Los ficheros van con prefijo
/// numérico entero (`0001_init.sql`) porque el orden lo fija ese número y no el
/// nombre.
static MIGRACIONES: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Por qué no se pudo dejar la base lista.
#[derive(Debug, Error)]
pub enum DbError {
    /// El DSN no se puede leer como una configuración de SQLite.
    #[error("`{dsn}` no es un DSN de SQLite válido: {causa}")]
    Dsn {
        /// El DSN tal y como llegó.
        dsn: String,
        /// El motivo, tal y como lo informó el lector.
        #[source]
        causa: sqlx::Error,
    },

    /// La base no se pudo abrir.
    ///
    /// El mensaje nombra el DSN —y con él la ruta— porque es lo único que quien
    /// despliega necesita para arreglarlo. Es el caso del directorio ausente: no
    /// se crea, se falla.
    #[error("no se puede abrir la base de datos `{dsn}`: {causa}")]
    Abrir {
        /// El DSN tal y como llegó.
        dsn: String,
        /// El motivo, tal y como lo informó SQLite.
        #[source]
        causa: sqlx::Error,
    },

    /// El esquema no se pudo poner al día.
    #[error("no se pudieron aplicar las migraciones: {0}")]
    Migrar(#[from] MigrateError),
}

/// Abre el pool de SQLite y deja el esquema al día.
///
/// Las tres decisiones que se toman aquí y en ningún otro sitio:
///
/// * **Claves foráneas en las opciones de conexión**, no en un `PRAGMA` suelto:
///   con el pool, una conexión se abre cuando hace falta, y un `PRAGMA` lanzado
///   una vez sólo valdría para la conexión en la que se lanzó —las demás
///   entrarían sin él y el `ON DELETE CASCADE` no ocurriría—.
/// * **WAL**, para que leer el estado no bloquee a quien escribe el registro.
/// * **`create_if_missing`**, para que una base nueva se cree, pero un
///   directorio ausente siga siendo un error: SQLite no inventa directorios.
///
/// Y el esquema se aplica **antes** de devolver el pool: quien lo recibe sabe que
/// las tablas existen.
pub async fn preparar(dsn: &str) -> Result<SqlitePool, DbError> {
    let opciones = opciones_del_dsn(dsn)?;

    let pool = SqlitePoolOptions::new()
        .connect_with(opciones)
        .await
        .map_err(|causa| DbError::Abrir {
            dsn: dsn.to_owned(),
            causa,
        })?;

    MIGRACIONES.run(&pool).await?;

    Ok(pool)
}

/// Lee el DSN y fija las opciones de conexión del pool.
///
/// El DSN no se traduce a mano: `SqliteConnectOptions` ya entiende el formato
/// (`sqlite://ruta`, `?mode=ro`, …) y traducirlo aquí sería inventar un dialecto.
/// Lo que sí se hace es **endurecer** lo que no puede quedar al azar.
fn opciones_del_dsn(dsn: &str) -> Result<SqliteConnectOptions, DbError> {
    let opciones = SqliteConnectOptions::from_str(dsn).map_err(|causa| DbError::Dsn {
        dsn: dsn.to_owned(),
        causa,
    })?;

    Ok(opciones
        // Una base nueva se crea; un directorio ausente, no: SQLite devuelve
        // `unable to open database file` y el error nombra el DSN.
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        // Si otra conexión está escribiendo, se espera un poco en vez de fallar:
        // el registro de acciones y el panel pueden coincidir.
        .busy_timeout(Duration::from_secs(5)))
}

#[cfg(test)]
mod tests {
    use super::preparar;

    /// Una base de datos en un directorio temporal que se borra al soltarlo.
    ///
    /// Se devuelve el `TempDir` además del DSN porque el directorio se borra
    /// cuando el valor se suelta: si la prueba no lo mantiene vivo, la base
    /// desaparece antes de usarla.
    fn base_temporal() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let dsn = format!("sqlite://{}/podpot.db", dir.path().display());

        (dir, dsn)
    }

    /// Escenario «Claves foráneas activas»: no basta con que la primera conexión
    /// las traiga encendidas —cada conexión del pool se configura sola—, así que
    /// se comprueban varias.
    #[tokio::test]
    async fn las_claves_foraneas_estan_activas_en_toda_conexion() {
        let (_dir, dsn) = base_temporal();

        let pool = preparar(&dsn)
            .await
            .expect("la base temporal tiene que abrirse");

        let mut conexiones = Vec::new();
        for _ in 0..3 {
            conexiones.push(
                pool.acquire()
                    .await
                    .expect("el pool tiene que dar una conexión"),
            );
        }

        for conexion in &mut conexiones {
            let (activas,): (i64,) = sqlx::query_as("PRAGMA foreign_keys")
                .fetch_one(&mut **conexion)
                .await
                .expect("el PRAGMA tiene que responder");

            assert_eq!(
                activas, 1,
                "una conexión del pool llegó con las claves foráneas apagadas"
            );
        }
    }

    /// Y el PRAGMA no es decorativo: una clave foránea que no se cumple se rechaza.
    #[tokio::test]
    async fn una_clave_foranea_incumplida_se_rechaza() {
        let (_dir, dsn) = base_temporal();
        let pool = preparar(&dsn)
            .await
            .expect("la base temporal tiene que abrirse");

        sqlx::query("CREATE TABLE padre (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("la tabla auxiliar se crea");
        sqlx::query(
            "CREATE TABLE hijo (id INTEGER PRIMARY KEY, padre_id INTEGER NOT NULL \
             REFERENCES padre (id) ON DELETE CASCADE)",
        )
        .execute(&pool)
        .await
        .expect("la tabla auxiliar se crea");

        let resultado = sqlx::query("INSERT INTO hijo (padre_id) VALUES (999)")
            .execute(&pool)
            .await;

        assert!(
            resultado.is_err(),
            "se insertó un hijo sin padre: las claves foráneas no estaban en vigor"
        );
    }

    /// Requisito «Pool de SQLite»: el modo WAL queda pedido en las opciones, así
    /// que la base tiene que informarlo.
    #[tokio::test]
    async fn la_base_va_en_modo_wal() {
        let (_dir, dsn) = base_temporal();
        let pool = preparar(&dsn)
            .await
            .expect("la base temporal tiene que abrirse");

        let (modo,): (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .expect("el PRAGMA tiene que responder");

        assert_eq!(
            modo.to_lowercase(),
            "wal",
            "la base no quedó en modo WAL: {modo}"
        );
    }

    /// Escenario «Base de datos nueva»: contra una base que no existía, el
    /// arranque la crea, aplica las migraciones solas y deja el esquema listo.
    #[tokio::test]
    async fn una_base_nueva_queda_con_el_esquema_aplicado() {
        let (_dir, dsn) = base_temporal();

        let pool = preparar(&dsn)
            .await
            .expect("una base vacía tiene que abrirse y migrarse");

        sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?)")
            .bind("tema")
            .bind("oscuro")
            .execute(&pool)
            .await
            .expect("la tabla de ajustes tiene que existir tras migrar");

        let (valor,): (String,) = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind("tema")
            .fetch_one(&pool)
            .await
            .expect("el ajuste tiene que poder leerse");

        assert_eq!(valor, "oscuro");

        let (aplicadas,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = 1")
                .fetch_one(&pool)
                .await
                .expect("el registro de migraciones tiene que existir");

        assert_eq!(aplicadas, 3, "se esperaban tres migraciones aplicadas");
    }

    /// Requisito «Registro de ediciones y borrados»: la migración nueva introduce
    /// la tabla de operaciones, con su tipo, su nombre de fichero y su marca
    /// temporal rellenada sola.
    #[tokio::test]
    async fn la_tabla_de_operaciones_existe_tras_migrar() {
        let (_dir, dsn) = base_temporal();
        let pool = preparar(&dsn)
            .await
            .expect("la base temporal tiene que abrirse");

        sqlx::query("INSERT INTO quadlet_operations (operation, file_name) VALUES (?, ?)")
            .bind("edit")
            .bind("web.container")
            .execute(&pool)
            .await
            .expect("la tabla de operaciones tiene que existir tras migrar");

        let (operacion, fichero, marca): (String, String, String) =
            sqlx::query_as("SELECT operation, file_name, occurred_at FROM quadlet_operations")
                .fetch_one(&pool)
                .await
                .expect("el registro tiene que poder leerse");

        assert_eq!(operacion, "edit");
        assert_eq!(fichero, "web.container");
        assert!(
            !marca.is_empty(),
            "la marca temporal tiene que rellenarse sola"
        );
    }

    /// La tabla sólo admite los dos tipos de operación que podpot registra.
    #[tokio::test]
    async fn la_tabla_de_operaciones_rechaza_un_tipo_desconocido() {
        let (_dir, dsn) = base_temporal();
        let pool = preparar(&dsn)
            .await
            .expect("la base temporal tiene que abrirse");

        let resultado =
            sqlx::query("INSERT INTO quadlet_operations (operation, file_name) VALUES (?, ?)")
                .bind("renombrar")
                .bind("web.container")
                .execute(&pool)
                .await;

        assert!(
            resultado.is_err(),
            "un tipo de operación desconocido no puede registrarse"
        );
    }

    /// Requisito «Migraciones embebidas»: aplicarlas dos veces no rompe nada ni
    /// duplica filas en el registro.
    #[tokio::test]
    async fn preparar_dos_veces_sobre_la_misma_base_es_idempotente() {
        let (_dir, dsn) = base_temporal();

        let primero = preparar(&dsn).await.expect("el primer arranque migra");
        primero.close().await;

        let segundo = preparar(&dsn)
            .await
            .expect("el segundo arranque no tiene que volver a migrar");

        let (aplicadas,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = 1")
                .fetch_one(&segundo)
                .await
                .expect("el registro de migraciones tiene que existir");

        assert_eq!(aplicadas, 3, "la migración se aplicó más de una vez");
    }

    /// Escenario «Directorio ausente»: si el directorio del DSN no existe, el
    /// arranque falla nombrando la ruta y **sin** crear el directorio.
    #[tokio::test]
    async fn un_directorio_ausente_falla_nombrando_la_ruta_sin_crearla() {
        let dir = tempfile::tempdir().expect("se tiene que poder crear un directorio temporal");
        let ausente = dir.path().join("no-existe");
        let dsn = format!("sqlite://{}/podpot.db", ausente.display());

        let error = preparar(&dsn)
            .await
            .expect_err("una base en un directorio que no existe no puede abrirse");

        assert!(
            error.to_string().contains("no-existe"),
            "el error tiene que nombrar la ruta: {error}"
        );
        assert!(
            !ausente.exists(),
            "el arranque creó el directorio en vez de fallar"
        );
    }
}
