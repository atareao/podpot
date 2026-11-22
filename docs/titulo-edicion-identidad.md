# Título de edición con la identidad publicada

## Qué es

Cierre del follow-up #34: el título de la tarjeta de edición usa la misma identidad que la fila, el `Popconfirm` y los avisos, es decir, el `file_name` publicado por la API. Antes usaba `enEdicion.name`.

- Issue: #34
- Change: `titulo-edicion-identidad`
- Plan: `plans/PLAN-017.md`

## Ficheros implicados

- `frontend/src/pages/unidades.tsx`
- `frontend/src/pages/unidades.test.tsx`

## Cómo funciona

- El título de la tarjeta usa `identidadDe(enEdicion)` (`file_name || name`).
- `enCurso` sigue usando `enEdicion.name` porque el bloqueo de acciones casa por nombre de unidad.
- El test abre la edición de una quadlet con `name: 'web.service'` y `file_name: 'web.container'` y exige `Editar quadlet «web.container»`.

## Variables y configuración

No aplica.

## Decisiones

- No se cambia `enCurso`: su clave es el nombre de unidad, igual que el indicador de progreso y la API de acciones.
- Se reutiliza `identidadDe`; no se duplica la lógica.

## Verificación

- `just check-all` verde: 140/140 tests, typecheck, lint y build.
- Test focalizado de `pages/unidades.test.tsx` en verde.

## Follow-ups

- Valorar un comentario en `enCurso` que explique por qué casa por nombre de unidad.
- Revisar otros textos de UI que aún usen `enEdicion.name` si aparecen en el futuro.
