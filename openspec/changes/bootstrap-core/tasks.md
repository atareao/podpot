# Tasks — bootstrap-core

Issue **#1**. TDD por bloques: primero las pruebas (rojo), luego la implementación mínima (verde), luego refactor.

## 1. Backend: shell y configuración

- [x] 1.1 Bootstrap del crate en `backend/` (Axum, tokio, tracing, serde, zbus, sqlx).
- [x] 1.2 Envoltura JSON única + errores tipados con `IntoResponse` (rojo → verde).
- [x] 1.3 `GET /api/v1/health` pública.
- [x] 1.4 Configuración desde el entorno con las reglas de arranque (`config`).
- [x] 1.5 Apagado ordenado ante `SIGTERM`.

## 2. Persistencia

- [x] 2.1 Pool de SQLite (WAL, `foreign_keys` en opciones) y migraciones embebidas.
- [x] 2.2 Primera migración `0001_init.sql` y prueba de arranque contra base vacía.

## 3. Capa D-Bus (`systemd-dbus`)

- [x] 3.1 Trait de cliente systemd + doble de pruebas.
- [x] 3.2 Escapado de nombres de unidad (`. → _2e`, `- → _2d`).
- [x] 3.3 Descubrimiento de quadlets (`UnitFileState=generated`).
- [x] 3.4 Lectura de `ActiveState`/`SubState`.
- [x] 3.5 `start`/`stop`/`restart` esperando el job.

## 4. API de quadlets

- [x] 4.1 `GET /api/v1/units`.
- [x] 4.2 `POST /api/v1/units/{name}/{start|stop|restart}`.
- [x] 4.3 Traducción de errores D-Bus a errores de dominio.

## 5. Frontend (`ui`)

- [x] 5.1 Scaffold Vite + React + Antd + TanStack Query en `frontend/`.
- [x] 5.2 Listado de quadlets con estado.
- [x] 5.3 Acciones de control con feedback y refresco.
- [x] 5.4 Pruebas de componente (Vitest + Testing Library).

## 6. Despliegue (`deploy`)

- [x] 6.1 `Containerfile` multi-stage (usuario no-root, deps *baked*).
- [x] 6.2 `compose.yaml` con los montajes validados (`--userns=keep-id`, bus, quadlets, socket Podman).
- [x] 6.3 Smoke test de arranque que consulta `health`.

## 7. Cierre del cambio

- [x] 7.1 `.justfile` (`check`, `check-all`, `ui-*`).
- [x] 7.2 `docs/FEATURE-001-bootstrap-core.md`.
- [ ] 7.3 `just check` en verde y revisión (`@rust-reviewer` / `@react-reviewer`).
- [ ] 7.4 PR a `development` (cierra #1) y archivar el change (`openspec archive bootstrap-core`).
