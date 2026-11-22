# Tasks — reducir-chunk-panel

Cambio de configuración de build, sin lógica. La red de seguridad es la propia salida del build (medida antes y después) más los 131 tests de frontend, que no deben moverse.

## 1. Baseline (estado de partida)

- [x] 1.1 Registrar el tamaño del chunk de entrada y el aviso >500 kB con `pnpm build` (en `frontend/`), antes de tocar nada. — Verificación: queda anotado el tamaño de partida (~1,18 MB) y el aviso.

## 2. Spec (SDD)

- [x] 2.1 Change OpenSpec `reducir-chunk-panel` creado y aprobado por el usuario antes de tocar `vite.config.ts`.

## 3. Implementación (GREEN)

- [x] 3.1 Añadir `build.rollupOptions.output.manualChunks` en `frontend/vite.config.ts`, agrupando antd / react / react-query por paquete de `node_modules`. — Verificación: `pnpm build` sin errores.

## 4. Verificación y regresión

- [x] 4.1 `pnpm build` sin el aviso >500 kB y con el chunk de entrada por debajo de 500 kB. — Verificación: salida del build.
- [x] 4.2 El hash de los chunks de dependencias es estable entre dos builds con el código de app cambiado (o sin cambios). — Verificación: comparar nombres de fichero.
- [x] 4.3 `just check-all` EXIT 0; 131 tests de frontend verdes. — Verificación: informes.
- [x] 4.4 Crear `docs/reducir-chunk-panel.md`; marcar tareas y renombrar el plan a `plans/PLAN-012-ARCHIVED.md`; PR a `development` referenciando el issue #25.
