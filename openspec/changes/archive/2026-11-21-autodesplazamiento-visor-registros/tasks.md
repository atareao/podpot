# Tasks — autodesplazamiento-visor-registros

TDD por bloques: contrato (spec) → prueba determinista → refactor → verificación → cierre.

## 1. Contrato

- [x] 1.1 Añadir a la delta de `ui` el requirement «Autodesplazamiento condicional del visor de registros» con sus escenarios (pegado al fondo; usuario subido; umbral; reapertura). — Verificación: `openspec validate autodesplazamiento-visor-registros --strict` en verde. — EXIT 0 (`Change 'autodesplazamiento-visor-registros' is valid`).

## 2. Prueba determinista (RED→GREEN)

- [x] 2.1 Hacer determinista «arrastra el scroll al final…»: esperar al autodesplazamiento en lugar de afirmar inmediatamente tras el render. — Verificación: el test pasa de forma fiable en N pasadas (p. ej. 20) consecutivas del fichero. — `await waitFor(...)`; bucle 20/20 en verde, 0 fallos.
- [x] 2.2 Añadir el caso del **umbral**: a >24 px del fondo NO desplaza; a ≤24 px, sí. — Verificación: test rojo→verde del caso de frontera. — ROJO verificado mutando el umbral a `distancia < 24`; VERDE con `<= 24`.

## 3. Refactor

- [x] 3.1 Reutilizar el helper de doblado de `scrollIntoView` y limpiar duplicaciones en el fichero de test, sin duplicar los `Object.defineProperty` de medida. — Verificación: `pnpm run lint` limpio y tests verdes. — Helpers `doblarScrollIntoView`, `simularScroll` y `abrirConPrimeraLinea`; lint EXIT 0.

## 4. Verificación

- [x] 4.1 Ejecutar el fichero N veces sin flake y la suite completa. — Verificación: `pnpm run test` (suite) + bucle estable del fichero, EXIT 0. — Suite 131/131 (17 files); bucle 0/20 fallos.
- [x] 4.2 `pnpm run typecheck`, `pnpm run typecheck:test`, `pnpm run lint`, `pnpm run build`. — Verificación: EXIT 0 en todos. — EXIT 0 en los cuatro.

## 5. Cierre

- [x] 5.1 CI del PR en verde y `just check-all` en verde. — Verificación: salidas EXIT 0.
- [x] 5.2 `docs/autodesplazamiento-visor-registros.md` redactado y `openspec validate --all` en verde. — Verificación: doc existente + validación OK.
