# Tasks

## 1. Especificación

- [x] 1.1 Cuatro artefactos (`proposal.md`, `specs/ui/spec.md`, `design.md`, `tasks.md`) escritos y `openspec validate --change ux-alloy-dashboard` en verde

## 2. RED — tests que capturan el síntoma

- [x] 2.1 Tests de cabecera, usuario, salida, persistencia de tema y preferencia del sistema; verificar fallo de los nuevos tests
- [x] 2.2 Tests de las cuatro métricas desde una instantánea, incluido el caso vacío
- [x] 2.3 Tests de búsqueda con debounce, combinación con filtros y estado vacío
- [x] 2.4 Tests de agrupación en tarjetas, expansión/colapso con `aria-expanded` y orden determinista
- [x] 2.5 Tests de acciones deshabilitadas con Spin, finalización del job y resaltado SSE de 2 s
- [x] 2.6 Tests del modal de detalles con campos reales y del modal de logs con búsqueda, autoscroll y desconexión
- [x] 2.7 Tests de notificaciones de éxito/error y de `ErrorBoundary`
- [x] 2.8 Tests de `useMediaQuery` con viewport inferior y superior a 768 px

## 3. GREEN — implementación mínima

- [x] 3.1 `App.tsx` y `main.tsx`: cabecera, `ConfigProvider`, `ErrorBoundary` y `useTema`
- [x] 3.2 `pages/unidades.tsx`: métricas, búsqueda/filtros y composición de grupos
- [x] 3.3 Componentes nuevos y migración del render de `TablaDeUnidades` a tarjetas
- [x] 3.4 `ModalDeDetalles` y `ModalDeRegistros` reutilizando hooks y streams existentes
- [x] 3.5 `estilos.css` y `useMediaQuery` para el diseño adaptable
- [x] 3.6 `npx tsc --noEmit` sin errores

## 4. REFACTOR

- [x] 4.1 Constantes y estilos compartidos, accesibilidad de teclado y limpieza de `EventSource`
- [x] 4.2 `npx eslint .`, `npx vitest run` y `just check-all` en verde, con chunks `vendor-antd*` <500 kB

## 5. Revisión, cierre y despliegue

- [x] 5.1 Auditoría de `@react-reviewer` con APPROVE y hallazgos cerrados — APPROVE en reauditoría.
- [x] 5.2 Commits gitmoji y PR a `development` — PR #71 abierto.
- [x] 5.3 `docs/ux-alloy-dashboard.md`, `openspec archive ux-alloy-dashboard` y renombrar `plans/PLAN-030.md` a `PLAN-030-ARCHIVED.md` — incluye `docs/ux-alloy-dashboard.md` y plan archivado.
- [x] 5.4 `just build`, reinicio de `podpot.service` y verificación de health — just build + reinicio verificado; health 200.
