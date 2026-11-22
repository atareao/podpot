# Tasks — titulo-edicion-identidad

## 1. Baseline

- [x] 1.1 Ejecutar `pages/unidades.test.tsx` y confirmar verde.
- [x] 1.2 Crear la rama `feature/titulo-edicion-identidad` desde `development`.

## 2. RED

- [x] 2.1 Añadir test del título de edición con `file_name`.
- [x] 2.2 Ejecutar el test y confirmar el fallo esperado.

## 3. GREEN

- [x] 3.1 Usar `identidadDe(enEdicion)` en el título de la tarjeta.
- [x] 3.2 Ejecutar el test focalizado y confirmar verde.

## 4. REFACTOR y verificación

- [x] 4.1 Revisar que no queda `enEdicion.name` en textos de UI.
- [x] 4.2 Ejecutar `pnpm --dir frontend vitest run`.
- [x] 4.3 Ejecutar typecheck y lint.
- [x] 4.4 Ejecutar `just check-all`.

## 5. Cierre

- [x] 5.1 Crear `docs/titulo-edicion-identidad.md`.
- [ ] 5.2 Actualizar `HANDOFF.md`.
- [ ] 5.3 Commit gitmoji, push y PR a `development`.
- [ ] 5.4 Archivar el change y el plan tras el merge.
