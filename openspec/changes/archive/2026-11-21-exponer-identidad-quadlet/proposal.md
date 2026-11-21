# Proposal

## Why

El panel deriva la identidad de una quadlet (`<stem>.<kind>`, el nombre de fichero con el que se opera `PATCH`/`DELETE`) **invirtiendo** el nombre de unidad que publica `GET /api/v1/units` (`frontend/src/quadlets/identidad.ts`, `identidadDeUnidad`). Esa regla inversa **no es inyectiva**: un contenedor con stem `web-volume` y un volumen `web` generan ambos la unidad `web-volume.service`, y la deducción resuelve siempre a `web.volume` —el fichero equivocado—. Editar o borrar en ese caso colisionado apunta al fichero incorrecto; la mitigación actual (un `GET` de verificación antes del `DELETE`) reduce el daño pero no elimina la ambigüedad.

El backend ya tiene la identidad real: `UnitStatus.origin` guarda el `SourcePath` que systemd informa para la unidad, y el **basename** de esa ruta *es* el nombre de fichero de la quadlet. Hoy se descarta en `UnitSummary` por prudencia (no publicar rutas del sistema de ficheros del usuario). Exponer **solo el basename** (no la ruta) resuelve la identidad sin filtrar rutas.

## What Changes

- `GET /api/v1/units` SHALL incluir, por cada quadlet, su `file_name` (el basename de `SourcePath`, p. ej. `web.container`), que es la identidad real de la quadlet. NUNCA se publica la ruta completa.
- Se expone además `kind` (el tipo derivado de la extensión del fichero) cuando la extensión corresponde a un tipo conocido; es `null`/ausente para extensiones no reconocidas.
- El dominio (`UnitStatus`/`UnitOrigin`) SHALL exponer el nombre de fichero de la quadlet (basename) como dato derivado y **testeable**, sin publicar la ruta.
- El frontend SHALL usar el `file_name` que publica la API como identidad de la quadlet para `PATCH`/`DELETE`, y SHALL **dejar de deducirla** del nombre de unidad. `identidadDeUnidad` se elimina (o queda solo como fallback documentado si no hay `file_name`).

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `api-shell`: el listado `GET /api/v1/units` pasa a publicar `file_name` (y `kind` opcional) por quadlet; se añade el requirement del listado, hoy ausente de la spec.
- `systemd-dbus`: `UnitStatus`/`UnitOrigin` exponen el nombre de fichero de la quadlet (basename del `SourcePath`) como dato derivado.
- `ui`: el listado y las acciones usan el `file_name` publicado por la API como identidad, sin deducción inversa.

## Impact

- Backend (`backend/src/`): `UnitSummary` y su `From<UnitStatus>` (`api/units.rs`); un accesor de nombre de fichero en `UnitOrigin`/`UnitStatus` (`systemd/mod.rs`); el doble de pruebas (`systemd/fake.rs`) ya construye `UnitStatus` con origen y no cambia.
- Frontend (`frontend/src/`): `types/api.ts` (`QuadletUnit`), `components/TablaDeUnidades`, `pages/unidades.tsx`, y retirada de `quadlets/identidad.ts` (+ su test).
- Contrato de API: se **añade** un campo al listado; cambio aditivo, sin romper consumidores.
- Sin cambios en despliegue, configuración ni persistencia.
