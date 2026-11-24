# antd-bundle-budget — presupuesto de tamaño por chunk de Ant Design

## Qué es
Optimización del bundle de producción del panel para que cada chunk atribuible a Ant Design (`vendor-antd`, `vendor-antd-rc`) quede por debajo de 500 kB raw, **sin eliminar ni sustituir Ant Design**. Cierra el issue #56 (deuda 2) vía PR #57.

## Ficheros implicados
- `frontend/vite.config.ts`: `manualChunks` (antd → `vendor-antd`, `rc-*` → `vendor-antd-rc`, React → `vendor-react`, React Query → `vendor-query`), Terser con `compress.passes: 2`, `drop_console`, `drop_debugger`, `mangle.safari10`, y `testTimeout: 60000`.
- `frontend/tests/tamanoDelBundle.test.ts`: test de presupuesto tipado (sin `any`), con casos de dependencias, build real y chunk simulado en el umbral.
- `frontend/package.json` / `pnpm-lock.yaml`: nueva devDependency `terser@^5.51.4`.
- `openspec/changes/antd-bundle-budget/`: propuesta, spec y tareas.
- `plans/PLAN-025.md`: plan referenciado al issue #56.

## Cómo funciona
- El umbral es `500 * 1024` bytes **por chunk individual**, no sobre la suma. Este criterio fue confirmado por el usuario porque el issue citaba tamaños individuales (~455 kB y ~379 kB).
- El test hace `build({ mode: 'production', write: false })` con Vite, normaliza `RollupOutput | RollupOutput[]`, filtra por `type === 'chunk'` y por el último segmento del `fileName` que empieza por `vendor-antd`, y falla con un mensaje que incluye nombre de chunk y kB si alguno alcanza el umbral.
- Comprueba además que `antd` y `@ant-design/icons` siguen en `dependencies`.
- Tamaños medidos tras el cambio: `vendor-antd` 458 246 bytes (447,51 kB) y `vendor-antd-rc` 372 669 bytes (363,93 kB).

## Configuración / variables
- `UMBRAL_RAW = 500 * 1024` en el test.
- `chunkSizeWarningLimit` de Vite no se ha cambiado; el presupuesto lo impone el test.
- No hay variables de entorno nuevas.

## Cómo verificarlo
En `frontend/`: `npx vitest run`, `npx tsc --noEmit`, `npx tsc -p tsconfig.test.json --noEmit`, `npx eslint .`, `pnpm build` y revisar `dist/assets/vendor-antd*.js` con `wc -c`. La CI ejecuta los mismos pasos (job Frontend de `.github/workflows/ci.yml`).

## Decisiones
- Mantener `antd` y todos sus componentes (`Form`, `Input`, `Select`, `Space`, `Table`, `Typography`): la PR #57 había intentado sustituirlos por HTML nativo y fue rechazada por la auditoría; se restauraron desde `development`.
- No añadir `rollup` como dependencia solo para importar tipos: el test usa un tipado estructural local sin `any`.
- No externalizar Antd por CDN.

## Follow-ups
- Si una actualización de Antd eleva algún chunk por encima del umbral, el test fallará en CI: revisar imports y configuración de `manualChunks`, nunca sustituir la librería.
- El test de dependencias no comprueba los imports desde `antd` en los componentes; los tests de los propios componentes cubren ese uso.
