# Proposal

## Why

Las acciones de la tabla de unidades se muestran hoy como texto («Arrancar», «Parar», «Reiniciar», «Editar», «Logs», «Borrar»), lo que recarga visualmente cada fila y dificulta el escaneo rápido del panel. Sustituirlas por iconos con nombre accesible uniforme mejora la densidad y la UX sin cambiar lo que hace cada acción.

## What Changes

- Sustituir las seis etiquetas textuales de la columna «Acciones» por iconos de `@ant-design/icons` (ya presente, sin nueva dependencia).
- Centralizar en un único módulo del frontend el mapeo acción → icono + nombre accesible.
- Exponer cada icono con `aria-label` y `Tooltip` con el nombre de la acción, de modo que lectores de pantalla y automatizaciones sigan identificando el control.
- Actualizar los tests que localizan los botones por nombre accesible y añadir cobertura de ausencia de texto visible.
- No se modifica el backend, la API ni el contrato de props de `TablaDeUnidades`.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `ui`: se añade el requirement «Controles de acción con iconos», que prescribe la presentación iconográfica y accesible de las acciones sobre unidades. El comportamiento existente de arrancar/parar/reiniciar/editar/borrar/ver logs no cambia.

## Impact

- Código: `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx` y sus tests (`TablaDeUnidades.test.tsx`, `TablaDeUnidades.filtros.test.tsx`), tests de `pages/unidades`.
- Dependencias: ninguna nueva; se usa `@ant-design/icons ^6.3.4` ya instalado.
- API/backend: sin impacto.
- Riesgo: regresión de accesibilidad si el icono no aporta nombre accesible, y presión sobre el presupuesto de tamaño de bundle si se importa la librería completa en lugar de iconos individuales.
