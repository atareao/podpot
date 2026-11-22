# Toast de borrado con la identidad publicada

## Qué es

Cierre del follow-up #30: el toast de éxito del borrado usa la misma identidad que la fila y el `Popconfirm`, es decir, el `file_name` publicado por la API. Antes usaba `unidad.name` y mostraba un nombre distinto al de la fila.

- Issue: #30
- Change OpenSpec: `toast-identidad-borrado`
- Plan: `plans/PLAN-015.md`

## Ficheros implicados

- `frontend/src/identidad.ts` (nuevo): módulo puro con `identidadDe`.
- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx`: importa la utilidad.
- `frontend/src/pages/unidades.tsx`: usa la utilidad en el toast de borrado.
- `frontend/src/pages/unidades.test.tsx`: expectativas del toast y fallback.

## Cómo funciona

- `identidadDe(unidad)` devuelve `unidad.file_name || unidad.name`.
- La fila, los `aria-label`, el `Popconfirm` y el toast de borrado usan la misma función.
- El fallback a `name` solo aplica si `file_name` es cadena vacía; el contrato de la API es `file_name: string` no nulo.
- Las peticiones `GET`/`DELETE` siguen usando `unidad.file_name` como clave de la API.

## Variables y configuración

No aplica: cambio solo de frontend, sin nuevas variables de entorno ni configuración.

## Cómo habilitar o desplegar

No requiere despliegue específico. Se publica con el build normal del frontend (`pnpm --dir frontend build`).

## Decisiones

- Se extrae `identidadDe` a `frontend/src/identidad.ts` para evitar el warning `react-refresh/only-export-components` que provocaba exportar una función no-componente desde `TablaDeUnidades.tsx`.
- El módulo solo importa el tipo `QuadletUnit`; no crea ciclos.
- No se cambian los toasts de crear/editar/acción; quedan como follow-up separado.

## Verificación

- `pnpm --dir frontend vitest run` → suite completa verde.
- `pnpm --dir frontend exec tsc --noEmit` → sin errores.
- `pnpm --dir frontend lint` → sin errores ni warnings.
- `just check-all` → verde, incluido build.

## Follow-ups

- Toasts de crear/editar/acción: revisar si deben mostrar el `file_name` en lugar del nombre de unidad.
- Endurecer el contrato si el fallback defensivo con `file_name` vacío deja de ser necesario.
