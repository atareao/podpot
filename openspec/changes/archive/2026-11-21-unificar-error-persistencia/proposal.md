# Proposal

## Why

`QuadletError` declara hoy dos variantes casi idénticas para el mismo desenlace (fallo de escritura en SQLite): `Persistencia` (creación, `backend/src/quadlet/mod.rs:229`) y `Registro` (edición/borrado, `:244`). Ambas colapsan al mismo código HTTP `internal` (`500`). La segunda es superficie duplicada sin valor de contrato: la causa real ya viaja en el campo `causa`. Es el follow-up registrado en `docs/edicion-borrado-quadlets.md:55`.

## What Changes

- Se elimina la variante `QuadletError::Registro`; `QuadletError::Persistencia` pasa a ser la única variante tipada de fallo de persistencia del ciclo de vida de una quadlet (creación, edición y borrado).
- Las construcciones de `Registro` en `backend/src/quadlet/service.rs:209` pasan a `Persistencia`.
- Se elimina el brazo `Registro` de `traducir_quadlet` (`backend/src/error.rs:423`) y su test asociado.
- El mapeo HTTP NO cambia: el fallo de persistencia se mantiene en `internal` (`500`) con la envoltura existente `{error:{code,message}}`.
- Documentación actualizada (`docs/edicion-borrado-quadlets.md`, `docs/FEATURE-002-creacion-quadlets.md`).

## Capabilities

### Modified Capabilities

- `persistencia`: se documenta que todo fallo al escribir en la base de datos durante el ciclo de vida de una quadlet (creación, edición o borrado) se representa con una única variante tipada (`Persistencia`) mapeada a `internal` (`500`), sin exponer la causa subyacente.

## Impact

- Backend (`backend/src/`): `quadlet/mod.rs`, `quadlet/service.rs`, `error.rs` y sus tests.
- Sin cambios en el frontend, en el contrato de la API, en el despliegue ni en la configuración.
