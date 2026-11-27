# PLAN-028 — Agrupar quadlets por prefijo con override explícito

Issue: https://github.com/atareao/podpot/issues/65
Fecha: 2026-11-27
Estado: Completado (2026-11-27)

## Contexto

- La feature anterior (`plans/PLAN-027-ARCHIVED.md`) implementó la agrupación por stack a partir del stem completo.
- Regla previa: tipos principales conservaban el stem completo; auxiliares (`volume`, `network`) recortaban sufijo case-insensitive.
- Resultado observado: `atareao-mariadb.container` → stack `atareao-mariadb`; `atareao-mariadb-data.volume` → stack `atareao-mariadb-data`; `atareao-network.network` → stack `atareao`.
- Usuario espera: todo bajo el stack `atareao` (o `centinela`) en una sola agrupación.

## Objetivo

1. Cambiar la regla base de cálculo de stack a: **primer segmento del stem antes del primer `-`**. Si no hay `-`, usar el stem completo.
2. Añadir override explícito mediante un fichero de configuración `groups.toml`, con prioridad sobre la regla base.
3. Mantener el contrato aditivo de `UnitSummary.stack` y el uso de `file_name` como identidad.
4. Actualizar tests, docs y OpenSpec.

## Reglas de negocio propuestas

- Base: `stack = stem.split('-').next()` (case-insensitive para la decisión, pero conservando la grafía del primer segmento? Decidir y documentar). Si el stem está vacío, fallback a `unit_name` sin `.service` ni sufijo de tipo.
- Override: tabla `mappings` en `groups.toml`. Claves: `file_name` (p. ej. `atareao-mariadb.container`) o `unit_name` (p. ej. `atareao-mariadb.service`). Valor: stack. Prioridad: override > base.
- Fallback: si no hay override, aplicar regla base.
- Auxiliares: la regla de prefijo ya elimina sufijos implícitos al partir por `-`; pero se mantiene la lógica de sufijo si el override no aplica. Definir claramente: para `foo-volume.volume`, stem = `foo-volume`, prefijo = `foo`. Para `foo-network.network`, stem = `foo-network`, prefijo = `foo`.

## Decisiones abiertas

- Ubicación exacta de `groups.toml`: en el directorio de la fuente del quadlet (p. ej. `~/.config/containers/systemd/groups.toml`) o en una ruta de configuración del backend. Recomendación inicial: buscar en el directorio fuente escaneado (`./quadlets/`, `.`).
- Formato TOML: tabla `[mappings]` con claves string y valores string. Ejemplo:
```toml
[mappings]
"atareao-mariadb.container" = "atareao"
"atareao-wordpress-data.volume" = "atareao"
```
- Alcance de keys: solo `file_name` y `unit_name`; valor string.
- Carga: lazy al resolver, o cacheada en backend. Recomendación inicial: cache en backend con recarga al cambiar filesystem.

## Cambios propuestos

- Backend:
  - `backend/src/quadlet/stack.rs`: añadir `stack_name_with_overrides(file_name, kind, unit_name, overrides)` o equivalente. Mantener `stack_name` como base si se prefiere no romper contrato. Implementar prefijo + fallback.
  - Nuevo módulo/cargador para `groups.toml` (por ejemplo `backend/src/quadlet/groups.rs`).
  - `backend/src/api/units.rs`: poblar `stack` con nueva lógica. Tests de contrato JSON.
- Frontend:
  - `frontend/src/lib/groupByStack.ts` y tests: agrupar por `stack`; no debería cambiar si el backend envía el correcto. Sólo actualizar tests si la semántica de stack cambia en fixtures.
- Docs:
  - `docs/agrupacion-por-stack.md`: describir prefijo automático y override explícito.

## Tareas TDD

1. RED: tests en `backend/src/quadlet/stack.rs` para base por prefijo (`atareao-mariadb` → `atareao`), override (`atareao-mariadb.container` → `atareao-mariadb`), fallback sin prefixo.
2. GREEN: implementar.
3. REFACTOR: limpiar.

## Verificación

- Backend: `cargo test`, `cargo check`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`.
- Frontend: `npx vitest run`, `npx tsc --noEmit`, `pnpm run lint`, `pnpm run build`.
- OpenSpec: `openspec validate agrupacion-por-prefijo --strict` (nuevo change).
- Manual: subir los quadlets actuales y verificar que todo cae bajo `atareao`.

## Riesgos / decisiones

- Over-grouping si el prefijo agrupa unidades no relacionadas.
- Falta de override actualmente; se añade.
- Cambio de semántica del stack desde feature cerrada; debe reflejarse en specs y plan archivado.

## Entregables

- `openspec/changes/agrupacion-por-prefijo/`
- `plans/PLAN-028.md`
- `docs/agrupacion-por-prefijo.md` (o actualización de docs existente)
- PR.

## Estado

Draft pendiente de aprobación del usuario tras revisión de OpenSpec.