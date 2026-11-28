# ux-alloy-dashboard — UX de alloy en el panel de unidades

## Qué es

Rediseño del panel de unidades para acercar la experiencia de `/data/rust/alloy` a podpot: cabecera con marca y usuario, métricas del panel, búsqueda, tarjetas agrupadas por stack, modales de detalle y registros, notificaciones, tema claro/oscuro y diseño adaptable a móvil. Se inspira en la UX de alloy (`DashboardPage.tsx`, `ContainerTable.tsx`, `ContainerRow.tsx`, `ContainerActions.tsx`, `LogsModal.tsx`, `InspectModal.tsx`, `useRealtime.ts`, `useMediaQuery.ts`), pero podpot sigue usando **Ant Design 5**: no se ha migrado a Antd 6 ni se han añadido dependencias.

Issue: #70 · Rama: `feature/ux-alloy-dashboard` · Plan: `plans/PLAN-030.md` · Change OpenSpec: `ux-alloy-dashboard` (9 requirements `ADDED` y 3 `MODIFIED` sobre la spec `ui`).

## Ficheros implicados

Frontend (`frontend/src/`):

- `App.tsx` — cabecera con la marca «podpot», el usuario de `GET /auth/session`, control de tema y botón de cerrar sesión; envuelve las rutas en `ErrorBoundary` y aplica `ConfigProvider` con `theme.defaultAlgorithm` / `theme.darkAlgorithm`. Conserva `SoloConSesion`.
- `main.tsx` — raíz de providers (`QueryClientProvider`, `App` de Antd, `BrowserRouter`) e importación de `estilos.css`; sin cambios de librerías.
- `pages/unidades.tsx` — orquestador del dashboard: métricas, búsqueda y filtros, grupos de unidades, notificaciones y modales; sigue usando `useUnits`, `useUnidadesEnVivo` y `useTrabajosEnVivo`.
- `components/MetricasDelPanel/` — cuatro tarjetas de métricas derivadas de la instantánea de unidades.
- `components/BarraDeBusqueda/` — búsqueda con debounce de 250 ms sobre `file_name`, `name`, `stack` y `kind`, combinable con los filtros `Select` por `kind` y `active_state`.
- `components/GrupoDeUnidades/` — tarjetas por stack con cabecera expandible (`aria-expanded`), estado, sub-estado, progreso de jobs y acciones iconográficas.
- `components/TablaDeUnidades/` — contenedor del listado (`role="list"`); conserva su ruta de imports para minimizar el churn y reutiliza `acciones-icono.ts` y `EstadoDeUnidad`.
- `components/ModalDeDetalles/` — inspección de la quadlet con campos reales de `GET /api/v1/quadlets/{name}`.
- `components/ModalDeRegistros/` — registros en vivo por SSE, con búsqueda de líneas, autoscroll y baja del `EventSource` al cerrar.
- `components/ErrorBoundary/` — límite de errores global con mensaje de recuperación.
- `hooks/useTema.ts` — tema persistido en `localStorage` (`podpot-tema`) con `prefers-color-scheme` como valor inicial.
- `hooks/useMediaQuery.ts` — suscripción a `matchMedia` mediante `useSyncExternalStore`; el panel usa el breakpoint `max-width: 767px`.
- `estilos.css` — cabecera, tarjetas, resaltado por SSE, densidad reducida en móvil y reglas responsive.

Especificación y planes: `openspec/changes/ux-alloy-dashboard/` (`proposal.md`, `specs/ui/spec.md`, `tasks.md`), `plans/PLAN-030.md`.

## Cómo funciona

1. La cabecera muestra la marca y, si hay sesión, el `email` (o el `sub` si no hay email), un botón para alternar tema y la salida. No hay sidebar: la vista única es el panel de unidades, con ancho máximo contenido.
2. Las métricas se calculan a partir de la snapshot de `GET /api/v1/units` y de los eventos SSE: **Total**, **En ejecución**, **Paradas** y **Fallidas**. La cuarta tarjeta es «Fallidas» y no «Actualizaciones» de alloy porque el backend de podpot no expone ese dato; no se fabrica información.
3. La búsqueda aplica el texto con 250 ms de debounce y se combina (AND) con los filtros existentes por tipo y estado; si no hay coincidencias se muestra un estado vacío explícito.
4. El listado se agrupa por `stack`: cada grupo es una tarjeta con una cabecera botón (`aria-expanded`) que expande o colapsa las unidades. Se conservan la identidad por `file_name`, el badge de estado, el sub-estado, el progreso de jobs, los tooltips, las seis acciones iconográficas con `aria-label` y el resaltado de 2 s tras un evento SSE.
5. «Detalles» abre `ModalDeDetalles` con los campos que realmente devuelve el endpoint: imagen, nombre del contenedor, puertos publicados, volúmenes, entornos, nombre del volumen, conexión (red/subred) y agrupación (pod). Las listas vacías y los campos ausentes no se renderizan; los errores de carga se muestran dentro del modal con `role="alert"`.
6. «Logs» abre `ModalDeRegistros`, que reutiliza `useRegistrosDeUnidad` sobre `GET /api/v1/units/{name}/logs/stream`: líneas SSE con origen stdout/stderr, búsqueda local, autoscroll y cierre del `EventSource` al desmontar.
7. Mientras hay un job en curso se muestran `Spin` y texto («Iniciando…», «Parando…», «Reiniciar…»), se deshabilitan los botones y se emiten notificaciones de éxito/error/aviso de 3 s. `ErrorBoundary` evita que un fallo de render deje el panel en blanco.
8. El tema se persiste en `localStorage` (`podpot-tema`); si no hay elección, sigue `prefers-color-scheme`. Por debajo de 768 px, `useMediaQuery` reduce padding y etiquetas sin perder accesibilidad por teclado.

No hay cambios en `backend/`, en la API REST/SSE ni en el esquema SQLite: `git diff --name-only origin/development -- backend/` está vacío.

## Variables/configuración

- `podpot-tema` (clave de `localStorage`): `light` o `dark`. Si no existe, se usa `window.matchMedia('(prefers-color-scheme: dark)')`.
- Breakpoint móvil: `(max-width: 767px)` en `useMediaQuery`.
- Búsqueda: retardo de 250 ms (`RETARDO_MS` en `BarraDeBusqueda`).
- Notificaciones: 3 s; resaltado por SSE: 2 s.
- Sin variables de entorno nuevas ni cambios de despliegue: no requiere montajes, migraciones ni reverse-proxy distintos.

## Cómo verificarlo

- `just check-all`: rc = 0; 445 tests de backend y 210 de frontend (30 ficheros), además de `tsc --noEmit`, lint y build.
- Presupuesto de bundle: chunks `vendor-antd` 323,95 kB y `vendor-antd-rc` 284,80 kB, ambos por debajo del límite de 500 kB (test `frontend/tests/tamanoDelBundle.test.ts`).
- `openspec validate ux-alloy-dashboard`: válido.
- Comprobación de alcance: `git diff --name-only origin/development -- backend/` sin salida.
- Comprobación manual prevista: `just build` y `systemctl --user restart podpot.service`, seguido de `GET /api/v1/health` (200) y revisión visual del panel.

## Decisiones

- La UX se toma como inspiración, no como dependencia: se mantienen React + TypeScript + Ant Design 5 y el pipeline de Vite/pnpm/Vitest existente. No se han añadido paquetes.
- Se conserva la ruta `TablaDeUnidades/` para no reescribir imports; el render pasa a tarjetas de stack.
- Cuarta métrica «Fallidas», no «Actualizaciones»: el backend no ofrece ese dato y la spec exige no inventarlo.
- `ModalDeDetalles` solo muestra campos del contrato real de `GET /quadlets/{name}`: sin health, labels ni métricas ficticias.
- `ModalDeRegistros` reutiliza `useRegistrosDeUnidad` en lugar de duplicar la lógica SSE del anterior `Drawer`.
- `useTema` evita efectos adicionales usando una referencia al valor vigente para `alternarTema`.
- Los tests de `matchMedia` y tooltips se simulan en jsdom; no se asertan detalles dependientes del navegador.
- Fuera de alcance: las páginas **Historial** y **Config** de alloy, cambios en backend/API/esquema y dependencias nuevas.

## Estado final

- Auditoría `@react-reviewer`: **APPROVE** tras 3 rondas de correcciones (contraste en tema oscuro, resaltado pegado al filtrar, semántica de lista y contrastes AA).
- PR **#71** abierto desde `feature/ux-alloy-dashboard` a `development`; commit `fc4399c`. Pendiente: CI, merge y cierre del issue #70.
- OpenSpec `ux-alloy-dashboard` archivado como `2026-11-28-ux-alloy-dashboard`; plan renombrado a `plans/PLAN-030-ARCHIVED.md`; `openspec validate --specs`: 13/13.
- Despliegue local realizado: `just build` + `systemctl --user restart podpot.service`; servicio `active` y `GET /api/v1/health` 200.
- Follow-up técnico: comprobación manual en devtools de `--ant-color-*` con `cssVar: true` (los fallbacks ya cubren el caso).
