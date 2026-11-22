# Tasks — toast-identidad-borrado

## 1. Baseline

- [x] 1.1 Ejecutar `pages/unidades.test.tsx` y confirmar verde.
- [x] 1.2 Crear la rama `feature/toast-identidad-borrado` desde `development`.

## 2. RED

- [x] 2.1 Actualizar las expectativas del toast para exigir el `file_name`.
- [x] 2.2 Añadir cobertura del fallback con `file_name: ''`.
- [x] 2.3 Ejecutar tests y confirmar los fallos esperados.

## 3. GREEN

- [x] 3.1 Extraer `identidadDe` a `frontend/src/identidad.ts`.
- [x] 3.2 Usar `identidadDe` en el toast de borrado.
- [x] 3.3 Ejecutar tests focalizados y confirmar verde.

## 4. REFACTOR y verificación

- [x] 4.1 Eliminar el warning de `react-refresh` moviendo la utilidad a `identidad.ts`.
- [x] 4.2 Ejecutar `pnpm --dir frontend vitest run`.
- [x] 4.3 Ejecutar typecheck y lint del frontend.
- [x] 4.4 Ejecutar `just check-all`.

## 5. Cierre

- [x] 5.1 Crear `docs/toast-identidad-borrado.md`.
- [x] 5.2 Actualizar `HANDOFF.md`.
- [x] 5.3 Commit gitmoji, push y PR a `development`.
- [x] 5.4 Archivar el change y el plan tras el merge.
