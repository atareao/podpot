# Proposal — Agrupación de quadlets por stack

## Why

El listado de unidades del panel es plano: `foo.container`, `foo-volume.service` y `foo-network.service` aparecen como tres filas independientes aunque formen una sola aplicación. El usuario no puede ver de un vistazo qué piezas pertenecen a cada stack, y no hay forma de filtrar por tipo de recurso ni por estado sin leer cada fila.

## What Changes

- El backend deriva un identificador `stack` determinista para cada `UnitSummary` a partir de `file_name` y `kind`, sin escanear directorios ni consultar metadatos nuevos.
- `GET /api/v1/units` pasa a incluir el campo `stack: String` (cambio aditivo).
- El espejo TypeScript de `UnitSummary` incluye `stack`.
- La tabla de unidades se agrupa por stack con una cabecera por grupo, mantiene el estado en vivo y añade filtros secundarios por tipo (`kind`) y estado (`active_state`), combinables.
- El orden de stacks y de unidades dentro de cada stack es determinista para que los eventos SSE no reordenen filas.

## Capabilities

### New Capabilities

- `agrupacion-por-stack`: regla de derivación del identificador de stack y su representación agrupada en la UI.

### Modified Capabilities

- `unidades-en-vivo`: el contrato de `UnitSummary` incorpora `stack`.
- `ui`: el listado de unidades pasa de fila plana a grupos por stack con filtros.

## Impact

- Backend: nuevo módulo puro de derivación, cambio aditivo en `UnitSummary` y en el handler de listado. Sin migración SQLite.
- Frontend: nuevo campo en el tipo, helper puro de agrupación y cambios en `<TablaDeUnidades>`. El cliente `listarUnidades()` no cambia de forma.
- Sin impacto en creación/edición de quadlets, autenticación, registros ni despliegue.
- Issue: #62. Rama: `feature/agrupacion-por-stack`. Plan: `plans/PLAN-027.md`.
