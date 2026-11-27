# Agrupación por prefijo y override con `groups.toml`

## Qué es

Evolución de la agrupación por stack introducida en `docs/agrupacion-por-stack.md`. Donde antes el stack era el nombre base completo del fichero (o el de la unidad), ahora se deriva del **prefijo antes del primer `-`**, y ese valor puede sobrescribirse de forma explícita mediante un fichero `groups.toml`.

Así, `atareao-mariadb.container`, `atareao-mariadb-data.volume` y `atareao-network.network` caen todos bajo el stack `atareao`, sin necesidad de configuración. Si se quiere otro agrupamiento, se declara en `groups.toml`.

## Ficheros implicados

- `backend/src/quadlet/stack.rs`: `stack_name` aplica la regla de prefijo; `stack_name_with_groups` aplica el override de `groups.toml` (por `file_name` o por `unit_name`).
- `backend/src/quadlet/groups.rs`: carga y fusión de `groups.toml` desde las fuentes reales (`GroupsConfig`, `load_groups`, `source_dirs`, `load_groups_para`).
- `backend/src/quadlet/mod.rs`: exposición del módulo `groups`.
- `backend/src/api/units.rs`: `from_con_grupos` usa el mapa de grupos al poblar `UnitSummary.stack` en `GET /api/v1/units`.
- `backend/src/api/stream.rs`: la instantánea inicial y cada evento SSE usan el mismo mapper con grupos.
- `frontend/src/...`: sin cambios — el panel ya agrupa por el valor `stack` que envía el backend.

## Cómo funciona

### Regla base

1. Se toma el `file_name` de la quadlet (el basename de la procedencia) y se le quita la extensión de quadlet (`.container`, `.volume`, `.network`, `.pod`, …).
2. El stack es el primer segmento del stem antes del primer `-`, conservando la grafía original. Ejemplos:
   - `atareao-mariadb.container` → `atareao`
   - `standalone.container` → `standalone`
   - `MiPila-Volume.volume` → `MiPila`
3. Si el stem empieza por `-`, no se recorta: `-VOLUME.volume` → `-VOLUME`.
4. Si no hay `file_name`, la extensión no es de quadlet o el `kind` no concuerda con la extensión, se usa el nombre de la unidad sin `.service` y se le aplica la misma regla de prefijo:
   - `centinela-app.service` → `centinela`
   - `singleton.service` → `singleton`

El campo `stack` sigue siendo estable: no depende de `active_state` ni de `sub_state`, así que los eventos SSE no cambian de grupo a una unidad.

### Override con `groups.toml`

Cada directorio fuente de quadlets puede contener un `groups.toml`:

```toml
[mappings]
"atareao-mariadb.container" = "db-cluster"
"centinela-app.service" = "monitoring"
```

- Las claves son `file_name` (p. ej. `atareao-mariadb.container`) o `unit_name` (p. ej. `centinela-app.service`).
- El valor es el nombre del stack que se quiere.
- El override tiene prioridad absoluta sobre la regla base. Si una unidad tiene entrada por `file_name`, gana; si no, se mira `unit_name`; si no hay ninguna, se aplica la regla base.
- Se buscan `groups.toml` en los directorios que son **fuente real** de las unidades: el directorio de la procedencia de cada quadlet (`UnitStatus.origin`) y `PODPOT_QUADLET_DIR` (o `$HOME/.config/containers/systemd/` por defecto). El directorio de trabajo del proceso **no** se consulta: un `groups.toml` accidental en el CWD no afecta a unidades que no vienen de ahí.
- La base configurada se carga primero y las procedencias después (ordenadas, sin duplicados), de modo que la fuente de una unidad puede anular la base y el resultado no depende del orden del listado.
- Los ficheros ausentes, ilegibles o mal formados se ignoran con una traza (`tracing::warn!` con ruta y causa): no borran los mapeos ya cargados de otros directorios y no rompen el listado.
- No hay caché: el mapa se recarga en cada petición a `GET /api/v1/units` y en cada instantánea del flujo SSE. Cambiar un `groups.toml` surte efecto en la siguiente petición o evento, sin reconectar ni reiniciar.
- Un override vacío o formado sólo por espacios se descarta y cae a la regla base.

## Variables/configuración

No hay variables de entorno nuevas. La única configuración es el fichero `groups.toml` en los directorios indicados arriba.

## Cómo habilitar o desplegar

- Sin `groups.toml`: funciona automáticamente con la regla de prefijo.
- Con override: crear un `groups.toml` con una tabla `[mappings]` en uno de los directorios fuente y escribir las entradas deseadas. No hace falta reiniciar.
- El contrato de la API no cambia: `UnitSummary.stack` sigue siendo un `String` aditivo. Los clientes actuales siguen funcionando.

## Decisiones y follow-ups

- La regla de prefijo se aplica también al fallback de `unit_name` para que `centinela-app.service` y `centinela-app.container` caigan en el mismo stack.
- El override por `unit_name` se mira después del de `file_name`: permite fijar un nombre de unidad concreto sin depender de cómo se llame el fichero.
- La fusión se hace sobre fuentes reales, no sobre el directorio de trabajo: el `groups.toml` de la procedencia gana al de la base configurada. Así la configuración del usuario puede anular el destino por defecto sin que un fichero ajeno cambie el stack por accidente.
- Se descartó una caché con vigilancia del filesystem en el MVP: recargar un fichero pequeño por petición es más simple y evita estados obsoletos. Si el número de peticiones crece, se puede añadir una caché con `notify` como follow-up.
- Fuera de alcance: editar `groups.toml` desde la UI, agrupar en vistas de histórico/logs y persistir grupos en SQLite.
