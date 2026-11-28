# Design

## Context

Podpot ya tiene un panel funcional con tabla agrupada por stack, streams SSE y acciones iconográficas. La referencia UX es alloy, que usa cabecera, métricas, tarjetas por proyecto, modales, notificaciones, tema y responsive. Ant Design en podpot es 5.x (alloy usa 6.x), así que se adaptan los patrones sin copiar dependencias. El backend expone suficiente información para métricas, búsqueda, inspección y logs; no expone actualizaciones, historial ni configuración.

## Goals / Non-Goals

**Goals:** mejorar la escaneabilidad del panel; reutilizar los streams y endpoints actuales; mantener accesibilidad y presupuesto de bundle; añadir tema y vista móvil.

**Non-Goals:** no cambiar backend ni API; no añadir Historial ni Config de alloy; no sustituir Antd; no añadir dependencias; no fabricar datos de actualizaciones.

## Decisions

- **Tarjetas por stack en lugar de tabla**: el grupo es la unidad visual equivalente a `compose_project` en alloy; se conserva `stack` y el orden determinista del backend.
- **Métricas derivadas del cliente**: se calculan de la instantánea SSE/REST; evita un endpoint nuevo. La cuarta métrica es «Fallidas».
- **Búsqueda local con debounce**: los datos ya están en memoria; no se pide al backend.
- **Modales reutilizando hooks**: `ModalDeRegistros` usa `useRegistrosDeUnidad`; `ModalDeDetalles` usa el endpoint de quadlet existente.
- **`ConfigProvider` con `darkAlgorithm`**: sin librerías nuevas; se mide el tamaño de chunks.
- **`useMediaQuery` propio**: un hook pequeño con `matchMedia`, sin dependencias.

## Risks / Trade-offs

- `darkAlgorithm` puede incrementar los chunks → verificar `frontend/tests/tamanoDelBundle.test.ts`.
- Tests que consultan `role="table"` → migrarlos a consultas por encabezado de grupo y tarjeta.
- `matchMedia`/tooltip inestables en jsdom → mocks y aserciones sobre atributos accesibles.
- SSE duplicado o sin limpieza → desconectar `EventSource` al desmontar.

## Migration Plan

Cambio puramente de frontend: no hay migración de datos ni de API. Despliegue con `just build` y reinicio de `podpot.service`. Rollback: restaurar la imagen anterior.

## Open Questions

(ninguna)
