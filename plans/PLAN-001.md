# PLAN-001 — bootstrap-core

- **Feature:** esqueleto caminante con estado y control de quadlets.
- **Issue:** atareao/podpot#1
- **Change OpenSpec:** `openspec/changes/bootstrap-core/`
- **Rama:** `feature/bootstrap-core` (desde `development`)
- **Estado:** en curso (planificación completa; pendiente de aprobación para TDD).

## Objetivo

Dejar podpot de extremo a extremo —backend Rust+Axum, capa D-Bus (`zbus`) contra `org.freedesktop.systemd1`, API REST, persistencia SQLite, panel React+AntD y contenedor— mostrando el estado de las quadlets y permitiendo arrancarlas, pararlas y reiniciarlas. La **creación** de quadlets queda para un cambio posterior.

## Bloques (TDD, rojo → verde → refactor)

1. **Backend: shell y configuración** — envoltura JSON, salud, config del entorno, apagado ordenado.
2. **Persistencia** — pool SQLite (WAL, `foreign_keys`), migraciones embebidas.
3. **Capa D-Bus** — descubrimiento, lectura de estado, `start`/`stop`/`restart` con jobs, escapado de nombres.
4. **API de quadlets** — `GET /units`, `POST /units/{name}/{start|stop|restart}`.
5. **Frontend** — listado con estado y acciones de control.
6. **Despliegue** — `Containerfile` + `compose.yaml` con los montajes validados.
7. **Cierre** — `.justfile`, doc de feature, `just check`, revisión, PR y archivar el change.

Detalle de tareas en `openspec/changes/bootstrap-core/tasks.md`.

## Fuera de alcance

Creación de quadlets; autenticación OIDC; SSE/`PropertiesChanged`.

## Al cerrar

Marcar tareas, crear `docs/FEATURE-001-bootstrap-core.md` y renombrar este plan a `plans/PLAN-001-ARCHIVED.md`.
