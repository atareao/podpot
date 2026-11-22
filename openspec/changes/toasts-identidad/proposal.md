# Proposal — toasts-identidad

## Why

La fila, el `Popconfirm` y el toast de borrado ya usan el `file_name` publicado por la API. Los avisos de crear, editar y accionar siguen mostrando el nombre de unidad, así que el usuario ve dos identidades distintas para la misma quadlet.

## What Changes

- El aviso de acción usa `identidadDe(unidad)` y el texto «Quadlet».
- El aviso de creación usa `creada.file_name`.
- El aviso de edición usa `identidadDe(enEdicion)`.
- Se actualizan las expectativas de los tests.
- Sin cambios de backend, API ni tipos.

## Impact

- Affected specs: `ui`
- Affected code: `frontend/src/pages/unidades.tsx`, `frontend/src/pages/unidades.test.tsx`
- Riesgo: bajo; cambio acotado a textos de avisos y pruebas.
