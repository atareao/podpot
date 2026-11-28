# Design

## Context

Ver `proposal.md` para la motivación. La tabla de unidades es el único punto donde se renderizan los seis botones de acción; recibe un callback `onAccion` y no tiene menús contextuales ni toolbar. `@ant-design/icons` ya está instalado pero no se usa en `frontend/src`. Los tests actuales localizan los botones por nombre accesible, no por texto interno.

## Goals / Non-Goals

**Goals:**

- Sustituir la presentación textual por iconos manteniendo el nombre accesible y el tooltip.
- Evitar duplicación del mapeo icono/etiqueta en varios puntos del render.
- No alterar el contrato de props ni el flujo de llamadas a la API.

**Non-Goals:**

- No cambiar los textos de progreso de las acciones ni los badges de estado.
- No cambiar los botones genéricos de formularios (`Crear`, `Guardar`, `Cancelar`, `Reintentar`, `Cerrar`).
- No introducir una librería de iconos nueva ni cambiar el tema de Ant Design.

## Decisions

- **Iconos individuales de `@ant-design/icons`**: se importa cada icono por separado para favorecer el tree-shaking y no romper el presupuesto de chunks. Alternativa descartada: importar el namespace completo, más simple pero con riesgo de tamaño.
- **Módulo único de mapeo**: un fichero junto a `TablaDeUnidades` define la relación acción → (icono, etiqueta). Alternativa descartada: literales inline en cada botón, que duplican la etiqueta y dificultan los tests.
- **`Button` con `icon` + `aria-label` + `Tooltip`**: `Button` mantiene la semántica de botón y el nombre accesible; `Tooltip` cubre el descubrimiento con ratón. Alternativa descartada: usar solo `title`, que no garantiza el mismo comportamiento accesible en todos los navegadores.
- **Sin cambios de interfaz**: `onAccion`, `onEditar`, `onVerRegistros` y `onBorrar` permanecen iguales, de modo que `pages/unidades.tsx` no se toca.

## Risks / Trade-offs

- [Los tests actuales pierden el nombre del botón] → Mantener `aria-label` idéntico al texto anterior en los seis controles.
- [Aumento del bundle al importar iconos] → Usar imports individuales y verificar el test de presupuesto de chunks en `just check-all`.
- [El tooltip no es visible en tests jsdom] → No asertar sobre el tooltip; asertar sobre `aria-label`, que es lo que expone la accesibilidad.

## Migration Plan

No hay migración de datos ni de API. El despliegue requiere reconstruir la imagen local con `just build` y reiniciar `podpot.service`; rollback: restaurar la imagen anterior y reiniciar el servicio.

## Open Questions

(ninguna)
