# Follow-ups de identidad en la tabla de unidades

## Qué es

Cierre de tres deudas de identidad detectadas al archivar `identidad-file-name-tabla`: el `Popconfirm` de borrado no usaba la misma identidad que la fila y los nombres accesibles, un test simulaba un `file_name` nulo con un cast que contradice el contrato, y el test de orden dependía de `indexOf` sobre `textContent`.

- Issue: #29
- Change OpenSpec: `followups-identidad-popconfirm`
- Plan: `plans/PLAN-014.md`

## Ficheros implicados

- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx`
- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.test.tsx`
- `frontend/src/pages/unidades.test.tsx`

## Cómo funciona

- `identidadDe(unidad)` devuelve `unidad.file_name || unidad.name` y es la única fuente de la identidad mostrada y accesible.
- El `Popconfirm` de borrado usa `identidadDe(unidad)` en el título y en la descripción, igual que la celda «Unidad» y los `aria-label`.
- El fallback a `name` es defensivo y solo aplica si `file_name` es una cadena vacía. El contrato de la API es `file_name: string` no nulo.
- El test de orden compara posiciones DOM reales (`compareDocumentPosition`), no índices de cadena.
- No hay cambios de backend, API, base de datos ni tipos públicos.

## Variables y configuración

No aplica: cambio solo de frontend, sin nuevas variables de entorno ni configuración.

## Cómo habilitar o desplegar

No requiere despliegue específico. Se publica con el build normal del frontend (`pnpm --dir frontend build`).

## Decisiones

- Mantener `identidadDe` como única fuente de identidad evita divergencias entre fila, botones y confirmación.
- No se cambia el tipo `QuadletUnit.file_name`: ya es `string`; el problema era el cast del test.
- No se amplía el alcance al toast de éxito de borrado (`pages/unidades.tsx`), que sigue usando `unidad.name`; queda como follow-up separado.

## Verificación

- `pnpm --dir frontend vitest run` → 138/138
- `pnpm --dir frontend exec tsc --noEmit` → sin errores
- `pnpm --dir frontend lint` → sin errores
- `just check-all` → verde, incluido build

## Follow-ups

- Toast de borrado: `pages/unidades.tsx` usa `unidad.name`; alinearlo con `identidadDe(unidad)`.
- Revisar si el fallback defensivo debe compartirse con las llamadas HTTP o si el contrato no nulo hace innecesario ese caso.
