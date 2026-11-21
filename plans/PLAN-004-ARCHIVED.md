# PLAN-004 — exponer identidad real de quadlet en `GET /api/v1/units`

- **Issue:** #6 — feat: exponer `file_name`/`kind` real en `GET /api/v1/units` (identidad de quadlet)
- **PR:** #8 → `development`
- **Change OpenSpec:** `openspec/changes/archive/2026-11-21-exponer-identidad-quadlet`
- **Estado:** completado

## Objetivo

Eliminar la deducción inversa (no inyectiva) de la identidad de una quadlet en el frontend, exponiendo el **nombre de fichero real** (`file_name`, basename del `SourcePath`) y el `kind` opcional por cada quadlet en `GET /api/v1/units`.

## Alcance

- Backend: accesor de dominio `file_name` (basename de `UnitOrigin`); `UnitSummary` publica `file_name` + `kind: Option<QuadletKind>`; la ruta del sistema de ficheros sigue sin publicarse.
- Frontend: `QuadletUnit` gana `file_name`/`kind`; editar/borrar operan sobre `file_name`; `rowKey="file_name"`; se elimina `src/quadlets/identidad.ts`.
- Specs: delta en `api-shell` (nuevo requirement del listado), `systemd-dbus` (basename derivado) y `ui` (identidad publicada).

## Criterios de aceptación (verificados)

- [x] `GET /api/v1/units` publica `file_name` por quadlet y NO la ruta.
- [x] El frontend usa `file_name` como identidad; sin deducción inversa.
- [x] Caso colisionado (`web-volume.service`) distinguido por `file_name`.
- [x] `just check-all` EXIT 0; `openspec validate --all` 8/8.
- [x] `rust-reviewer` y `react-reviewer` sin bloqueantes.

## Fuera de alcance

- Unificar códigos de error `Persistencia`/`Registro`; auth OIDC; SSE/`PropertiesChanged`; chunk de antd >500 kB (follow-ups en HANDOFF).
