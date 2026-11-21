# progreso-de-acciones-por-job — progreso del job en el panel (SSE)

## Qué es

Un canal **SSE** (`text/event-stream`) que empuja al panel el **ciclo de vida de los jobs** de systemd (`new` al crearse, `done` al terminar con su resultado), alimentado por las señales `JobNew`/`JobRemoved` del manager D-Bus. El panel muestra por unidad un indicador **«en curso»** con el tipo de acción. Es **aditivo**: no cambia la acción HTTP (sigue síncrona) ni el stream de unidades.

Issue: #16 · Change OpenSpec: `openspec/changes/archive/2026-11-21-progreso-de-acciones-por-job` · Plan: `plans/PLAN-008-ARCHIVED.md`.

## Ficheros implicados

Backend (`backend/src/`):
- `api/jobs.rs` — ruta `GET /api/v1/jobs/stream`: evento `job` por transición, `retry` en la apertura, latido y cierre por `watch`.
- `systemd/mod.rs` — dominio (`FaseDeJob`, `ResultadoDeJob`, `ProgresoDeJob`) y puerto `ProgresoDeTrabajos` (`fn observar(&self) -> broadcast::Receiver<ProgresoDeJob>`).
- `systemd/dbus.rs` — vigilante de trabajos: escucha `JobNew`/`JobRemoved`, lee la propiedad `JobType` del objeto del job, mantiene el mapa `en_curso` y publica los eventos; helper `suscribir()` (tolera `AlreadySubscribed`).
- `systemd/fake.rs` — `FakeSystemd` implementa `ProgresoDeTrabajos` y emite eventos sin bus.
- `state.rs` — `AppState` gana `trabajos: Arc<dyn ProgresoDeTrabajos>`.
- `main.rs` — entrega el mismo cliente D-Bus como `SystemdClient` + `UnitWatcher` + `ProgresoDeTrabajos`.
- `test_support.rs` — entorno con el doble de trabajos.

Frontend (`frontend/src/`):
- `hooks/useTrabajosEnVivo.ts` — `EventSource` a `/api/v1/jobs/stream`; mantiene por unidad el conjunto de jobs en curso (`jobId → kind`) y deriva `TrabajosEnCurso` (kinds vivos).
- `acciones.ts` — `VERBOS` y `etiquetaDeAccionEnCurso(kind)` (fallback al `kind` crudo).
- `components/TablaDeUnidades/TablaDeUnidades.tsx` — columna «Progreso»: una etiqueta por job en curso.
- `pages/unidades.tsx` — usa el hook y pasa el progreso a la tabla.
- `types/api.ts` — tipos `ProgresoDeJob`/`FaseDeJob`.

## Cómo funciona

1. El cliente D-Bus escucha las señales del manager `JobNew(id, job, unit)` y `JobRemoved(id, job, unit, result)`. Al nacer el job lee su propiedad `JobType` (con un plazo corto; si el job ya desapareció, el tipo queda `unknown`).
2. Publica `ProgresoDeJob` (`new` con el tipo; `done` con el resultado) en su `broadcast`; el puerto `ProgresoDeTrabajos` lo expone.
3. La ruta SSE (bajo el gate) reemite cada transición como evento `job` con envoltura `{data}`, `retry` en el cuadro de apertura, latidos y cierre ordenado por el `watch` de apagado.
4. En el panel, un `EventSource` mantiene por unidad el mapa `jobId → kind`: la tabla muestra una etiqueta «en curso» por job y la retira cuando ese job termina. Sin canal, el panel degrada a su comportamiento anterior.

## Contrato

- `GET /api/v1/jobs/stream` → `text/event-stream`, evento `job`, `data: {"data":{"job":<n>,"unit":"<s>","kind":"<s>","phase":"new"|"done","result":<s|null>}}`. `result` **se omite** en la fase `new`.
- Es **edge-triggered** (no hay instantánea inicial). Solo `GET`; otro método → `405` envuelto. Bajo el gate: en modo `oidc` sin sesión → `401` y no abre el flujo.
- Nota: `JobNew` **no** trae el tipo de job; se lee de la propiedad `JobType`. Si no puede leerse, `kind` es `"unknown"`.

## Configuración / despliegue

- Sin configuración nueva: mismo origen (`/api/v1/jobs/stream`); en dev, Vite proxya `/api`. Mismo bind loopback.
- El panel abre una **segunda** conexión `EventSource` (unidades + trabajos). Coste despreciable en un panel de un usuario.

## Decisiones

- **Señales del manager, no polling**: cubre **todos** los jobs del usuario, incluidos los que el panel no pidió (reinicios automáticos, dependencias).
- **Publicación desde el vigilante, no desde `action()`**: una sola ruta (señal → evento) y cobertura de jobs ajenos.
- **Edge-triggered, sin instantánea**: un job «en curso» es efímero; el estado final lo entrega el canal de unidades.
- **Endpoint propio** (no fundido con el de unidades): estado idempotente y transiciones son preocupaciones distintas.
- **La acción HTTP no cambia** (sigue síncrona); el canal es aditivo. Evita degradar el mapeo de errores a HTTP.
- **`Subscribe()` es por conexión, no por regla**: `suscribir()` tolera `AlreadySubscribed` (arregló un fallo latente que dejaba al vigilante en bucle de reintentos).
- **Carrera del `JobType`**: el tipo se pregunta al nacer; si ya no está, `kind` es `unknown` sin perder el evento.

## Follow-ups

- La **reconexión** del canal se delega al `EventSource` nativo (+ `retry` del servidor); el doble de test no la modela.
- Job **sin `JobRemoved`** (no debería ocurrir): la entrada de `en_curso` se descarta al reconectar el vigilante.
- Varios jobs **simultáneos** sobre la misma unidad: soportado (una etiqueta por job); poco frecuente porque systemd serializa por unidad.
