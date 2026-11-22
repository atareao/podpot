# Proposal — toast-identidad-borrado

## Why

El `Popconfirm` de borrado ya usa la identidad publicada por la API, pero el toast de éxito sigue usando `unidad.name`. El usuario ve «web.container» en la fila y la confirmación, y «web.service» en el aviso posterior. Es una inconsistencia de identidad visible y una deuda pendiente del cambio anterior.

## What Changes

- Extraer `identidadDe` a `frontend/src/identidad.ts`, reutilizable y sin warning de `react-refresh`.
- Usar `identidadDe(unidad)` en el toast de borrado de `pages/unidades.tsx`.
- Actualizar las expectativas de los tests del toast y añadir cobertura del fallback con `file_name: ''`.
- Sin cambios de backend, API, base de datos ni tipos públicos.

## Impact

- Affected specs: `ui`
- Affected code: `frontend/src/identidad.ts` (nuevo), `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx`, `frontend/src/pages/unidades.tsx`, `frontend/src/pages/unidades.test.tsx`
- Riesgo: bajo; cambio acotado a un aviso y sus pruebas.
