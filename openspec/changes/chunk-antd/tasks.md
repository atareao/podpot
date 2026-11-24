# TDD task checklist: chunk-antd

## RED
- [ ] `frontend/tests/tamanoDelBundle.test.ts`: build programático (`write: false`)
      y aserto de que la suma de chunks `vendor-antd*` ≤ 500 kB raw (y cada uno < 500 kB).
- [ ] Confirmar en CLI que el test FALLA hoy (~834 kB) y que el resto de la suite queda verde.

## GREEN
- [ ] Recortar superficie antd no usada (imports muertos).
- [ ] Sustituir componentes pesados (Table, Form, Select, Drawer) por equivalentes
      ligeros manteniendo los tests de UI en verde (adaptación mínima documentada).
- [ ] Ajustar `vite.config.ts` solo si el reparto de chunks lo requiere.
- [ ] `pnpm test` (suite completa) verde + test de tamaño verde.

## REFACTOR
- [ ] `pnpm run lint` limpio; `tsc -b` sin errores.
- [ ] Segundo build consecutivo con hashes estables en los chunks de dependencias.
- [ ] `docs/<feature>.md` + cierre: renombrar plan a `plans/PLAN-025-ARCHIVED.md`,
      archivar change (`openspec archive chunk-antd`), cerrar issue #56.
