# Tasks — identidad-file-name-tabla

Refactor de presentación: la red de seguridad son los tests de `TablaDeUnidades` y el resto de la suite de frontend.

## 1. Baseline (estado de partida)

- [x] 1.1 Confirmar la suite de frontend en verde (`pnpm test` en `frontend/`) antes de tocar nada.

## 2. Spec (SDD)

- [x] 2.1 Change OpenSpec `identidad-file-name-tabla` creado y aprobado por el usuario antes de tocar código.

## 3. TDD — RED

- [x] 3.1 Añadir/ajustar tests en `TablaDeUnidades.test.tsx`: la fila muestra `file_name` como identidad primaria con `name` secundario, y los `aria-label` de acciones y progreso usan `file_name`. — Verificación: los tests nuevos fallan.

## 4. TDD — GREEN

- [x] 4.1 Implementar en `TablaDeUnidades.tsx` la identidad mostrada por `file_name` (columna + nombres accesibles), conservando las claves internas de progreso y acción en curso por nombre de unidad. — Verificación: tests en verde.

## 5. REFACTOR y verificación

- [x] 5.1 `npx tsc --noEmit` y lint verdes.
- [x] 5.2 `just check-all` EXIT 0.
- [x] 5.3 Auditoría `@react-reviewer` sin bloqueantes.
- [x] 5.4 `docs/identidad-file-name-tabla.md` creado; plan renombrado a `plans/PLAN-013-ARCHIVED.md`; PR a `development` referenciando el issue #27.
