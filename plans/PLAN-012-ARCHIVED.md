# PLAN-012 — Reducir el chunk del panel (bundle de Vite)

- **Issue**: #25 — «Reducir el chunk del panel (bundle de Vite >500 kB)»
- **Feature**: `reducir-chunk-panel`
- **Tipo**: perf / build (⚡), solo frontend
- **Estado**: propuesto — pendiente de aprobación de la spec OpenSpec antes de tocar código

## Objetivo

Hoy el build de producción de Vite emite un único chunk `index-*.js` de ~1,18 MB (≈370 kB gzip) y avisa de que supera 500 kB: todo el panel —código, antd, React, React Query— viaja en un solo fichero. Se quiere dividir el bundle en chunks por proveedor mediante `build.rollupOptions.output.manualChunks` para bajar el chunk de entrada por debajo del umbral y cachear las dependencias estables entre despliegues, sin cambio de comportamiento.

## Mapa arquitectónico

- `frontend/vite.config.ts` — no declara hoy ninguna sección `build`; es el único fichero a cambiar (config de Vite + plugin react; la sección `test` es de Vitest).
- Sin cambios en `frontend/src/**`, dependencias, backend ni contrato de API.

## Decisión a cerrar en la spec

- Qué agrupaciones de `manualChunks`: como mínimo separar `antd` (y su dependencia `rc-*`/`@ant-design/*`), `react`/`react-dom` y `react-query` del código de la aplicación. Fijar el criterio: agrupar por paquete de `node_modules` (función que lea `id`) y dejar el código de app en el chunk de entrada.

## Tareas

1. [x] **Medir (baseline)**: registrar el tamaño del chunk de entrada hoy y el aviso >500 kB (`pnpm build`), como estado de partida (RED del cambio: el aviso existe).
2. [x] Abrir el change OpenSpec `reducir-chunk-panel` (proposal + spec delta en `ui` + tasks) y **esperar aprobación** antes de tocar código.
3. [x] **GREEN**: añadir `build.rollupOptions.output.manualChunks` en `frontend/vite.config.ts` agrupando antd / react / react-query.
4. [x] **Verificar**: `pnpm build` sin el aviso >500 kB; el chunk de entrada < 500 kB; los chunks de dependencias con hash estable.
5. [x] **Regresión**: `just check-all` EXIT 0 (typecheck, lint, 131 tests, build).
6. [x] Crear `docs/reducir-chunk-panel.md`; marcar tareas y renombrar el plan a `PLAN-012-ARCHIVED.md`; PR a `development` referenciando el issue #25.

## Fuera de alcance

- Lazy-loading de rutas/componentes.
- Cambios de UI o de contrato de la API.

## Riesgos

- Que `manualChunks` provoque ciclos de módulos o un reparto peor del esperado: mitigado midiendo el resultado y ajustando las agrupaciones.
