# Proposal — creación de quadlets desde el panel

## Why

podpot ya sabe **ver y controlar** las quadlets instaladas (cambio `bootstrap-core`), pero no puede **crear** quadlets nuevas: es la segunda función del producto y sigue sin existir. La referencia `qctl` (`/data/rust/qctl/src/main.rs`) solo **enlaza** ficheros ya existentes hacia `~/.config/containers/systemd`; nunca **escribe** contenido de quadlet. Escribir el fichero es, por tanto, trabajo nuevo de podpot y el objetivo de este cambio.

## What Changes

- **Nueva operación de dominio**: crear una quadlet de un tipo soportado a partir de un **struct tipado** (no contenido libre), validando nombre y tipo, escribiendo el fichero en el directorio canónico configurado y recargando systemd.
- **Nueva ruta** `POST /api/v1/quadlets` en la API REST, con la envoltura JSON existente y errores tipados mapeados a HTTP.
- **Nueva clave de configuración** `PODPOT_QUADLET_DIR` (por defecto `$HOME/.config/containers/systemd`), validada como directorio.
- **Nueva migración** `0002_*.sql` con la tabla de registro de creaciones; primer uso real del pool de SQLite (se preservan WAL y `foreign_keys`).
- **UI**: formulario Antd de creación con selector de tipo y campos mínimos, que refresca el listado al crear y muestra `error.message` al fallar.

Fuera de alcance (**non-goals**): unidades de sistema/root; editar o borrar quadlets existentes; autenticación OIDC; SSE/`PropertiesChanged`.

## Capabilities

### New Capabilities

- `quadlet-creation`: validación (unicidad de nombre, charset y tipo), render desde plantilla tipada, escritura sin pisar, recarga de systemd vía D-Bus, nombres de unidad generados, errores tipados con su mapeo HTTP y registro en persistencia.

### Modified Capabilities

- `api-shell`: nueva ruta `POST /api/v1/quadlets`; se preservan los fallbacks `404`/`405` con la envoltura existente.
- `config`: nueva clave `PODPOT_QUADLET_DIR`, con valor por defecto y validación como directorio.
- `persistencia`: nueva migración con prefijo entero para el registro de creaciones; se preservan WAL y `foreign_keys` en el pool.
- `ui`: formulario de creación y refresco del listado tras crear.

## Impact

- **Código nuevo**: módulo de creación de quadlets en `backend/` (dominio + escritura de ficheros + orquestación con D-Bus y persistencia), ruta en la API, migración `0002_*.sql` y formulario en `frontend/`.
- **Contratos en uso**: trait `SystemdClient` (nueva operación de recarga/`daemon-reload`), envoltura `{data}`/`{error:{code,message}}`, pool `sqlx`.
- **Sistemas**: escribe en el directorio de quadlets del usuario y recarga `org.freedesktop.systemd1` de la instancia de usuario; nunca el manager de sistema.
- **Riesgo**: colisiones de nombre en el directorio de quadlets configurado (`PODPOT_QUADLET_DIR`) y ficheros ya presentes; se mitiga con validación previa por **nombre de fichero exacto** y rechazo tipado **sin** escribir en disco.
