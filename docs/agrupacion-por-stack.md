# Agrupación de quadlets por stack

## Qué es

El listado de unidades del panel agrupa las quadlets que forman una misma aplicación. `foo.container`, `foo-volume.volume` y `foo-network.network` aparecen juntas bajo el stack `foo`, con el estado y las acciones de cada unidad conservados.

## Ficheros implicados

- `backend/src/quadlet/stack.rs`: función pura `stack_name`, que deriva el stack de `file_name` y `kind`.
- `backend/src/quadlet/mod.rs`: exposición del módulo.
- `backend/src/api/units.rs`: campo aditivo `stack: String` en `UnitSummary`, población en el mapper de `GET /api/v1/units` y tests de contrato JSON.
- `frontend/src/types/api.ts`: espejo TS con `stack: string`.
- `frontend/src/lib/groupByStack.ts` (+ test): helper puro de agrupación y orden determinista.
- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx`: cabeceras de stack con `rowSpan`, filtros por `kind` y `active_state`, estado vacío explícito.
- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.filtros.test.tsx`: tests de grupos, filtros combinados y estabilidad ante SSE.
- `frontend/tests/fixtures.ts`: `stack` por defecto en las fixtures.

## Cómo funciona

La extensión real del fichero quadlet fija el nombre base y la clasificación. Los tipos principales (`container`, `pod`, `kube`, `build`, `image`) conservan el nombre base completo. Los auxiliares (`volume`, `network`) recortan el sufijo `-volume`/`-network` de forma case-insensitive conservando la grafía, solo si queda prefijo no vacío; si no, forman su propio stack. Sin `file_name`, con extensión no reconocida o incoherente con `kind`, se usa el nombre de la unidad sin `.service` como fallback. `kube`/`image`/`build` se tratan como principales aunque `kind` sea `None`, porque `QuadletKind` solo modela los tipos creables.

El campo `stack` es estable: no depende de `active_state` ni de `sub_state`, por lo que los eventos SSE actualizan el estado sin reordenar filas. El frontend ordena stacks de forma case-insensitive y, dentro de cada uno, principales antes que auxiliares y luego por nombre.

Los filtros por tipo y estado son combinables (AND) y se derivan de la snapshot completa de unidades. La identidad de cada fila sigue siendo `file_name`; `name` se mantiene como dato secundario y para cotejar el progreso de acciones.

## Variables/configuración

No hay variables nuevas ni cambios de configuración.

## Cómo habilitar o desplegar

No requiere activación: está disponible al abrir el panel. No hay migración SQLite ni cambios en el descubrimiento D-Bus. El contrato de la API es aditivo, por lo que los clientes actuales siguen funcionando.

## Decisiones y follow-ups

- El stack se calcula en el backend para tener una sola fuente de verdad; el frontend no reinterpreta nombres.
- **Actualizado en PLAN-028**: la regla base ahora toma el prefijo antes del primer `-` y admite override vía `groups.toml`. Ver `docs/agrupacion-por-prefijo.md`.
- No se amplió `QuadletKind`: eso cambiaría el contrato de creación y el descubrimiento.
- Fuera de alcance: grupos explícitos del usuario (`X-Podpot-Group=`, `groups.toml`) y lectura del directorio `available` de qctl.
- Follow-up posible: añadir un grupo explícito como override del stack derivado.
