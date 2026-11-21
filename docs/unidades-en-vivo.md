# unidades-en-vivo — panel en vivo con SSE

## Qué es

Un canal **SSE** (`text/event-stream`) que empuja al panel la instantánea de las quadlets cuando cambia su estado en systemd, sin que el usuario actúe. Materializa la decisión arquitectónica de usar **D-Bus directo (`zbus`)** frente al shell-out: el estado autoritativo vive en systemd y se empuja con sus señales.

Issue: #10 · Change OpenSpec: `openspec/changes/archive/2026-11-21-unidades-en-vivo` · Plan: `plans/PLAN-005-ARCHIVED.md`.

## Ficheros implicados

Backend (`backend/src/`):
- `api/stream.rs` — ruta `GET /api/v1/units/stream`: instantánea inicial + un evento `units` por publicación, `retry`, latido y cierre por canal.
- `fanout.rs` — tarea que convierte los avisos del vigilante en instantáneas publicadas.
- `systemd/mod.rs` — puerto `UnitWatcher` (`fn subscribe(&self) -> broadcast::Receiver<()>`).
- `systemd/dbus.rs` — vigilante real: `Subscribe()`, regla `PropertiesChanged` (espacio de rutas de unidades) + `UnitNew`/`UnitRemoved`/`UnitFilesChanged`, coalescencia (debounce 250 ms) y **reconexión con backoff** (1 s → 30 s).
- `systemd/fake.rs` — `FakeSystemd` implementa `UnitWatcher` y `notificar()`.
- `state.rs` — `AppState` gana `eventos: broadcast::Sender<Vec<UnitStatus>>` y `apagado: watch::Receiver<bool>`.
- `main.rs` — arranca el fan-out y dispara el cierre antes del `graceful shutdown`.

Frontend (`frontend/src/`):
- `hooks/useUnidadesEnVivo.ts` — `EventSource` a `/api/v1/units/stream`, escucha el evento `units`, desenvuelve `{data}` y hace `setQueryData(['units'])`.
- `hooks/claves.ts` — claves de react-query compartidas.
- `hooks/useUnits.ts` — invoca el hook (panel en vivo).

## Cómo funciona

1. Al arrancar, el cliente D-Bus llama `Subscribe()` y escucha las señales de systemd; cada señal (coalescida) emite un aviso en un `broadcast`.
2. La tarea de **fan-out** consume los avisos, recalcula la instantánea con `quadlets()` y la publica en `AppState.eventos` (`broadcast<Vec<UnitStatus>>`).
3. La ruta SSE, al conectar, **se suscribe primero** y luego envía la instantánea inicial; a partir de ahí reemite cada instantánea como evento `units` (envoltura `{data}`), con `retry` y latidos.
4. El frontend, al recibir un evento, reemplaza el dato de la consulta `['units']` (sin refetch). `EventSource` reconecta solo; el hook cierra al desmontar.

## Contrato

- `GET /api/v1/units/stream` → `text/event-stream`. Evento `units` con `data: {"data":[UnitSummary,...]}` (misma forma que `GET /api/v1/units`). Solo `GET`; otro método → `405` envuelto. Si el gestor falla al conectar → `503` envuelto (no se abre el flujo).
- El SSE es la **excepción** a la envoltura JSON única (es continuo), pero el contenido de cada evento sí usa `{data}`.

## Configuración / despliegue

- Sin configuración nueva: el SSE va por el **mismo origen** (`/api/v1/units/stream`); en dev, Vite proxya `/api`. Mismo bind loopback que el resto.
- No requiere puertos ni variables adicionales. La autenticación del stream llegará con OIDC.

## Decisiones

- **Señales reales, no polling**: el vigilante avisa «algo cambió» (sin datos) y la instantánea se recalcula con la vía ya probada (`quadlets()`).
- **Evento con la instantánea completa** (no deltas): idempotente, tolera eventos perdidos y reutiliza `UnitSummary`.
- **Cierre ordenado**: `watch::channel(false)`; la señal de terminación lo pone a `true` **antes** de que `with_graceful_shutdown` espere conexiones, así un SSE perpetuo no cuelga el apagado. El vigilante tiene su propio `watch` + `Drop` (nace antes que los canales de la app).
- **Reconexión con backoff** (1 s → 30 s, con reinicio tras sesión sana): la caída del bus se registra (`warn`) y se reintenta; no muere en silencio.
- **Sin dependencias nuevas**: `axum 0.8` expone `response::sse` sin feature; el flujo se compone con `futures_util::stream::unfold` (ya presente).

## Follow-ups

- **Sondeo de seguridad** si el canal se corta de forma permanente (la reconexión cubre cortes transitorios).
- **Autenticación del stream** (OIDC).
- **SSE de logs / progreso de acciones** (fuera de alcance).
- `useUnits` abre un `EventSource` por consumidor: hoy solo lo usa `unidades.tsx`; si se reutiliza el hook en varios componentes, montar la suscripción una sola vez (p. ej. en un provider).
