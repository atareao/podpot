# Proposal

## Why

El panel solo refleja cambios de estado de las quadlets cuando el usuario actúa o recarga: no hay canal de eventos. La decisión arquitectónica de usar **D-Bus directo (`zbus`)** frente al shell-out se tomó, entre otras razones, para poder **empujar** los cambios con las señales de systemd; ese push no existe todavía. Un cambio hecho fuera del panel (otro cliente, `systemctl --user`, un fallo del contenedor, un `daemon-reload`) queda invisible hasta el siguiente refetch.

## What Changes

- Nuevo endpoint **`GET /api/v1/units/stream`** (`text/event-stream`): al conectar envía la **instantánea** actual de las quadlets; después emite un evento `units` con la instantánea recalculada cada vez que cambia el estado. Incluye `retry:` y latido periódico para sostener la conexión.
- **Vigilante de unidades** en el backend: escucha las señales de systemd por D-Bus (`PropertiesChanged` de las unidades; `UnitNew`/`UnitRemoved`/`UnitFilesChanged` del manager) y avisa de «algo cambió». Sobre cada aviso, recalcula la instantánea y la publica en un canal `broadcast` del `AppState`.
- **Puerto `UnitWatcher`** (notificaciones de cambio) para que el doble de pruebas inyecte eventos sin bus real, igual que `SystemdClient`/`FakeSystemd`.
- **Cierre ordenado**: el stream termina cuando el servidor se apaga (no bloquea el shutdown).
- **Frontend**: suscripción con `EventSource` que actualiza la consulta `['units']` con la instantánea recibida, reconecta sola y se da de baja al desmontar.

## Capabilities

### New Capabilities

- `unidades-en-vivo`: contrato del canal SSE (formato de los eventos, instantánea inicial, latido/reconexión, cierre) y del vigilante que lo alimenta.

### Modified Capabilities

- `api-shell`: se añade la ruta `GET /api/v1/units/stream`; la envoltura JSON única se **exceptúa** para el stream (no es una respuesta de un solo documento).
- `systemd-dbus`: nuevo puerto `UnitWatcher` (notificaciones de cambio) con implementación real por señales D-Bus y doble de pruebas.
- `ui`: el panel pasa a ser **en vivo**, consumiendo el SSE y reflejando los cambios sin refetch manual.

## Impact

- Backend (`backend/src/`): nueva ruta SSE en `api/`; campo de canal `broadcast` en `AppState`; puerto `UnitWatcher` + implementación en `systemd/dbus.rs` y doble en `systemd/fake.rs`; tarea de fan-out en el arranque; ajuste del apagado.
- `backend/Cargo.toml`: habilitar la feature `sse` de Axum; añadir `tokio-stream`/`async-stream` si hiciera falta.
- Frontend (`frontend/src/`): hook de `EventSource` enganchado a `useUnits`/react-query.
- Contrato de API: **aditivo** (`GET /api/v1/units` no cambia); se añade un endpoint de stream.
- Despliegue: sin cambios (mismo bind loopback; el SSE va por el mismo origen).
