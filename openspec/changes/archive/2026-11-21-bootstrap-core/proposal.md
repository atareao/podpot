# Proposal — bootstrap de podpot: esqueleto caminante con estado y control de quadlets

## Why

No existe todavía código. podpot debe ser un panel web para gestionar las **quadlets de Podman** de la instancia rootless de systemd (`systemctl --user`): informar del estado de cada contenedor y controlarlo (start/stop/restart), y crear quadlets nuevas. El *spike* de despliegue ya validó que D-Bus al systemd de usuario funciona desde un contenedor con `--userns=keep-id` y el bus montado (`/run/user/<uid>/bus`); falta el esqueleto real que lo materialice.

## What Changes

- **Backend** Rust + Axum en `backend/`: arranque, configuración desde entorno, envoltura JSON única, salud, apagado ordenado y errores tipados.
- **Capa D-Bus** (`zbus`) contra `org.freedesktop.systemd1`: descubrimiento de unidades quadlet, lectura de estado (`ActiveState`/`SubState`/`UnitFileState`) y acciones `start`/`stop`/`restart` respetando el modelo de **jobs asíncronos**.
- **API REST** bajo `/api/v1/`: `GET /health`, `GET /units`, `POST /units/{name}/{start|stop|restart}`.
- **Persistencia** SQLite vía `sqlx` (pool, migraciones embebidas, WAL) para el estado propio de podpot (ajustes y registro de acciones).
- **Frontend** React + TypeScript + Ant Design en `frontend/`: layout y tabla de quadlets con su estado y las acciones de control.
- **Despliegue**: `Containerfile` y `compose.yaml` con los montajes validados en el spike.
- **Andamiaje de proyecto**: `.justfile` (`check`, `check-all`, `ui-*`), flujo gitflow, `docs/` y `plans/`.

Fuera de alcance de este cambio (**non-goals**): la **creación** de quadlets nuevas (cambio siguiente), autenticación OIDC (queda como bind a loopback), y la suscripción SSE a `PropertiesChanged` (esta entrega se apoya en consulta).

## Capabilities

### New Capabilities

- `api-shell`: arranque del servidor, envoltura JSON de respuestas, ruta de salud, manejo de `404`/`405`, errores tipados y apagado ordenado.
- `config`: configuración desde el entorno (`BIND_ADDRESS`, `DATABASE_URL`, `RUST_LOG`, prefijo `PODPOT_*`) con reglas de arranque y valores por defecto.
- `persistencia`: pool de SQLite, migraciones embebidas con prefijo numérico, WAL y las entidades propias.
- `systemd-dbus`: cliente D-Bus contra `org.freedesktop.systemd1` del usuario: descubre unidades quadlet, lee su estado y ejecuta `start`/`stop`/`restart` esperando el job.
- `deploy`: imagen de contenedor y `compose.yaml` con los montajes y variables de entorno necesarios.
- `ui`: panel Antd con el listado de quadlets y sus acciones de control.

### Modified Capabilities

_(ninguna: no hay especificaciones previas)_

## Impact

- **Código nuevo**: `backend/` (Rust), `frontend/` (React/TS), `migrations/`, `Containerfile`, `compose.yaml`, `.justfile`.
- **Dependencias**: `axum`, `tokio`, `zbus`, `sqlx` (SQLite), `serde`, `tracing`.
- **Sistemas**: systemd de usuario (D-Bus) y Podman rootless del host; requiere `Linger=yes` y los montajes documentados en `AGENTS.md`.
- **Riesgo**: la variabilidad de systemd/Podman en otros hosts; el contrato D-Bus se fija en las especificaciones para poder probarlo con un *fakes*.
