# persistencia Specification

## Purpose
La persistencia del estado propio de podpot sobre SQLite, con migraciones embebidas y modo WAL.

## Requirements

### Requirement: Pool de SQLite

La aplicación SHALL abrir un pool de SQLite con `foreign_keys` habilitado en las opciones de conexión (no en un `PRAGMA` suelto) y modo WAL.

#### Scenario: Claves foráneas activas

- **WHEN** el pool se abre
- **THEN** `PRAGMA foreign_keys` SHALL devolver `1` en cualquier conexión del pool

### Requirement: Migraciones embebidas

El esquema SHALL aplicarse con migraciones embebidas, nombradas con un prefijo numérico entero (`0001_init.sql`), sin intervención manual.

#### Scenario: Base de datos nueva

- **WHEN** la aplicación arranca contra una base vacía
- **THEN** las migraciones SHALL aplicarse solas y el esquema SHALL quedar disponible

#### Scenario: Directorio ausente

- **WHEN** `DATABASE_URL` apunta a un directorio que no existe
- **THEN** el arranque SHALL fallar nombrando la ruta, sin crear el directorio

#### Scenario: Migración posterior

- **WHEN** se añade una migración nueva con el prefijo entero (`0002_*.sql`)
- **THEN** SHALL aplicarse sola tras las anteriores sobre una base existente, sin renombrar las previas

### Requirement: Registro de creaciones de quadlets

La persistencia SHALL almacenar un registro de cada quadlet creada, en una tabla propia introducida por una migración nueva con prefijo numérico entero. El pool SHALL preservar WAL y `foreign_keys` habilitado en las opciones de conexión al escribir el registro.

#### Scenario: Tabla disponible tras migrar

- **WHEN** la aplicación arranca con la migración de creaciones aplicada
- **THEN** SHALL existir la tabla de registro de creaciones, con el nombre de fichero y de unidad y el momento de creación

#### Scenario: Claves foráneas y WAL preservados

- **WHEN** se escribe un registro por el pool
- **THEN** `PRAGMA foreign_keys` SHALL seguir devolviendo `1` y el modo WAL SHALL permanecer activo

#### Scenario: Consulta de creaciones

- **WHEN** existen registros de creaciones
- **THEN** SHALL poder consultarse ordenados por momento de creación

### Requirement: Registro de ediciones y borrados

La persistencia SHALL almacenar un registro de cada edición y cada borrado de quadlet completados con éxito, en una tabla propia introducida por una migración nueva con prefijo numérico entero (`0003_*.sql`). El registro SHALL distinguir el tipo de operación (edición o borrado) e incluir el nombre de fichero afectado y una marca temporal. El pool SHALL preservar WAL y `foreign_keys` habilitado en las opciones de conexión al escribir el registro.

#### Scenario: Registro tras una edición

- **WHEN** una edición termina con éxito
- **THEN** SHALL existir un registro con el tipo de operación de edición y el nombre de fichero afectado

#### Scenario: Registro tras un borrado

- **WHEN** un borrado termina con éxito
- **THEN** SHALL existir un registro con el tipo de operación de borrado y el nombre de fichero afectado

#### Scenario: El pool conserva WAL y foreign_keys

- **WHEN** el registro se escribe a través del pool
- **THEN** el pool SHALL mantener `foreign_keys=1` y el modo WAL
