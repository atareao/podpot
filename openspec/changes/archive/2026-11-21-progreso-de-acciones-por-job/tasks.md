# Tasks — progreso-de-acciones-por-job

TDD por bloques: primero las pruebas (rojo), luego la implementación mínima (verde), luego refactor. Cada tarea indica su criterio de verificación. Orden por dependencia: modelo → puerto → señales reales → doble → estado/canal → API SSE → frontend → cierre.

## 1. Modelo de dominio

- [x] 1.1 Tipo `ProgresoDeJob` en `systemd` con `job` (id del job), `unit`, `kind` (tipo de job de systemd), `phase` (`new`/`done`) y `result` (ausente en `new`). — Verificación: test de construcción y de la serialización esperada del evento.
- [x] 1.2 Mapeo de la cadena `result` de `JobRemoved` a un conjunto acotado (`done`, `failed`, `canceled`, `timeout`, `dependency`, `skipped`, `abandoned`) sin pérdida del valor. — Verificación: test con cada valor y con uno desconocido.

## 2. Puerto y publicación real

- [x] 2.1 Puerto `ProgresoDeTrabajos` (`observar() -> broadcast::Receiver<ProgresoDeJob>`) en la capa de systemd. — Verificación: test rojo→verde con el doble que emite y un suscriptor que recibe.
- [x] 2.2 El cliente real escucha `JobNew`/`JobRemoved` del manager y publica `new`/`done` en su canal. — Verificación: test con doble de conexión/stream que simula las señales y comprueba los eventos publicados.

## 3. Doble de pruebas

- [x] 3.1 `FakeSystemd` implementa `ProgresoDeTrabajos` y expone un método para emitir eventos de job. — Verificación: test que emite desde el doble y recibe en el suscriptor.

## 4. Estado y canal

- [x] 4.1 `AppState` gana `trabajos: Arc<dyn ProgresoDeTrabajos>`, cableado en `main.rs` y en `test_support.rs`. — Verificación: test que construye el estado con el doble y observa un evento.
- [x] 4.2 El apagado ordenado no se ve bloqueado por el canal de trabajos. — Verificación: test que dispara el cierre y comprueba que el flujo termina.

## 5. API SSE

- [x] 5.1 `GET /api/v1/jobs/stream` (`text/event-stream`): emite un evento `job` por cada transición publicada en el canal. — Verificación: test de integración que lee un evento tras publicar en el canal.
- [x] 5.2 `retry:` y latidos periódicos. — Verificación: test que comprueba la presencia de `retry` y de un latido.
- [x] 5.3 Cada evento reutiliza la envoltura `{data}` con la forma del progreso del job. — Verificación: test que compara el `data` del evento con la forma esperada.
- [x] 5.4 `405` con envoltura para métodos distintos de `GET`; la ruta queda bajo el gate. — Verificación: test de `405` y de `401` sin sesión en modo OIDC.

## 6. Frontend

- [x] 6.1 Hook de suscripción con `EventSource` a `/api/v1/jobs/stream` que mantiene el mapa de jobs en curso por unidad. — Verificación: test de hook con `EventSource` simulado (alta en `new`, baja en `done`). — Revisión: granularidad **por job** (`unit → {jobId → kind}`), un `done` da de baja solo su job (M1); referencia estable cuando no cambia nada; nota de que la reconexión es nativa de `EventSource` (B5).
- [x] 6.2 La tabla muestra el indicador «en curso» para las unidades con job, con el tipo de acción. — Verificación: test de componente. — Revisión: etiquetas en español vía `VERBOS` compartido con fallback al `kind` crudo (B3); `div role="status"` + `Spin aria-hidden` (B1/B2); tipo `TrabajosEnCurso` importado del hook (B4).
- [x] 6.3 Baja al desmontar y degradación a la carga inicial si el canal no está. — Verificación: test que comprueba `close()` al desmontar y que el panel sigue funcionando sin canal.

## 7. Cierre

- [x] 7.1 `just check-all` en verde y `clippy -D warnings` limpio. — Verificación: salida EXIT 0.
- [x] 7.2 Reviewers (`rust-reviewer`, `react-reviewer`) sin bloqueantes. — Verificación: informes de review.
- [x] 7.3 `openspec validate --all` en verde y `docs/progreso-de-acciones-por-job.md` redactado. — Verificación: validación OK y doc existente.
