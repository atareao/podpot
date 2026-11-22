# Proposal

## Why

El build de producción del panel emite **un único chunk** `index-*.js` de ~1,18 MB (≈370 kB gzip) y avisa de que supera 500 kB. Todo el panel —código de la aplicación, antd, React y React Query— viaja en un solo fichero: la primera carga descarga y parsea la dependencia entera antes de pintar nada, y cualquier cambio en el código de la aplicación invalida la caché de **todo** el bundle, incluidas las dependencias que no han cambiado. Es la deuda de UI registrada en `HANDOFF.md` («chunk de antd >500 kB»).

## What Changes

- Se añade a `frontend/vite.config.ts` una sección `build.rollupOptions.output.manualChunks` que agrupa las dependencias por proveedor (al menos `antd`, `react`/`react-dom` y `react-query`), dejando el código de la aplicación en el chunk de entrada.
- El chunk de entrada pasa a quedar por debajo del umbral de aviso de 500 kB.
- Los chunks de dependencias estables conservan su hash entre despliegues, de modo que un cambio en el código de la aplicación no invalida su caché.
- **Sin cambio de comportamiento**: ni UI, ni rutas, ni contrato de la API, ni dependencias nuevas.

## Capabilities

### Modified Capabilities

- `ui`: se documenta que el build de producción del panel se divide en chunks por proveedor, con el chunk de entrada por debajo de 500 kB y las dependencias estables cacheadas por separado, sin alterar el comportamiento del panel.

## Impact

- Frontend (`frontend/`): `vite.config.ts` (única fuente del cambio). Sin cambios en `frontend/src/**`, backend, contrato de API ni despliegue.
