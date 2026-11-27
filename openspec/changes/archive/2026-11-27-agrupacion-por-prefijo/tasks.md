# Tasks — Agrupación por Prefijo (PLAN-028)

## Backend (Rust + Axum)

- [x] Añadir campo `stack` a `UnitSummary` (ya presente de PLAN-027; se reutiliza).
- [x] Implementar cálculo de stack por prefijo antes del primer `-` en `stack_name` (`backend/src/quadlet/stack.rs`).
- [x] Implementar `stack_name_with_groups(file_name, kind, unit_name, groups)` con override por `file_name` y `unit_name`.
- [x] Implementar carga de `groups.toml` (`backend/src/quadlet/groups.rs`: parseo TOML, fusión por directorios, fallback si no existe).
- [x] Modificar handler `GET /api/v1/units` para incluir `stack` con override (`from_con_grupos`).
- [x] Modificar el flujo SSE (`GET /api/v1/units/stream`) para usar el mismo mapper con grupos.
- [x] Test unitario: prefijo (`atareao-mariadb.container` → `atareao`).
- [x] Test unitario: sin prefijo (`standalone.container` → `standalone`).
- [x] Test unitario: fallback a `unit_name` sin `file_name` (`centinela-app.service` → `centinela`).
- [x] Test unitario: override explícito por `file_name` en `groups.toml`.
- [x] Test unitario: override explícito por `unit_name` en `groups.toml`.
- [x] Test unitario: parseo y fusión de `groups.toml` (último directorio gana).
- [x] Test unitario: fusión de `groups.toml` por directorios (último directorio gana).
- [x] Contrato JSON: `GET /api/v1/units` publica `stack` con override (test del mapper `from_con_grupos`, que es la misma función que usa el handler y el SSE).

## Frontend (React + TypeScript + Ant Design)

- [x] Tipo `LiveUnit` con `stack: string` (ya presente de PLAN-027).
- [x] Vista "Unidades en vivo" agrupada por stack (cabeceras con `rowSpan`, ya presente de PLAN-027; el backend ahora envía el prefijo correcto).
- [x] Badge/tag del stack en cada fila de la tabla (ya presente).
- [x] Test de componente: render agrupado con unidades mock (ya presente).
## Verificación

- [x] Crear `docs/agrupacion-por-prefijo.md` (qué es, regla de prefijo, formato `groups.toml`, directorios, precedencia, recarga).
- [x] Actualizar `docs/agrupacion-por-stack.md` para apuntar al nuevo documento.

## Verificación

- [x] `cargo test` (backend) en verde.
- [x] `cargo check` (backend) en verde.
- [x] `cargo clippy -- -D warnings` sin warnings.
- [x] `cargo fmt --all -- --check` (backend) sin diferencias.
- [x] `npx vitest run` (frontend) en verde (154 tests).
- [x] `npx tsc --noEmit` (frontend) sin errores.
- [x] `npx eslint .` (frontend) sin errores.

## Follow-ups (fuera de este change)

- Prueba manual: vista agrupada con stacks reales (`atareao`, `centinela`).
- Test E2E (Playwright): no hay infraestructura E2E en el repositorio.
- Caché de `groups.toml` con vigilancia del filesystem si el volumen de peticiones crece.
