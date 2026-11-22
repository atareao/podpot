# Tasks — toasts-identidad

## 1. Baseline

- [x] 1.1 Ejecutar el test de `pages/unidades.test.tsx` y confirmar verde.
- [x] 1.2 Crear la rama `feature/toasts-identidad` desde `development`.

## 2. RED

- [x] 2.1 Actualizar las expectativas de los avisos de acción, creación y edición para exigir `web.container`.
- [x] 2.2 Ejecutar el test y confirmar los fallos esperados.

## 3. GREEN

- [x] 3.1 Acción: usar `identidadDe(unidad)` y texto «Quadlet».
- [x] 3.2 Creación: usar `creada.file_name`.
- [x] 3.3 Edición: usar `identidadDe(enEdicion)`.
- [x] 3.4 Ejecutar el test focalizado y confirmar verde.

## 4. REFACTOR y verificación

- [x] 4.1 Revisar que no quedan avisos con `name`/`unit_name` en crudo.
- [x] 4.2 Ejecutar `pnpm --dir frontend vitest run`.
- [x] 4.3 Ejecutar typecheck y lint.
- [x] 4.4 Ejecutar `just check-all`.

## 5. Cierre

- [x] 5.1 Crear `docs/toasts-identidad.md`.
- [ ] 5.2 Actualizar `HANDOFF.md`.
- [ ] 5.3 Commit gitmoji, push y PR a `development`.
- [ ] 5.4 Archivar el change y el plan tras el merge.
