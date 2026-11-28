# Proposal

## Why

El panel de podpot funciona pero su presentación es más densa y menos scaneable que la UX de alloy (referencia: `/data/rust/alloy/frontend/src`): una tabla larga sin resumen, sin búsqueda, sin vista móvil, sin tema oscuro y con los registros en un Drawer. Queremos adoptar los patrones que ya funcionan en alloy —cabecera, métricas, búsqueda, tarjetas agrupadas, modales, notificaciones y tema— sin tocar el backend ni el contrato REST/SSE.

## What Changes

- Añadir una cabecera fija con marca, usuario y salida, y un `ConfigProvider` con tema claro/oscuro persistido.
- Añadir cuatro métricas derivadas de la instantánea de unidades: total, en ejecución, paradas y fallidas (no existe dato de actualizaciones en podpot).
- Añadir búsqueda con debounce combinable con los filtros actuales, y un estado vacío explícito.
- Sustituir el render de tabla por tarjetas agrupadas por `stack`, expandibles y accesibles por teclado.
- Convertir el visor de registros en un modal con búsqueda y autoscroll, reutilizando el SSE actual.
- Añadir un modal de inspección con los campos reales de `GET /quadlets/{name}`.
- Añadir feedback de progreso con `Spin`, botones deshabilitados, notificaciones de 3 s, resaltado SSE de 2 s y `ErrorBoundary`.
- Hacer el panel adaptable por debajo de 768 px.
- No se modifica backend, API, esquema ni dependencias.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `ui`: se añaden los requirements de cabecera/tema, métricas, búsqueda, inspección, diseño adaptable y notificaciones; se modifican los de listado agrupado, visor de logs y progreso de acciones.

## Impact

- Código: `frontend/src/App.tsx`, `main.tsx`, `pages/unidades.tsx`, `components/`, `hooks/`, `estilos.css` y sus tests.
- API/backend: sin impacto; se consumen `/units`, `/units/stream`, `/jobs/stream`, `/units/{name}/logs/stream`, `/quadlets/{name}` y `/auth/session` tal cual.
- Riesgo: presupuesto de chunks `vendor-antd*` (<500 kB) al activar `darkAlgorithm`; migración de tests que localizan una tabla por roles; limpieza de `EventSource`.
- Dependencia: PR #69 debe estar integrado antes de tocar `TablaDeUnidades`.
