# PLAN-017 — Título de edición con la identidad publicada

- **Issue:** #34 (https://github.com/atareao/podpot/issues/34)
- **Change OpenSpec:** `titulo-edicion-identidad`
- **Rama:** `feature/titulo-edicion-identidad` desde `development`
- **Alcance:** solo frontend. Sin cambios de backend.

## Objetivo

Alinear el título de la tarjeta de edición con la identidad publicada: hoy usa `enEdicion.name`; debe usar `identidadDe(enEdicion)` (`file_name`).

## Contexto y decisiones

- `identidadDe` vive en `frontend/src/identidad.ts`.
- El título de la tarjeta de edición (`pages/unidades.tsx`) debe mostrar el `file_name`, igual que la fila, el `Popconfirm` y los avisos.
- `enCurso` seguirá usando `enEdicion.name` para casar el bloqueo de acciones por nombre de unidad; no se toca.
- Sin cambios de backend.

## Archivos implicados

- `frontend/src/pages/unidades.tsx`
- `frontend/src/pages/unidades.test.tsx`

## Tareas (SDD + TDD)

### 1. Baseline

- [ ] Ejecutar `pages/unidades.test.tsx` y confirmar verde.
- [ ] Crear la rama `feature/titulo-edicion-identidad` desde `development`.

### 2. RED

- [ ] Añadir un test que abra la edición de una quadlet con `name: 'web.service'` y `file_name: 'web.container'` y exija el título `Editar quadlet «web.container»`.
- [ ] Ejecutar el test y confirmar que falla porque el título usa `enEdicion.name`.

### 3. GREEN

- [ ] Usar `identidadDe(enEdicion)` en el título de la tarjeta de edición.
- [ ] Ejecutar el test focalizado y confirmar verde.

### 4. REFACTOR y verificación

- [ ] Revisar que no queda `enEdicion.name` en textos de UI.
- [ ] Ejecutar `pnpm --dir frontend vitest run`.
- [ ] Ejecutar typecheck y lint.
- [ ] Ejecutar `just check-all`.

### 5. Cierre

- [ ] Crear `docs/titulo-edicion-identidad.md`.
- [ ] Actualizar `HANDOFF.md`.
- [ ] Commit gitmoji, push y PR a `development`.
- [ ] Archivar el change y el plan tras el merge.

## Criterios de aceptación

- [ ] El título de edición identifica la quadlet con su `file_name`.
- [ ] `enCurso` sigue casando por nombre de unidad.
- [ ] Tests, typecheck, lint y `just check-all` en verde.
- [ ] Sin cambios de backend.
