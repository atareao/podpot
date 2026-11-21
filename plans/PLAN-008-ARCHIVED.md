# PLAN-008 — progreso de acciones por job

- **Issue:** #16 — ✨ Progreso de acciones por job (start/stop/restart) en el panel
- **Rama:** `feature/progreso-de-acciones-por-job` (desde `development`, HEAD `8aa138f`)
- **Change OpenSpec:** `openspec/changes/progreso-de-acciones-por-job/`
- **Estado:** PLANIFICACIÓN (pendiente de aprobación de la spec)

## Objetivo

Mostrar en el panel el **progreso del job** que systemd ejecuta al accionar una unidad (start/stop/restart), y los jobs que systemd arranca por su cuenta, mediante un canal SSE nuevo alimentado por las señales `JobNew`/`JobRemoved` del manager D-Bus. La acción HTTP sigue siendo síncrona (no cambia su contrato); el canal es **aditivo**.

## Mapa arquitectónico

- **Dominio (`backend/src/systemd/`)**: nuevo tipo `ProgresoDeJob` (`job`, `unit`, `kind`, `phase`, `result`) y nuevo puerto `ProgresoDeTrabajos` con suscripción (`observar() -> broadcast::Receiver<ProgresoDeJob>`).
- **Cliente real (`backend/src/systemd/dbus.rs`)**: el cliente posee un `broadcast::Sender<ProgresoDeJob>` (como ya posee el de avisos de unidades) y lo publica al recibir `JobNew` (`new`) y `JobRemoved` (`done` + `result`). Sin sondeo.
- **Doble (`backend/src/systemd/fake.rs`)**: `FakeSystemd` implementa también `ProgresoDeTrabajos` y puede emitir eventos sin bus.
- **Estado (`backend/src/state.rs`)**: `AppState` gana `trabajos: Arc<dyn ProgresoDeTrabajos>` (como `systemd`/`registros`), cableado en `main.rs` y en `test_support.rs`.
- **API (`backend/src/api/`)**: nueva ruta `GET /api/v1/jobs/stream` (`text/event-stream`, evento `job`, contenido `{data}`), con `retry`, latido y cierre ordenado por el `watch` de apagado.
- **Frontend (`frontend/src/`)**: hook de `EventSource` al canal de trabajos; mapa de jobs en curso por unidad; indicador «en curso» en la tabla.

## Tareas (TDD: rojo → verde → refactor)

Cada tarea se implementa test-first; ver detalle en `openspec/changes/progreso-de-acciones-por-job/tasks.md`.

1. Modelo `ProgresoDeJob` + mapeo de `phase`/`result` (unit).
2. Puerto `ProgresoDeTrabajos` + publicación en el cliente real desde `JobNew`/`JobRemoved` (unit con doble de conexión/stream).
3. `FakeSystemd` implementa `ProgresoDeTrabajos` y emite eventos.
4. `AppState.trabajos` + cableado en `main.rs`/`test_support.rs`.
5. Ruta `GET /api/v1/jobs/stream` (evento `job`, `{data}`, `retry`, latido, `405`, cierre ordenado).
6. Frontend: hook de suscripción, estado por unidad «en curso», baja al desmontar, degradación.
7. Cierre: `just check-all` verde, reviewers sin bloqueantes, `openspec validate --all`, `docs/progreso-de-acciones-por-job.md`.

## Criterios de aceptación

- `just check-all` EXIT 0; `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios.
- `openspec validate progreso-de-acciones-por-job --strict` y `openspec validate --all` en verde.
- Reviewers `rust-reviewer` y `react-reviewer` sin bloqueantes.
- El canal `job` no altera el contrato de `GET /api/v1/units`, del stream de unidades ni de la acción.

## Gitflow

Feature → `development` por PR; conventional commits + gitmoji; al cerrar, marcar tareas y renombrar este plan a `plans/PLAN-008-ARCHIVED.md`.
