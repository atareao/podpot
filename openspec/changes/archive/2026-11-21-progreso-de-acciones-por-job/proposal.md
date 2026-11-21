# Proposal

## Why

El panel ya refleja el **estado** de las quadlets en vivo (SSE `units`), pero no el **ciclo de vida del job** que systemd ejecuta al arrancar, parar o reiniciar una unidad. Cuando el usuario pulsa «arrancar» solo ve un botón deshabilitado hasta que la petición termina, y si systemd arranca un job por su cuenta (reinicio automático, dependencia, otro cliente) el panel no lo muestra. El estado final llega por el canal de estado, pero el **avance de la acción** —que hay un job en curso sobre una unidad concreta y cómo termina— queda invisible.

## What Changes

- Nuevo endpoint **`GET /api/v1/jobs/stream`** (`text/event-stream`): emite un evento `job` por cada señal de ciclo de vida de un job de systemd (`JobNew`/`JobRemoved`), con la unidad, el tipo de job (`start`/`stop`/`restart`/…), la fase (`new`/`done`) y, al terminar, el resultado (`done`/`failed`/…). Incluye `retry`, latido y cierre ordenado.
- **Puerto `ProgresoDeTrabajos`**: la capa de systemd publica los eventos del ciclo de vida de los jobs (alimentados por las señales D-Bus del manager, sin sondeo) y expone la suscripción. El doble de pruebas puede emitirlos sin bus real, igual que `UnitWatcher`.
- **Frontend**: el panel consume el canal y muestra, por unidad, el **progreso de la acción en curso** (indicador «en curso» con el tipo de job), sin bloquear la interfaz ni duplicar suscripciones. Al terminar, el estado llega por el canal de estado ya existente.

## Capabilities

### New Capabilities

- `progreso-de-trabajos`: contrato del canal SSE de progreso de jobs (formato de los eventos, latido/reconexión, cierre) y del puerto que lo alimenta.

### Modified Capabilities

- `systemd-dbus`: nuevo puerto de **eventos de progreso de job** (fase y resultado), alimentado por las señales `JobNew`/`JobRemoved` del manager, con doble de pruebas.
- `api-shell`: se añade la ruta `GET /api/v1/jobs/stream`; la excepción a la envoltura JSON única pasa a cubrir los flujos SSE (unidades y trabajos). El apagado ordenado cierra cualquier flujo SSE.
- `ui`: el panel muestra el progreso de las acciones por unidad desde el canal de trabajos.

## Impact

- Backend (`backend/src/`): nuevo puerto `ProgresoDeTrabajos` + publicación de señales de job en `systemd/dbus.rs` y doble en `systemd/fake.rs`; campo `trabajos` en `AppState`; ruta SSE nueva en `api/`; ajuste de la impresión de cierre.
- Frontend (`frontend/src/`): hook de `EventSource` al canal de trabajos y estado por unidad «en curso» en la tabla.
- Contrato de API: **aditivo** (`GET /api/v1/units`, `POST /api/v1/units/{name}/{action}` y el stream de unidades no cambian); se añade un endpoint de stream.
- Despliegue: sin cambios (mismo origen; el SSE va por el mismo bind).
