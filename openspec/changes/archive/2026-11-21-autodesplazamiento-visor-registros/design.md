# Design — autodesplazamiento-visor-registros

## Contexto

`CuerpoDeRegistros` (`VisorDeRegistros.tsx`) mantiene un ref `pegadoAlFondo` (por defecto `true`), lo recalcula en `onScroll` con el umbral `UMBRAL_DE_FONDO_PX = 24` y, en un `useEffect([lineas])`, llama `fin.current?.scrollIntoView?.({ block: 'end' })` si el usuario estaba pegado al fondo. El cuerpo se monta y desmonta con la unidad, así que al reabrir el visor el ref vuelve a `true`.

## Decisiones

### Especificar el comportamiento existente, no cambiarlo

El comportamiento es correcto y deseable; el problema es que era un contrato implícito sostenido por una prueba frágil. Se escribe el contrato en `ui` (Phase 0: baseline del comportamiento heredado) y se alinea la prueba con él.

### Prueba determinista del autodesplazamiento

La aserción debe esperar al efecto que hace el desplazamiento, no al mero render del texto. Se sustituye la afirmación inmediata por una espera (`waitFor`) que tolere el volcado por lote, evitando el falso rojo en runners lentos. Opcionalmente se puede forzar el volcado con temporizadores de test; se prefiere la espera por ser menos acoplada a la implementación.

### Umbral explícito en la prueba

Se añade un caso de frontera: con distancia > 24 px respecto al fondo, NO se desplaza; con ≤ 24 px, sí. Fija el valor del umbral como parte del contrato probado.

## Alternativas descartadas

- **Cambiar el componente** (p. ej. desplazar en `useLayoutEffect` o exponer un «flush»): innecesario; el comportamiento es correcto y el flakeo es de la prueba.
- **Marcar el test como `retry`**: oculta el problema en vez de fijar el contrato.
- **`sleep` arbitrario** en la prueba: no determinista y lento.
