# PLAN-030 — Adoptar la UX de alloy en el panel de unidades (#70)

- Issue: https://github.com/atareao/podpot/issues/70 (asignado a @atareao).
- Rama: `feature/ux-alloy-dashboard` (creada desde `development`).
- Referencia de UX: `/data/rust/alloy/frontend/src` (`DashboardPage.tsx`, `components/ContainerTable.tsx`, `components/ContainerRow.tsx`, `components/ContainerActions.tsx`, `components/LogsModal.tsx`, `components/InspectModal.tsx`, `hooks/useRealtime.ts`, `hooks/useMediaQuery.ts`, `main.tsx`).
- Tipo: feature frontend, SDD (OpenSpec) + TDD (RED → GREEN → REFACTOR). Cero cambios en backend y en el contrato REST/SSE.
- Dependencia: PR #69 (cierre de iconos) debe estar mergeado en `development` antes de tocar `TablaDeUnidades`; si no lo está, integrar `origin/feature/iconos-acciones` en esta rama.

## Objetivos

1. Sustituir la pantalla única actual por un dashboard con cabecera fija estilo alloy: marca «podpot», nombre de usuario obtenido de `GET /auth/session` y botón de salida; sin sidebar; ancho máximo 1200 px.
2. Añadir cuatro tarjetas de métricas derivadas de la instantánea de `GET /units` / SSE `units`: total, en ejecución, paradas y fallidas. No se inventa la métrica «actualizaciones» de alloy porque podpot no expone ese dato.
3. Añadir una barra de búsqueda con debounce (~250 ms) sobre `file_name`, `name`, `stack` y `kind`, combinable con los filtros `Select` existentes por `kind` y `active_state`, con estado vacío explícito.
4. Convertir la tabla en tarjetas de unidad agrupadas por `stack`, con grupos expandibles/colapsables, conservando badge de estado, sub-estado, progreso de jobs, identidad `file_name` y las seis acciones iconográficas con `aria-label` y `Tooltip`.
5. Sustituir el `Drawer` de registros por un modal estilo `LogsModal` que reutilice `useRegistrosDeUnidad` (SSE `logs/stream`), con búsqueda de líneas, autoscroll y cierre que desmonte el `EventSource`.
6. Añadir un modal de inspección quemuestre únicamente campos reales de `GET /quadlets/{name}`: imagen, `container_name`, puertos, volúmenes, entornos, red/subred y pod. No se inventan health, labels ni métricas inexistentes.
7. Mejorar el feedback de acciones: mientras haya job en curso, `Spin` y texto «Iniciando…/Parando…/Reiniciar…», botones deshabilitados, notificaciones de 3 s (éxito/error/aviso) y resaltado de la unidad durante 2 s tras un evento SSE. Añadir un `ErrorBoundary` global.
8. Incorporar tema claro/oscuro con `ConfigProvider` (`theme.defaultAlgorithm` / `darkAlgorithm` de Ant Design 5), persistido en `localStorage` y respetando `prefers-color-scheme`, sin sustituir Antd ni añadir dependencias.
9. Hacer el panel adaptable por debajo de 768 px mediante un hook `useMediaQuery`: las tarjetas reducen padding y etiquetas, y los controles siguen siendo accesibles por teclado y con nombre accesible.
10. Mantener el presupuesto de bundle (`vendor-antd*` < 500 kB), la cobertura de tests y la compatibilidad con la spec `ui` existente.

## Mapa arquitectónico

- `frontend/src/App.tsx` — composición de cabecera, tema, `ErrorBoundary` y ruta única; conserva `SoloConSesion`.
- `frontend/src/main.tsx` — `ConfigProvider` con el tema resuelto; sin cambiar librerías.
- `frontend/src/pages/unidades.tsx` — orquestador del dashboard: métricas, búsqueda/filtros y grupos; sigue usando `useUnits`, `useUnidadesEnVivo` y `useTrabajosEnVivo`.
- `frontend/src/components/TablaDeUnidades/` — se mantiene la ruta para minimizar churn de imports, pero su render pasa a tarjetas; se reutilizan `acciones-icono.ts` y `EstadoDeUnidad`.
- Nuevos componentes (bajo `frontend/src/components/`): `MetricasDelPanel`, `BarraDeBusqueda`, `GrupoDeUnidades`, `TarjetaDeUnidad`, `ModalDeDetalles`, `ModalDeRegistros`, `ErrorBoundary`.
- Nuevos hooks (`frontend/src/hooks/`): `useMediaQuery`, `useTema`. Se reutilizan `useRegistrosDeUnidad`, `useSesion` y los streams SSE existentes.
- `frontend/src/api/{client,units,quadlets,sesion}.ts` y `frontend/src/types/api.ts` — sin cambios de contrato; `ModalDeDetalles` consume el endpoint de quadlet ya existente.
- `frontend/src/estilos.css` — variables de resaltado, reglas responsive y ajustes de densidad.
- `openspec/specs/ui/spec.md` — requirements modificados por el delta `ux-alloy-dashboard`.
- `frontend/tests/tamanoDelBundle.test.ts` — debe seguir verde; ningún chunk `vendor-antd*` por encima de 500 kB.
- Fuera de alcance: `backend/`, migraciones, endpoints, esquema, `docs/` de feature anterior y las páginas Historial/Config de alloy.

## Tareas técnicas (TDD)

1. [OPENSPEC] Crear el change `ux-alloy-dashboard` con `proposal.md`, `specs/ui/spec.md` (delta MODIFIED) y `tasks.md`; validar con `openspec validate --change ux-alloy-dashboard`; presentarlo al usuario y esperar aprobación antes de escribir código o tests.
2. [RED] Escribir tests colocated con Vitest + Testing Library/user-event para: conteo de las 4 métricas desde una instantánea de unidades; búsqueda con debounce y combinación con filtros; estado vacío; expansión/colapso de grupos por `stack` con `aria-expanded`; render de acciones deshabilitadas con Spin durante un job; click en stop/restart con el callback correcto; modal de detalles con los campos reales del quadlet; modal de registros con líneas SSE, búsqueda y autoscroll; notificaciones de éxito/error; persistencia del tema y `prefers-color-scheme`; `matchMedia` para móvil; y `ErrorBoundary`. Ejecutar `npx vitest run` y confirmar que fallan solo los tests nuevos.
3. [GREEN] Implementación mínima: componentes y hooks nuevos, migración del render de `TablaDeUnidades` a tarjetas, integración en `pages/unidades.tsx`, `ConfigProvider` en `main.tsx`, cabecera en `App.tsx`. Ejecutar tests y `npx tsc --noEmit` hasta que todo pase.
4. [REFACTOR] Extraer constantes y estilos compartidos, revisar accesibilidad (nombres accesibles con `file_name`, foco y teclado en grupos), limpiar `EventSource` al desmontar, ejecutar `npx eslint .`, `npx vitest run` y `just check-all` (incluye el test de presupuesto de bundle).
5. [AUDIT] Auditoría de `@react-reviewer` sobre el diff; si hay hallazgos, devolver a `@frontend-dev` con los logs y reauditar hasta APPROVE.
6. [GITFLOW] Commits gitmoji (`✨ feat(ui): ...`, `♻️ refactor(ui): ...`), push y PR a `development`; verificar CI; no commitear en `main` ni `development`.
7. [DOCS/ARCHIVE] Crear `docs/ux-alloy-dashboard.md`, marcar las tareas del change, ejecutar `openspec archive ux-alloy-dashboard`, renombrar este plan a `plans/PLAN-030-ARCHIVED.md` y actualizar `HANDOFF.md`.
8. [DEPLOY] `just build` y `systemctl --user restart podpot.service`; verificar `GET /api/v1/health` 200 y que el panel refleja los cambios.

## Criterios de aceptación

- La cabecera, las 4 métricas, la búsqueda, los grupos de tarjetas, los modales, las notificaciones, el tema y el modo móvil funcionan con los datos y streams que ya expone el backend.
- `git diff --name-only origin/development -- backend/` está vacío: no hay cambios de backend ni de esquema.
- Las acciones siguen ejecutando las mismas llamadas y conservan `aria-label` con `file_name`; no se pierde el progreso por jobs ni el resaltado SSE.
- `npx vitest run`, `npx tsc --noEmit`, `npx eslint .` y `just check-all` en verde; chunks `vendor-antd*` < 500 kB.
- `openspec validate --specs` en verde y auditoría `@react-reviewer` con APPROVE.

## Riesgos

- El `ConfigProvider` con `darkAlgorithm` puede engordar los chunks de Antd: vigilar el presupuesto y-medir con el test dedicado antes de aceptar el tema.
- Los tests actuales localizan una tabla por roles: migrarlos a tarjetas sin perder aserciones de identidad y accesibilidad.
- `matchMedia` y los tooltips no son estables en jsdom: simular con mocks y no asertar detalles dependientes del navegador.
- Los streams SSE pueden duplicar eventos o dejar `EventSource` abierto: garantizar limpieza en `useEffect` y descarte de eventos tras el desmonte.
- No hay dato de «actualizaciones» en podpot: la cuarta métrica es «Fallidas» para no fabricar información.
- Dependencia de PR #69: si no está mergeado, integrarlo antes de modificar `TablaDeUnidades`.
