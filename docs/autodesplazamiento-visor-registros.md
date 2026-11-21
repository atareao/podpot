# autodesplazamiento-visor-registros — contrato del scroll del visor de logs

## Qué es

Especifica el **autodesplazamiento condicional** del visor de registros (un comportamiento heredado que no estaba escrito) y **estabiliza su prueba**, que fallaba de forma intermitente en CI. No cambia el comportamiento del componente.

Issue: #18 · Change OpenSpec: `openspec/changes/archive/2026-11-21-autodesplazamiento-visor-registros` · Plan: `plans/PLAN-009-ARCHIVED.md`.

## Ficheros implicados

- `frontend/src/components/VisorDeRegistros/VisorDeRegistros.test.tsx` — prueba endurecida (aserción determinista, caso de umbral, reapertura; helpers `simularScroll`/`abrirConPrimeraLinea`).
- `frontend/src/components/VisorDeRegistros/VisorDeRegistros.tsx` — **sin cambios** (referencia: `UMBRAL_DE_FONDO_PX = 24`, ref `pegadoAlFondo`, `onScroll`, `useEffect([lineas])`).
- `openspec/specs/ui/spec.md` — requirement «Autodesplazamiento condicional del visor de registros».

## Cómo funciona

El cuerpo del visor (`CuerpoDeRegistros`) mantiene un ref `pegadoAlFondo` (por defecto `true`). En `onScroll` recalcula la distancia al fondo (`scrollHeight − scrollTop − clientHeight`) y considera «pegado» si es ≤ 24 px. En un `useEffect([lineas])`, si está pegado, llama `fin.current?.scrollIntoView?.({ block: 'end' })`. El cuerpo se monta/desmonta con la unidad, así que al **reabrir** el visor el ref vuelve a `true`.

## Contrato

- Con el usuario **pegado al fondo** (≤ 24 px) → al llegar líneas nuevas, el visor se desplaza al fondo.
- Si el usuario **subió** (> 24 px) → NO se le desplaza el scroll.
- **Reapertura** → se parte de «pegado al fondo».

## Configuración / despliegue

Sin configuración ni cambios de despliegue (UI pura).

## Decisiones

- **Especificar sin cambiar** el comportamiento heredado: el fallo era una prueba frágil sobre un contrato implícito, no un defecto del componente.
- **Prueba determinista**: la aserción espera al efecto (`waitFor`) en vez de afirmar tras el render, eliminando la dependencia del timing del runner (el desplazamiento ocurre tras el volcado por lote del hook, ~150 ms).
- **Umbral probado en la frontera** (24 px desplaza, 25 px no) para fijar el valor como parte del contrato.

## Follow-ups

- El flakeo era **preexistente** y ajeno a la feature de progreso de trabajos (PR #17); con esta prueba endurecida queda cubierto.
