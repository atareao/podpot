# PLAN-003 — Edición y borrado de quadlets

- **Issue**: #5 (https://github.com/atareao/podpot/issues/5)
- **Change OpenSpec**: `edicion-borrado-quadlets`
- **Rama**: `feature/edicion-borrado-quadlets` (desde `development`)
- **Estado**: planificado — pendiente de aprobación del change para iniciar TDD.

## Objetivo

Completar el ciclo de vida de las quadlets permitiendo **editar** y **borrar** una quadlet existente desde podpot (API + UI), reutilizando el contrato tipado y la sanitización de la creación, con recarga de systemd por D-Bus y registro en persistencia.

## Alcance

### Backend (Rust + Axum)
- `PATCH /api/v1/quadlets/{name}`: reescritura atómica del fichero desde el struct tipado del `kind`.
- `DELETE /api/v1/quadlets/{name}`: borrado del fichero.
- Validación/sanitización idéntica a la creación; identidad por nombre de fichero `<stem>.<kind>`; sin renombrado.
- `daemon-reload` por D-Bus tras editar y borrar.
- Errores tipados con el mapeo HTTP estable de la creación.
- Migración `0003_*.sql` con la tabla de operaciones (edición/borrado).

### Frontend (React + TS + Antd)
- Acción de editar reutilizando el formulario de creación precargado.
- Acción de borrar con confirmación (acción destructiva).
- Refresco del listado y muestra de `error.message` en fallo.

## Fuera de alcance

- Renombrado de quadlets.
- Exponer campos opcionales (`subnet`, `pod_name`, `volumes`, `environments`, `publish_ports`, `container_name`) en la UI.
- Parada automática de la unidad antes del borrado (follow-up).

## Hitos y verificación

1. Aprobación del change `edicion-borrado-quadlets` (STOP & WAIT).
2. TDD backend: dominio → escritura → borrado → D-Bus → persistencia → API (`cargo test`, `cargo clippy -D warnings`).
3. TDD frontend: acciones y confirmación (`npx vitest run`, `npx tsc --noEmit`, `npm run lint`).
4. Auditorías `rust-reviewer` y `react-reviewer` sin bloqueantes.
5. Cierre: `just check-all` EXIT 0, `openspec validate --all`, `openspec archive edicion-borrado-quadlets`, doc `docs/edicion-borrado-quadlets.md`, PR a `development`, cierre del issue #5 y renombrado de este plan a `PLAN-003-ARCHIVED.md`.

## Tareas

Ver `openspec/changes/edicion-borrado-quadlets/tasks.md` (8 bloques).
