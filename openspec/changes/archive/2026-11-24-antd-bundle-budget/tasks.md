## 1. Preparación
- [x] 1.1 Confirmar issue #56 y change `antd-bundle-budget` aprobado por el usuario
- [x] 1.2 Crear `plans/PLAN-025.md` referenciando #56

## 2. RED
- [x] 2.1 Sustituir el test de la PR #57 por la versión tipada, con presupuesto por chunk y sin aserción de suma
- [x] 2.2 Ejecutar `npx vitest run tests/tamanoDelBundle.test.ts` y verificar el fallo esperado

## 3. GREEN
- [x] 3.1 Ajustar `frontend/vite.config.ts` (Terser + `manualChunks` estable + `testTimeout`)
- [x] 3.2 Ejecutar `npx vitest run` y `npx tsc --noEmit` hasta verde

## 4. REFACTOR
- [x] 4.1 `npm run lint`, build de producción y verificación de tamaños por chunk

## 5. Auditoría y cierre
- [x] 5.1 Auditoría `@react-reviewer` y aplicar hallazgos
- [x] 5.2 Actualizar PR #57, CI en verde, merge a `development`, cerrar #56
- [x] 5.3 `docs/antd-bundle-budget.md`, plan archivado, `openspec archive` o ajuste manual, corregir `HANDOFF.md`
