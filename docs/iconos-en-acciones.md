# Iconos en acciones

## Qué es

Feature frontend (#67) que sustituye las seis etiquetas textuales de la columna «Acciones» de la tabla de unidades (`Arrancar`, `Parar`, `Reiniciar`, `Editar`, `Logs`, `Borrar`) por iconos, manteniendo el nombre accesible y el tooltip de cada control. No cambia qué hace cada acción ni el contrato de la API: es un cambio de presentación y accesibilidad.

Implementada en el commit `f996738` (`✨ feat(ui): reemplazar acciones con nombre por iconos`) sobre la rama `feature/iconos-acciones`, e integrada en `development` mediante el PR #68 (merge commit `8ec4276`, 2026-11-28). Plan: `plans/PLAN-029.md` (archivado como `PLAN-029-ARCHIVED.md`). Change OpenSpec: `2026-11-28-iconos-en-acciones`.

## Ficheros implicados

- `frontend/src/acciones-icono.ts` (nuevo): mapeo centralizado acción → (icono, nombre), con el tipo `AccionNombre`.
- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx`: único punto de render de los seis botones; pasa a usar `Button` con `icon` y `aria-label` en lugar de texto visible.
- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.test.tsx`: tests de ausencia de texto visible y de conservación del nombre accesible.
- `openspec/changes/archive/2026-11-28-iconos-en-acciones/`: `proposal.md`, `design.md`, `tasks.md` y el delta `specs/ui/spec.md` (requirement «Controles de acción con iconos»).
- Sin cambios: backend, contrato de la API, props de `TablaDeUnidades` ni `pages/unidades.tsx`.

## Cómo funciona

`acciones-icono.ts` exporta `iconoYAccion(accion)` y una tabla con seis entradas (`start`, `stop`, `restart`, `edit`, `logs`, `borrar`). Cada entrada combina un icono individual de `@ant-design/icons` (`PlayCircleOutlined`, `StopOutlined`, `ReloadOutlined`, `EditOutlined`, `FileTextOutlined`, `DeleteOutlined`) con una función `nombre(identidad)` que reproduce el texto anterior, p. ej. `Arrancar <identidad>` o `Logs <identidad>`. `TablaDeUnidades` obtiene de ahí tanto el icono como el nombre, que se usa como `aria-label` (y se expone en `Tooltip`) para que lectores de pantalla y tests sigan localizando el control por `getByRole('button', { name: 'Arrancar …' })`.

La dependencia `@ant-design/icons` ya estaba instalada (`^6.3.4`); no se añade ninguna nueva. Los imports son individuales para favorecer el tree-shaking y no romper el presupuesto de tamaño de bundle.

## Variables/configuración

Ninguna variable de entorno ni opción nueva. El comportamiento de las acciones (incluidos los callbacks `onAccion`, `onEditar`, `onVerRegistros` y `onBorrar`) permanece intacto.

## Decisiones

- **Iconos individuales** frente a importar el namespace completo de `@ant-design/icons`: evita crecer los chunks.
- **Módulo único de mapeo** frente a literales inline en cada botón: evita duplicar etiquetas y mantiene los tests estables.
- **`Button` con `icon` + `aria-label` + `Tooltip`**: conserva la semántica de botón y el nombre accesible; el tooltip cubre el descubrimiento con ratón. Se descartó usar solo `title`.
- **Sin cambios de interfaz**: las props y el wiring de `pages/unidades.tsx` no se tocan.
- Fuera de alcance: textos de progreso (`etiquetaDeAccionEnCurso`), badges de `EstadoDeUnidad` y botones genéricos de formularios (`Crear`, `Guardar`, `Cancelar`, `Reintentar`, `Cerrar`).

## Cómo verificarlo

- `npx vitest run` en `frontend/`: los tests de `TablaDeUnidades` confirman que no queda texto de las seis acciones y que cada botón conserva su nombre accesible; los tests heredados siguen localizando los controles por nombre.
- `npx tsc --noEmit` y `npx eslint .` sin errores.
- `just check-all` desde la raíz: incluye el test de presupuesto de bundle de los chunks de iconos.
- En el panel, cada fila debe mostrar solo iconos en «Acciones»; al pasar el ratón aparece el tooltip con el nombre de la acción.

## Follow-ups

- Auditoría @react-reviewer: APPROVE en reauditoría tras corregir cobertura de stop/restart y centralizar etiquetas; 177 tests frontend en verde.
- Issue #67: cerrado (COMPLETED) tras el PR #68; no queda pendiente de cierre.
- `openspec validate --specs`: verificado en verde, 13 passed / 0 failed.
- Despliegue local pendiente: ejecutar `just build` y reiniciar `podpot.service` (un merge en `development` no actualiza la imagen `localhost/podpot:dev`), y comprobar `GET /api/v1/health`.
- No se ha realizado en este cierre ninguna verificación adicional de despliegue ni de auditoría.
