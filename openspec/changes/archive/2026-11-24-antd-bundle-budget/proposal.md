# Change: Presupuesto de tamaño por chunk de Ant Design

## Why
El issue #56 pide que los chunks vendor de Ant Design estén por debajo de 500 kB. La PR #57 intentó imponer un presupuesto sobre la suma de chunks (~834 kB), criterio más estricto que el enunciado, y su test no pasa `typecheck` (`RollupOutput | RollupWatcher`, parámetros `any` implícitos). El handoff marcó la deuda como cerrada sin que la PR se hubiera mergeado. El usuario ha confirmado que el umbral se aplica **por chunk individual**.

## What Changes
- Mantener `antd` y `@ant-design/icons` como dependencias y conservar los componentes usados por el panel; NO se eliminan ni se sustituyen.
- Consolidar `manualChunks` para que el código de antd y sus paquetes `rc-*` sigan cayendo en chunks `vendor-antd*`.
- Configurar Terser con 2 pasadas de compresión y eliminación de `console`/`debug` en producción.
- Corregir `frontend/tests/tamanoDelBundle.test.ts`: tipado estructural local (sin `any`, sin añadir `rollup` como dependencia), medición por chunk individual, umbral de `500 * 1024` bytes raw, mensaje de fallo con desglose y timeout de 60 s.
- Eliminar la comprobación de suma de tamaños introducida en la PR #57, por estar fuera del alcance del issue.

## Impact
- Ficheros: `frontend/vite.config.ts`, `frontend/package.json` (+ `terser`), `frontend/pnpm-lock.yaml`, `frontend/tests/tamanoDelBundle.test.ts`.
- CI Frontend: el test compila con TypeScript estricto y el build de producción respeta el presupuesto por chunk.
- Sin cambios en la API Rust ni en el comportamiento de la UI.
