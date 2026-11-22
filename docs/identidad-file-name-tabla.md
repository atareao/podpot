# identidad-file-name-tabla

## Qué es

Refactor de presentación en el panel: la tabla de quadlets (`TablaDeUnidades`) identifica cada fila por su **`file_name`** (la identidad real de la quadlet publicada por la API) como dato primario, dejando el **nombre de unidad systemd** (`name`) como dato secundario. Antes, la fila se identificaba por `name`, lo que generaba una asimetría con el resto del sistema (edición/borrado ya operaban por `file_name`).

Issue: #27 · Change OpenSpec: `openspec/changes/archive/2026-11-22-identidad-file-name-tabla/` (capability `ui`).

## Ficheros implicados

- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx` — columna «Unidad» con `render` (identidad primaria + `name` secundario en `Typography.Text type="secondary"`); helper de módulo `identidadDe(unidad) = unidad.file_name || unidad.name`; `aria-label` de los 6 botones de acción y de la región `role="status"` usan `identidadDe`.
- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.test.tsx` — tests de identidad primaria/secundaria, nombres accesibles, progreso, bloqueo por `enCurso` y fallback.
- `frontend/src/pages/unidades.test.tsx` — queries de botón actualizadas a `file_name`.

## Cómo funciona

- **Identidad mostrada**: `identidadDe(unidad)` devuelve `file_name`; si es nulo o vacío, cae a `name` (el tipo actual de `file_name` es `string`, así que la caída es defensa ante entradas inesperadas).
- **Nombres accesibles**: acciones y región de progreso se anuncian con la identidad mostrada, de modo que con nombres de unidad colisionados cada fila sigue distinguiéndose.
- **Claves internas intactas**: `enCurso === unidad.name` y `progresoPorUnidad[unidad.name]` siguen por **nombre de unidad**, porque la acción HTTP envía `{ name: unidad.name }` (`pages/unidades.tsx`) y el canal SSE de jobs (`useTrabajosEnVivo`) indexa por el campo `unit`. Reindexarlas por `file_name` rompería el bloqueo de botones y el indicador de progreso.

## Variables / configuración

Ninguna. Sin cambios de contrato de API ni de backend.

## Cómo habilitar / desplegar

Ninguno específico: el build de producción (`pnpm build`) ya lo incluye; queda por debajo del umbral de aviso de chunk.

## Decisiones

- La identidad por `file_name` es **visual y de accesibilidad**; no se propagó a las claves internas de progreso/acción (ver arriba). Se documentó en la spec `ui` con el escenario «Estado de acción en curso por nombre de unidad».
- Fallback a `name` cuando `file_name` no está disponible.

## Follow-ups

- `TablaDeUnidades.tsx`: la `description` del `Popconfirm` de borrado usa `unidad.file_name` en crudo (no `identidadDe`). Pre-existente; unificar en un cambio posterior.
- Contrato de tipos: `QuadletUnit.file_name` está tipado `string` (no nulable) mientras el test de fallback usa un cast. Alinear el tipo del espejo con el contrato real del backend.
- Robustez de test: la comprobación de orden primario/secundario vía `textContent.indexOf` podría migrarse a `within(...)` con queries semánticas.
