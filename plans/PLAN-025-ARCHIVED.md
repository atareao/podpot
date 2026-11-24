# PLAN-025 — Presupuesto de tamaño por chunk de Ant Design

## Issue
- GitHub Issue: #56 (Reduce Ant Design bundle size: vendor-antd < 500 kB). Ya existe; NO crear otro.
- PR existente: #57 (`feature/chunk-antd` → `development`), con CI Frontend en rojo por errores de TypeScript en el test de presupuesto. Reutilizar esa rama/PR; NO abrir otra.

## Objetivo
Cada chunk de producción atribuible a Ant Design (`vendor-antd`, `vendor-antd-rc`, cualquier `vendor-antd*`) SHALL pesar menos de 500 kB raw, conservando la dependencia `antd` y todos los componentes actuales. Criterio confirmado por el usuario: «por debajo de 500 kB» se interpreta **por chunk individual**, NO como suma de chunks.

## No objetivo
- NO eliminar `antd` ni `@ant-design/icons`, ni sustituirlos por otra librería.
- NO cambiar la UI ni el comportamiento de los componentes.
- NO externalizar Antd por CDN.
- NO mantener la aserción de suma < 500 kB introducida en la PR #57: queda fuera del alcance del issue.

## Arquitectura y decisiones
- Vite 7: `manualChunks` ya existente reparte `antd` → `vendor-antd`, `rc-*` → `vendor-antd-rc`, React → `vendor-react`, React Query → `vendor-query`.
- Terser con `compress.passes: 2`, `drop_console`, `drop_debugger`, `mangle.safari10` (propuesto ya en PR #57) para reducir el raw de cada chunk.
- Test de presupuesto `frontend/tests/tamanoDelBundle.test.ts` con `// @vitest-environment node`, `build({ mode: 'production', write: false })`, umbral `UMBRAL_RAW = 500 * 1024`, medición `code.length` por chunk y mensaje de fallo con desglose.
- Tipado estructural local sin `any` (Rollup no es dependencia directa con pnpm, así que no se importan sus tipos): tipos mínimos para `output` y discriminante `type === 'chunk'`. `testTimeout: 60000` en la config de Vitest.

## Tareas (TDD)
- [x] 1. Change OpenSpec `antd-bundle-budget` con proposal + spec + tasks; esperar aprobación del usuario antes de código o tests.
- [x] 2. RED: sustituir el test de la PR #57 por la versión tipada, con presupuesto por chunk y sin aserción de suma; ejecutar `npx vitest run tests/tamanoDelBundle.test.ts` y verificar fallo esperado.
- [x] 3. GREEN: ajustar `frontend/vite.config.ts` (Terser, manualChunks estable, timeout) y el test hasta verde; ejecutar `npx vitest run` y `npx tsc --noEmit`.
- [x] 4. REFACTOR: `npm run lint`, `npx tsc --noEmit`, build de producción; sin cambiar comportamiento de UI.
- [x] 5. Auditoría `@react-reviewer`; aplicar hallazgos y re-verificar.
- [x] 6. Actualizar PR #57 → `development`; CI Backend y Frontend en verde; merge; cerrar issue #56 y PR #57.
- [x] 7. Crear `docs/antd-bundle-budget.md`, marcar tareas del plan, renombrar a `plans/PLAN-025-ARCHIVED.md`, `openspec archive` (o ajuste manual), y corregir `HANDOFF.md` (la Deuda 2 figuraba cerrada prematuramente).

## Verificación final
- `npx vitest run`, `npx tsc --noEmit`, `npm run lint`, build de producción.
- `gh pr checks 57` en verde y `development` limpio.
- Ningún chunk `vendor-antd*` con tamaño raw ≥ 500 * 1024 bytes; `antd` sigue en `package.json`.
