# Proposal — titulo-edicion-identidad

## Why

La fila, el `Popconfirm`, los avisos y el toast de borrado ya usan el `file_name` publicado. El título de la tarjeta de edición sigue usando `enEdicion.name`, así que el usuario ve dos identidades distintas para la misma quadlet.

## What Changes

- El título de la tarjeta de edición usa `identidadDe(enEdicion)`.
- Se añade un test que fija el título con el `file_name`.
- Sin cambios de backend, API ni tipos.

## Impact

- Affected specs: `ui`
- Affected code: `frontend/src/pages/unidades.tsx`, `frontend/src/pages/unidades.test.tsx`
- Riesgo: bajo; cambio acotado a un título y su prueba.
