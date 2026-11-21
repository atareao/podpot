# Spec Delta — persistencia

## Purpose

La persistencia propia de podpot (SQLite vía `sqlx`). Este delta añade el registro de ediciones y borrados de quadlets en una tabla propia.

## ADDED Requirements

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
