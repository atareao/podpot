# PLAN-029 — Reemplazar acciones con nombre por iconos (#67)

- Issue: https://github.com/atareao/podpot/issues/67 (asignado a @atareao).
- Rama: `feature/iconos-acciones` (desde `development`).
- Tipo: feature frontend, SDD (OpenSpec) + TDD (RED→GREEN→REFACTOR).

## Objetivos

1. Sustituir las 6 etiquetas textuales de la columna «Acciones» (`Arrancar`, `Parar`, `Reiniciar`, `Editar`, `Logs`, `Borrar`) por iconos de `@ant-design/icons`, sin añadir dependencias.
2. Centralizar el mapeo acción → icono + nombre accesible en un módulo único del frontend.
3. Mantener accesibilidad: `aria-label` / `Tooltip` con el nombre de la acción, de modo que lectores de pantalla y tests sigan identificando cada botón.
4. Actualizar los tests que asertan sobre las etiquetas y añadir cobertura de «no queda texto visible».
5. No tocar backend ni el contrato de la API.

## Mapa arquitectónico

- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx:177-252` — único punto de render de los botones de acción; prop `onAccion` sin cambios.
- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.test.tsx` y `TablaDeUnidades.filtros.test.tsx` — tests de botones, confirmación y tags de progreso.
- `frontend/src/pages/unidades.tsx` y sus tests — wiring `onAccion/onEditar/onVerRegistros/onBorrar`; la interfaz de props no cambia.
- `frontend/src/acciones.ts` — constantes `VERBOS` de mensajes; no se modifican.
- `openspec/specs/ui/spec.md` — requirements «Control desde el panel», «Edición de quadlets desde el panel», «Borrado de quadlets desde el panel», «Visor de logs de una quadlet».
- `frontend/package.json:16` — `@ant-design/icons ^6.3.4` ya instalado; no hay imports previos en `frontend/src`.
- Fuera de alcance: textos de progreso (`etiquetaDeAccionEnCurso`), formularios (`Crear/Guardar/Cancelar/Reintentar/Cerrar`), `EstadoDeUnidad`, backend.

## Tareas técnicas (TDD)

1. [RED] Tests en `TablaDeUnidades.test.tsx`: (a) los botones no contienen texto visible «Arrancar/Parar/Reiniciar/Editar/Logs/Borrar»; (b) cada botón expone su nombre accesible vía `aria-label`/`Tooltip`; (c) `getByRole('button', { name: 'Arrancar' })` sigue funcionando. Ejecutar `npx vitest run` → rojo.
2. [GREEN] Crear el módulo de iconos con el mapeo acción → (icono, etiqueta) e importar los iconos desde `@ant-design/icons`; sustituir los `<Button>{texto}</Button>` por `<Button icon aria-label>` con `Tooltip`. Ejecutar tests → verde y `npx tsc --noEmit`.
3. [REFACTOR] Extraer constantes, revisar accesibilidad, ejecutar `npx eslint .`, `npx vitest run` y `just check-all` (incluye el test de presupuesto de bundle).
4. [OPENSPEC] Crear el change `iconos-en-acciones`, poblar `proposal.md`, el delta de `ui` y `tasks.md`; presentar al usuario y esperar aprobación antes de escribir código de aplicación.
5. [GITFLOW] Commits gitmoji (`✨ feat(ui): ...`), PR a `development`, auditoría `@react-reviewer`, merge y cierre de #67.
6. [DEPLOY] Ejecutar `just build` y reiniciar `podpot.service` para desplegar (el merge no actualiza la imagen local).

## Criterios de aceptación

- No queda texto de acción visible en la columna «Acciones»; solo iconos.
- Cada icono conserva su nombre accesible y su tooltip.
- Los tests existentes y nuevos pasan; `just check-all` rc=0.
- Sin cambios en backend ni en el contrato de la API.

## Riesgos

- Los tests actuales localizan botones por nombre accesible: si el icono no aporta `aria-label`, se rompen.
- Ant Design `Button` con `icon` puede necesitar `Tooltip` para mostrar el nombre con ratón.
- Vigilar el tamaño de los chunks de `@ant-design/icons`: usar imports individuales y tree-shakeables para no romper el presupuesto de bundle.
