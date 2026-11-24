# Design — antd-bundle-budget

Issue de entrada: **#56** (PR #57, merge `778e629`).

## Context

El build de producción de Vite reparte Ant Design en dos chunks gestionados por `manualChunks`: `vendor-antd` (el paquete `antd`) y `vendor-antd-rc` (sus paquetes `rc-*`). La motivación completa está en `proposal.md`: la PR #57 original medía la **suma** de los chunks (~834 kB) e imponía un criterio más estricto que el del issue, además de romper `typecheck`. El usuario ha aprobado el umbral de **< 500 kB raw por chunk individual**, no sobre la suma. Ant Design debe conservarse: el panel usa sus componentes y la spec no permite eliminarlos ni sustituirlos.

## Goals / Non-Goals

- **Goals**: cumplir < 500 kB raw por chunk; añadir un test de regresión tipado que lo verifique en CI; conservar los componentes de Ant Design usados por la UI.
- **Non-Goals**: eliminar o sustituir Ant Design; externalizarlo por CDN; imponer un presupuesto sobre la suma de chunks; cambiar la UI.

## Decisions

### 1. Terser con 2 pasadas, no solo esbuild

Se configura Terser con `compress.passes: 2`, `drop_console`, `drop_debugger` y `mangle.safari10` en lugar de quedarse con el minificador por defecto de esbuild. Reduce el tamaño **raw** de cada chunk sin tocar dependencias ni imports; el coste es un build algo más lento (ver *Risks*).

### 2. `manualChunks` estable existente, no reagrupar componentes

Se mantiene el reparto ya presente (`antd` → `vendor-antd`, `rc-*` → `vendor-antd-rc`, React → `vendor-react`, React Query → `vendor-query`). Reagrupar componentes para repartir peso rompería la caché del navegador entre despliegues y no aporta nada al criterio por chunk; la reducción se logra con compresión.

### 3. Tipado estructural local en el test, sin `rollup` como devDependency

El test declara tipos mínimos para `output` y usa el discriminante `type === 'chunk'`, en lugar de importar los tipos de Rollup. Con pnpm, Rollup no es dependencia directa, así que añadirla solo para el test incrementaría las dependencias de desarrollo sin beneficio; el tipado estructural evita `any` y pasa `tsc --noEmit`.

### 4. Restaurar los componentes de Ant Design desde `development`

Tras la auditoría se recuperaron los componentes (`Form`, `Input`, `Select`, `Space`, `Table`, `Typography`) tal como estaban en `development`, descartando los `select`/tablas nativos introducidos por la PR #57. Esos cambios incumplían la spec y, además de conservar la librería exigida, recuperaron accesibilidad y estilos ya validados.

### 5. Presupuesto por chunk, no sobre la suma

El test mide `code.length` de cada chunk `vendor-antd*` y falla si alguno alcanza `500 * 1024` bytes. Se elimina la aserción de suma de la PR #57: el criterio del issue y la confirmación del usuario se aplican a chunks individuales, y la suma de dos chunks conformes (~834 kB) no es un incumplimiento.

## Risks / Trade-offs

- **[Actualización de antd eleva un chunk]** → el test falla en CI; ajustar imports/`manualChunks`, nunca sustituir la librería.
- **[Terser más lento]** → tiempo de build aceptable (~4-8 s).
- **[drop_console]** → no hay `console.*` en `src`, verificado.

## Migration Plan

No hay migración de datos: basta con desplegar el frontend (no cambia la API Rust ni el esquema SQLite). Rollback: revertir el merge de PR #57.
