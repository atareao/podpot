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
