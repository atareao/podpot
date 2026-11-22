# PLAN-016 — Toasts con la identidad publicada

- **Issue:** #32 (https://github.com/atareao/podpot/issues/32)
- **Change OpenSpec:** `toasts-identidad`
- **Rama:** `feature/toasts-identidad` desde `development`
- **Alcance:** solo frontend. Sin cambios de backend, API ni base de datos.

## Objetivo

Alinear los avisos de crear, editar y accionar con la identidad publicada por la API (`file_name`), igual que la fila, el `Popconfirm` y el toast de borrado.

## Contexto y decisiones

- `identidadDe` vive en `frontend/src/identidad.ts` y devuelve `file_name || name`.
- En creación, la respuesta `QuadletCreada` ya trae `file_name`; se usará directamente.
- En edición, se usará `identidadDe(enEdicion)` porque `enEdicion` es la `QuadletUnit` de la fila.
- En acción, se usará `identidadDe(unidad)` porque la respuesta `ResultadoDeAccion` solo trae el nombre de unidad.
- El texto del aviso de acción pasa de «Unidad» a «Quadlet» para ser coherente con la identidad mostrada y con los demás avisos.
- Sin cambios de backend.

## Archivos implicados

- `frontend/src/pages/unidades.tsx`
- `frontend/src/pages/unidades.test.tsx`

## Tareas (SDD + TDD)

### 1. Baseline

- [ ] Ejecutar el test de `pages/unidades.test.tsx` y confirmar verde.
- [ ] Confirmar rama limpia y crear `feature/toasts-identidad` desde `development`.

### 2. RED

- [ ] Actualizar las expectativas de los avisos de acción, creación y edición para exigir `web.container` en lugar de `web.service`.
- [ ] Ejecutar el test y confirmar los fallos esperados.

### 3. GREEN

- [ ] Acción: usar `identidadDe(unidad)` y texto «Quadlet».
- [ ] Creación: usar `creada.file_name`.
- [ ] Edición: usar `identidadDe(enEdicion)`.
- [ ] Ejecutar el test focalizado y confirmar verde.

### 4. REFACTOR y verificación

- [ ] Revisar que no quedan avisos con `name`/`unit_name` en crudo.
- [ ] Ejecutar `pnpm --dir frontend vitest run`.
- [ ] Ejecutar typecheck y lint.
- [ ] Ejecutar `just check-all`.

### 5. Cierre

- [ ] Crear `docs/toasts-identidad.md`.
- [ ] Actualizar `HANDOFF.md`.
- [ ] Commit gitmoji, push y PR a `development`.
- [ ] Archivar el change y el plan tras el merge.

## Criterios de aceptación

- [ ] Los avisos de crear, editar y accionar identifican la quadlet con su `file_name`.
- [ ] El aviso de acción usa «Quadlet» y no «Unidad».
- [ ] Tests, typecheck, lint y `just check-all` en verde.
- [ ] Sin cambios de backend.
