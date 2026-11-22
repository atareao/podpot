# Proposal

## Why

`GET /api/v1/units` publica ya `file_name` y `kind` (issue #6): la identidad real de una quadlet es `file_name` (basename del `SourcePath`, `<stem>.<kind>`). La spec `ui` ya exige usar esa identidad y las acciones de editar/borrar ya la usan. Pero la tabla `TablaDeUnidades` sigue **mostrando** cada fila identificada por `name` (columna «Unidad» y `aria-label` de las acciones y del progreso). Resultado: asimetría entre la identidad que la tabla muestra y la identidad con la que opera el resto del sistema.

## What Changes

- La fila de `TablaDeUnidades` se identifica **visualmente** por `file_name` como dato primario, con el nombre de unidad (`name`) como dato secundario visible.
- Los `aria-label` de las acciones y de la región de progreso usan el `file_name`.
- Las claves internas de progreso y de acción en curso (`progresoPorUnidad`, `enCurso`) se **mantienen por nombre de unidad**, que es lo que publican el canal de trabajos y la API de acciones; NO se reindexan por `file_name`.
- Sin cambios en el backend ni en el contrato de la API.

## Capabilities

### Modified Capabilities

- `ui`: se precisa que la identidad primaria **mostrada** de la fila del listado es el `file_name`, con el nombre de unidad como dato secundario, y que los nombres accesibles usan esa identidad, sin cambiar las claves internas de progreso.

## Impact

- Frontend (`frontend/`): `src/components/TablaDeUnidades/TablaDeUnidades.tsx` y `src/components/TablaDeUnidades/TablaDeUnidades.test.tsx`. Sin cambios en backend, contrato de API ni despliegue.
