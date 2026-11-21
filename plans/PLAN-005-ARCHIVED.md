# PLAN-005 — panel en vivo con SSE (empuje de cambios de estado de las quadlets)

- **Issue:** #10 — feat: panel en vivo con SSE (empuje de cambios de estado de las quadlets)
- **Change OpenSpec:** `openspec/changes/unidades-en-vivo`
- **Capability nueva:** `unidades-en-vivo` (deltas en `api-shell`, `systemd-dbus`, `ui`)
- **Estado:** en curso

## Objetivo

Que el panel refleje los cambios de estado de las quadlets sin que el usuario actúe: un canal SSE alimentado por las señales D-Bus de systemd (la razón arquitectónica de elegir `zbus` sobre shell-out).

## Arquitectura (dónde toca)

- `backend/src/systemd/`: puerto `UnitWatcher` (notificaciones de cambio, `broadcast::Receiver<()>`) + implementación real por señales D-Bus (`Subscribe()` + `PropertiesChanged` + `UnitNew`/`UnitRemoved`/`UnitFilesChanged`, con coalescencia) + doble en `fake.rs`.
- `backend/src/state.rs`: `AppState` gana `eventos: broadcast::Sender<Vec<UnitStatus>>` (instantánea de dominio) y canal de cierre.
- `backend/src/main.rs`: tarea de fan-out (notificación → recalcular `quadlets()` → publicar) y parada en el apagado.
- `backend/src/api/`: ruta `GET /api/v1/units/stream` (`text/event-stream`), instantánea inicial + evento `units` por cambio, `retry` y latidos.
- `backend/Cargo.toml`: feature `sse` de Axum (+ `tokio-stream`/`async-stream` si hace falta).
- `frontend/src/hooks/useUnits.ts` (+ api): `EventSource` → `setQueryData(['units'], instantánea)`; baja al desmontar; degradación a la carga inicial.

## Decisiones clave

- Señales reales, no polling. El vigilante avisa «algo cambió» (sin datos); el snapshot se recalcula con la vía ya probada (`quadlets()`).
- El canal lleva la instantánea **completa** (idempotente), no deltas; mismo tipo que `GET /api/v1/units` (`UnitSummary`).
- El SSE es la **excepción** a la envoltura JSON única; el contenido de cada evento sí usa `{data}`.
- Cierre ordenado: el stream termina con el servidor (no bloquea `with_graceful_shutdown`).

## Tareas (TDD por bloques)

- [ ] 1. Dependencias (`sse`) y puerto `UnitWatcher`.
- [ ] 2. Vigilante real D-Bus (`Subscribe()`, señales, coalescencia).
- [ ] 3. Doble de pruebas (`FakeSystemd` implementa `UnitWatcher`).
- [ ] 4. Canal de eventos + fan-out + cierre.
- [ ] 5. API SSE (instantánea inicial, eventos, `retry`/latidos, `{data}`, `405`).
- [ ] 6. Frontend (`EventSource` + react-query, baja al desmontar, degradación).
- [ ] 7. Cierre (`just check-all`, reviewers, `openspec validate`, `docs/unidades-en-vivo.md`).

## Criterios de aceptación

- Un cambio de estado producido fuera del panel se refleja en el panel en tiempo real.
- `GET /api/v1/units/stream` emite la instantánea inicial y un evento por cambio.
- El stream no bloquea el apagado ordenado.
- `just check-all` EXIT 0; `openspec validate --all` en verde; reviewers sin bloqueantes.

## Fuera de alcance

- SSE de logs o de progreso de acciones; autenticación del stream (llega con OIDC); sustituir `GET /api/v1/units` (el SSE es aditivo).
