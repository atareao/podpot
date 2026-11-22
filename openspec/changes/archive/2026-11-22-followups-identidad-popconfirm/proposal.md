# Proposal — followups-identidad-popconfirm

## Why

Al cerrar `identidad-file-name-tabla` quedaron tres deudas menores de coherencia: el diálogo de confirmación de borrado no usa la misma identidad que la fila y los nombres accesibles; un test simula un `file_name` nulo con un cast que contradice el contrato `string`; y el test de orden depende de `indexOf` sobre `textContent`. Esto deja una inconsistencia visible y pruebas frágiles.

## What Changes

- El `Popconfirm` de borrado usará `identidadDe(unidad)` en título y descripción.
- El test de fallback usará `file_name: ''` en lugar de un cast a `null`.
- El test de orden comparará posiciones DOM reales, no índices de cadena.
- Sin cambios de backend, API, base de datos ni tipos públicos.

## Impact

- Affected specs: `ui`
- Affected code: `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx`, `frontend/src/components/TablaDeUnidades/TablaDeUnidades.test.tsx`
- Riesgo: bajo; cambio acotado a presentación y pruebas.
