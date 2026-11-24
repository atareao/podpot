# Change Proposal: chunk-antd — Reducir la huella de antd en el bundle

## Intent

La deuda 2 (heredada de `docs/reducir-chunk-panel.md` → «Riesgos vivos» y HANDOFF)
pide reducir el chunk de Ant Design por debajo de 500 kB. Hoy `vendor-antd`
(454,91 kB) y `vendor-antd-rc` (379,47 kB) superan **juntos** el umbral de aviso
de Vite (500 kB, tamaño **raw**, no gzip): 834,38 kB. El margen de `vendor-antd`
respecto del umbral es de ~45 kB: un bump de antd reintroduce el aviso.

**Criterio de aceptación:** la suma de los tamaños raw de todos los chunks
atribuibles a antd (`vendor-antd*`) SHALL ser ≤ 500 kB, y cada chunk individual
< 500 kB, sin cambio de comportamiento observable (mismos rutas, mismas
acciones, tests existentes en verde salvo adaptación mínima documentada).

## Scope

- `frontend/src/**` — superficie antd usada (imports y uso de componentes).
- `frontend/vite.config.ts` — solo si el reparto de chunks lo requiere.
- NO se añaden dependencias nuevas. NO cambia el backend ni el contrato de API.

## Impact

- Carga inicial más ligera y margen sano frente al umbral de aviso de Vite.
- Los 131 tests existentes protegen el comportamiento: cualquier sustitución de
  componente debe mantenerlos verdes (adaptación mínima documentada si el rol
  accesible cambia).

## Vías permitidas (por orden de preferencia)

1. Recortar superficie antd no usada.
2. Sustituir componentes pesados (Table, Form, Select, Drawer) por equivalentes
   ligeros, mantiendo los tests verdes.
3. Lazy-loading de zonas pesadas como complemento (no reduce la huella total,
   solo la carga inicial).

## Métrica (fijada en la especificación)

Tamaños **raw** (bytes sin comprimir, como el umbral de `chunkSizeWarningLimit`
de Vite), verificados por `frontend/tests/tamanoDelBundle.test.ts`.
