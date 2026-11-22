# Toasts con la identidad publicada

## Qué es

Cierre del follow-up #32: los avisos de crear, editar y accionar usan la misma identidad que la fila y el `Popconfirm`, es decir, el `file_name` publicado por la API. Antes mostraban el nombre de unidad.

- Issue: #32
- Change OpenSpec: `toasts-identidad`
- Plan: `plans/PLAN-016.md`

## Ficheros implicados

- `frontend/src/pages/unidades.tsx`
- `frontend/src/pages/unidades.test.tsx`

## Cómo funciona

- Acción: `identidadDe(unidad)` y texto «Quadlet».
- Creación: `creada.file_name` de la respuesta de la API.
- Edición: `identidadDe(enEdicion)`.
- Borrado: sigue usando `identidadDe(unidad)`.
- Los errores siguen mostrando `error.message` sin cambios.

## Variables y configuración

No aplica: cambio solo de frontend, sin nuevas variables de entorno.

## Cómo habilitar o desplegar

No requiere despliegue específico. Se publica con el build normal del frontend.

## Decisiones

- Se reutiliza `identidadDe` de `frontend/src/identidad.ts`; no se duplica la lógica.
- El aviso de acción cambia de «Unidad» a «Quadlet» para coincidir con la identidad mostrada y con el resto de avisos.
- La respuesta de acción solo trae el nombre de unidad, por eso se usa la `QuadletUnit` de la fila como fuente del `file_name`.

## Verificación

- `pnpm --dir frontend vitest run src/pages/unidades.test.tsx` → 27/27.
- `just check-all` → verde, incluido build.
- Typecheck y lint sin errores.

## Follow-ups

- Título de la tarjeta de edición: sigue usando `enEdicion.name`; issue #34.
- Valorar tests con `file_name` no derivable del nombre de unidad para acción/edición.
