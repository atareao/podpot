# Proposal

## Why

El visor de registros (`VisorDeRegistros`) se autodesplaza al fondo cuando llegan líneas nuevas, salvo que el usuario haya subido a leer. Ese comportamiento **no está especificado** en ninguna capability, y su prueba (`VisorDeRegistros.test.tsx`) afirma de inmediato tras la aparición de la línea, mientras que el desplazamiento lo dispara un efecto que corre tras el volcado por lote del hook (~150 ms). Resultado: el test falla de forma **intermitente** en CI (pasó al reintentar). Es un contrato no escrito sostenido por una prueba frágil.

## What Changes

- **Especificar** el autodesplazamiento condicional del visor en la capability `ui`: se desplaza al fondo al llegar líneas nuevas cuando el usuario está pegado al fondo (umbral de 24 px) y NO lo hace si subió a leer; al reabrir el visor se parte de «pegado al fondo».
- **Endurecer la prueba** para que espere de forma determinista al autodesplazamiento (sin depender del timing del runner) y cubra el umbral.
- **Sin cambios de comportamiento** del componente: la implementación ya cumple lo que se especifica; el trabajo es de contrato y de estabilidad de la prueba.

## Capabilities

### Modified Capabilities

- `ui`: se añade el requirement de **autodesplazamiento condicional** del visor de registros (umbral de 24 px; reapertura vuelve a «pegado al fondo»).

## Impact

- Frontend (`frontend/src/components/VisorDeRegistros/`): solo la prueba; sin cambio de comportamiento del componente.
- Contrato de API: ninguno (es UI pura).
- Despliegue / backend: sin cambios.
