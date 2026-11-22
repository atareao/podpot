# Tasks — followups-identidad-popconfirm

## 1. Baseline

- [x] 1.1 Ejecutar el test focalizado de `TablaDeUnidades` y confirmar verde.
- [x] 1.2 Confirmar rama limpia y crear `feature/followups-identidad-popconfirm` desde `development`.

## 2. RED

- [x] 2.1 Test del `Popconfirm` con título y descripción basados en `identidadDe(unidad)`.
- [x] 2.2 Sustituir el cast `null as unknown as string` por `file_name: ''` y probar el fallback.
- [x] 2.3 Endurecer el test de orden con comparación DOM.
- [x] 2.4 Ejecutar tests y confirmar los fallos esperados.

## 3. GREEN

- [x] 3.1 Usar `identidadDe(unidad)` en título y descripción del `Popconfirm`.
- [x] 3.2 Ejecutar tests focalizados y confirmar verde.

## 4. REFACTOR y verificación

- [x] 4.1 Revisar casts y usos en crudo de `file_name` en la UI.
- [x] 4.2 Ejecutar `pnpm --dir frontend vitest run`.
- [x] 4.3 Ejecutar typecheck y lint del frontend.
- [x] 4.4 Ejecutar `just check-all`.

## 5. Cierre

- [x] 5.1 Crear `docs/followups-identidad-popconfirm.md`.
- [ ] 5.2 Actualizar `HANDOFF.md`.
- [ ] 5.3 Commit gitmoji, push y PR a `development`.
- [ ] 5.4 Archivar el change y el plan tras el merge.
