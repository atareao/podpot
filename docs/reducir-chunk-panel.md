# Reducir el chunk del panel (bundle de Vite)

## Qué es

Divide el bundle de producción del panel en **chunks por proveedor** para que el
chunk de entrada deje de cargar con todo y para que las dependencias estables se
cacheen entre despliegues. Antes, `pnpm build` emitía un único `index-*.js` de
**~1.181 kB** —app, antd, React y React Query en el mismo fichero— y Vite avisaba
de que superaba 500 kB. Ahora el código de la aplicación vive solo en el entry
(**16,88 kB**) y el resto se reparte en chunks con hash estable.

Es el cambio OpenSpec `reducir-chunk-panel` (issue #25, plano `PLAN-012`, tipo
⚡ perf / build). **Solo toca la configuración de build del frontend**: sin
cambios de comportamiento, de UI, de contrato de API ni de dependencias.

## Ficheros implicados

### Frontend (`frontend/`)

- `frontend/vite.config.ts` — el único fichero que cambia. Se añade
  `build.rollupOptions.output.manualChunks` con la función `vendorChunk(id)` y las
  RegExp de agrupación como constantes de módulo (`RE_ANTD`, `RE_ANTD_RC`,
  `RE_QUERY`), más la lista `REACT_PACKAGES` que se resuelve con `.some()`. No hay
  cambios en `frontend/src/**`, en `package.json` ni en la sección `test` de
  Vitest.

## Cómo funciona

`vendorChunk` recibe el `id` (ruta absoluta) de cada módulo y decide su chunk; el
código de la aplicación devuelve `undefined` y Rollup lo deja en el chunk de
entrada. El reparto, en orden de comprobación:

1. `node_modules/antd/` → **`vendor-antd`**.
2. `node_modules/@ant-design/`, `node_modules/@rc-component/` o
   `node_modules/rc-*/` → **`vendor-antd-rc`**.
3. `node_modules/@tanstack/` → **`vendor-query`**.
4. `react`, `react-dom`, `react-router`, `react-router-dom`, `scheduler` →
   **`vendor-react`**.
5. Cualquier otra dependencia → **`vendor`**.

La agrupación es **por proveedor** y no por paquete individual: cambiar el
código de la app no re-hashea los chunks de dependencias, así que el navegador
reutiliza su caché entre despliegues.

### Por qué `antd` y `antd-rc` van separados

El criterio inicial era meter todo lo de antd junto, pero ese chunk **pasaba de
500 kB**: antd y su árbol de componentes `rc-*`/`@ant-design/*` suman más que el
umbral por sí solos. Se parte en dos: `vendor-antd` (el paquete `antd` en sí,
454,91 kB) y `vendor-antd-rc` (`@ant-design/*` + `rc-*` fusionados, 379,47 kB).
Los `@ant-design` y `rc-*` **sí** van juntos porque comparten un ciclo de
dependencias mutuo con el propio antd: separarlos obligaría a un tercer chunk
compartido y no baja ninguno de los dos por debajo del margen útil.

## Verificación

- **Baseline:** `pnpm build` con el aviso `Some chunks are larger than 500 kB`
  y un único `index-*.js` de **~1.181 kB**.
- **Tras el cambio:** `pnpm build` **sin el aviso** y entry de **16,88 kB**
  (`dist/assets/index-CcOoDyPv.js`, gzip 6,33 kB). Reparto completo:
  `vendor-antd` 454,91 kB · `vendor-antd-rc` 379,47 kB · `vendor-react`
  260,41 kB · `vendor-query` 35,60 kB · `vendor` 32,90 kB.
- **Hash estable:** dos builds consecutivos dan los **mismos** nombres de fichero
  en los chunks de dependencias.
- **Regresión:** `pnpm test` → **131 tests verdes**; `pnpm run lint` limpio;
  `npx tsc -b` sin errores (lo corre el propio `build`).

## Configuración y variables

- Sin variables de entorno nuevas. Sin cambios en `compose.yaml` ni en el
  despliegue: es una decisión de `vite.config.ts` que se aplica al `pnpm build`.

## Cómo habilitar / despliegue

- Nada especial: el siguiente `pnpm build` ya emite el reparto nuevo. Sin
  migraciones, sin flags y sin tocar el backend.

## Riesgos vivos

- **Cascada de hash por chunks compartidos.** Hay módulos que acaban en `vendor`
  (el genérico) o que varios chunks referencian; tocar `vendor-react` o el
  `vendor` genérico **re-hashea también** `vendor-antd`/`vendor-antd-rc` por las
  dependencias compartidas. El beneficio de caché se mantiene para el caso normal
  (cambiar solo código de app), no para cambios en dependencias.
- **`vendor-antd` a 454,91 kB: margen de ~45 kB** hasta el umbral de aviso. Un
  bump de antd o un import nuevo puede reintroducir el aviso; conviene mirar el
  tamaño tras actualizar dependencias.
- **`id.includes('node_modules')` es amplio.** El reparto asume que todas las
  dependencias están bajo `node_modules` de un solo proyecto. Si algún día hay
  monorepo / workspaces (dependencias enlazadas fuera de ese prefijo), la
  clasificación puede quedar incompleta y hacer falta ajustar el criterio.

## Decisiones

- **Agrupar por proveedor, no por paquete.** Da chunk de entrada pequeño y hash
  estable frente a cambios de app, que es el objetivo; el coste es un `vendor`
  genérico que se re-hashea si cambia cualquier dependencia no clasificada.
- **Separar antd de su árbol `rc-*`.** Obligado por el umbral de 500 kB: juntos
  no caben. Se mantiene `@ant-design`+`rc-*` fusionados por el ciclo mutuo.
- **RegExp como constantes de módulo.** Evita recrearlas en cada módulo durante el
  build y hace el reparto explícito; `REACT_PACKAGES` + `.some()` sustituye al
  predicado multilínea sin cambiar el conjunto de paquetes.

## Follow-ups

- **Lazy-loading de rutas.** Hoy no hace falta: el entry ya está muy por debajo
  del umbral. Si el panel crece y el código de app vuelve a pesar, dividir por
  ruta (`React.lazy` + `Suspense`) es el siguiente paso natural, fuera del alcance
  de este cambio.
- **Revisar el margen de `vendor-antd`** en cada actualización mayor de antd.
