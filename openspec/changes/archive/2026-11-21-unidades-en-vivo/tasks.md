# Tasks — unidades-en-vivo

TDD por bloques: primero las pruebas (rojo), luego la implementación mínima (verde), luego refactor. Cada tarea indica su criterio de verificación. Orden por dependencia: dependencias → puerto → vigilante real → doble → canal/fan-out → API SSE → frontend → cierre.

## 1. Dependencias y puerto

- [x] 1.1 Habilitar la feature `sse` de Axum y añadir `tokio-stream`/`async-stream` si hace falta (documentar la elección). — Verificación: `cargo check` compila con las features nuevas.
- [x] 1.2 Puerto `UnitWatcher` en `systemd`: notificaciones de cambio (`broadcast::Receiver<()>`), sin datos de unidad. — Verificación: test rojo→verde con el doble que emite y un suscriptor que recibe.

## 2. Vigilante real (D-Bus)

- [x] 2.1 Llamar `Subscribe()` en el manager antes de escuchar señales (sin él systemd no emite señales de unidades no observadas). — Verificación: test (ignorado sin bus) o doble de conexión que verifica la llamada.
- [x] 2.2 Escuchar `PropertiesChanged` (unidades), `UnitNew`/`UnitRemoved`/`UnitFilesChanged` (manager) y reenviar como notificaciones. — Verificación: test con doble de conexión/stream que simula señales y comprueba la notificación.
- [x] 2.3 Coalescer ráfagas (debounce corto) en una sola notificación. — Verificación: test que inyecta varias señales seguidas y comprueba que solo se emite una notificación.
- [x] 2.4 Escenario «El bus cierra el stream de señales»: registrar el cierre y reintentar la suscripción con backoff creciente (con tope), rehaciendo `Subscribe()` y la regla; el bucle termina cuando el cierre del servidor lo pide. — Verificación: test puro del retraso, test del bucle con reloj parado, y test de bus real que arranca y para una quadlet y espera la notificación.

## 3. Doble de pruebas

- [x] 3.1 `FakeSystemd` implementa `UnitWatcher` y expone un método para emitir notificaciones. — Verificación: test que emite desde el doble y recibe en el suscriptor.

## 4. Canal de eventos, fan-out y cierre

- [x] 4.1 `AppState` gana `eventos: broadcast::Sender<Vec<UnitStatus>>` (y el canal de cierre del stream). — Verificación: test que construye el estado con el canal y publica/recibe una instantánea.
- [x] 4.2 Tarea de fan-out: por cada notificación del vigilante, recalcula `quadlets()` y publica la instantánea; arranca en `main` y se detiene al apagar. — Verificación: test con doble que emite → se publica la instantánea recalculada.
- [x] 4.3 El apagado ordenado detiene el fan-out y cierra los streams. — Verificación: test que dispara el cierre y comprueba que el fan-out termina y el stream acaba.

## 5. API SSE

- [x] 5.1 `GET /api/v1/units/stream` (`text/event-stream`): envía la instantánea inicial y luego un evento `units` por cada instantánea publicada. — Verificación: test de integración que lee el primer evento y, tras publicar en el canal, el siguiente.
- [x] 5.2 `retry:` y latidos periódicos. — Verificación: test que comprueba la presencia de `retry` y de un latido.
- [x] 5.3 Cada evento reutiliza la envoltura `{data}` con la forma de `UnitSummary`. — Verificación: test que compara el `data` del evento con la respuesta de `GET /api/v1/units`.
- [x] 5.4 `405` con envoltura para métodos distintos de `GET`; fallbacks preservados. — Verificación: test de `405` sobre la ruta del stream.
- [x] 5.5 Suscribirse al canal de eventos **antes** de calcular la instantánea inicial, para que un cambio ocurrido en el hueco entre listar y suscribirse no se pierda. — Verificación: test con el listado retenido que publica en ese hueco y comprueba que el evento llega igual.

## 6. Frontend

- [x] 6.1 Hook de suscripción con `EventSource` a `/api/v1/units/stream` que actualiza la consulta `['units']` con la instantánea. — Verificación: test de componente/hook con `EventSource` simulado que comprueba la actualización del listado.
- [x] 6.2 Baja al desmontar y reconexión; degradación a la carga inicial si el canal no está. — Verificación: test que comprueba `close()` al desmontar y que el panel sigue mostrando datos sin canal.

## 7. Cierre

- [x] 7.1 `just check-all` en verde y `clippy -D warnings` limpio. — Verificación: salida EXIT 0.
- [x] 7.2 Reviewers (`rust-reviewer`, `react-reviewer`) sin bloqueantes. — Verificación: informes de review.
- [x] 7.3 `openspec validate --all` en verde y `docs/unidades-en-vivo.md` redactado. — Verificación: validación OK y doc existente.
