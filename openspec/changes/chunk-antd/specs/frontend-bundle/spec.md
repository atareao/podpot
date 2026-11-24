# Specification: Huella de antd en el bundle (frontend)

## Purpose

La huella de Ant Design en el build de producción del panel (los chunks
atribuibles a antd) cabe con margen por debajo del umbral de aviso de Vite
(500 kB raw), sin cambio de comportamiento del panel.

## Requirements

### Requirement: Huella de antd bajo el umbral

El build de producción SHALL mantener la suma de los tamaños raw de todos los
chunks atribuibles a antd (nombres que contienen `vendor-antd`) en 500 kB o
menos, y cada uno de esos chunks individualmente por debajo de 500 kB.

#### Scenario: Build actual en rojo

- **WHEN** se ejecuta el test de tamaño del bundle con el código actual
- **THEN** la suma de `vendor-antd` + `vendor-antd-rc` (~834 kB raw) supera el umbral y el test FALLA

#### Scenario: Tras la optimización

- **WHEN** se ejecuta `pnpm test` y el build programático del test
- **THEN** la suma de los chunks `vendor-antd*` SHALL ser ≤ 500 kB raw
- **AND** cada chunk `vendor-antd*` SHALL ser < 500 kB raw
- **AND** el resto de la suite SHALL seguir en verde

### Requirement: Sin cambio de comportamiento

La optimización NO SHALL cambiar rutas, acciones ni contrato de API del panel.
Los tests existentes de UI SHALL seguir en verde; si un rol accesible cambia
como consecuencia de sustituir un componente, la adaptación del test SHALL ser
mínima y quedar documentada en la PR.

#### Scenario: Suite de regresión

- **WHEN** se ejecuta `pnpm test`, `pnpm run lint` y `tsc -b`
- **THEN** todo SHALL terminar sin errores

## Notes

- Métrica en **raw bytes** (la misma unidad que el `chunkSizeWarningLimit` de
  Vite), no gzip.
- El test `frontend/tests/tamanoDelBundle.test.ts` construye el bundle con la
  API programática de Vite (`write: false`) y no depende de un `dist/` previo.
