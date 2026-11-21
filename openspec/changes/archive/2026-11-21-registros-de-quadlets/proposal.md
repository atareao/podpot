# Proposal

## Why

El panel muestra el estado de las quadlets en vivo, pero **no permite ver los logs** del contenedor. Diagnosticar por qué falla una unidad obliga a salir del panel. Es la capacidad declarada fuera de alcance en `docs/unidades-en-vivo.md` y el complemento natural del panel en vivo.

## What Changes

- Nuevo endpoint **`GET /api/v1/units/{name}/logs/stream`** (`text/event-stream`): al conectar envía las últimas líneas (cola configurable) y luego las nuevas según se producen (`follow`). Un evento `log` por línea con su origen (stdout/stderr); `retry`, latido y cierre con el servidor.
- **Fuente: la API de Podman por el socket unix** (`$XDG_RUNTIME_DIR/podman/podman.sock`), **no** shell-out (coherente con la política del proyecto). El socket ya está montado en el contenedor (bajo `/run/user/<uid>`).
- **Mapeo unidad → contenedor por label** `PODMAN_SYSTEMD_UNIT=<unidad>.service` (verificado en esta máquina: los contenedores de quadlets la llevan). Si no hay contenedor (unidad sin contenedor, `.volume`/`.network`), respuesta explícita.
- **Puerto `FuenteDeRegistros`** con doble de pruebas, para probar sin socket real (patrón `SystemdClient`/`FakeSystemd`).
- **Cliente HTTP ligero** por socket unix (`hyper` + `hyperlocal`); **sin** bollard (árbol de deps 3× mayor) ni shell-out. Demultiplexado del stream de logs como función pura, probado aparte.
- **Frontend**: visor de logs (drawer) abierto desde una acción de fila en la tabla de unidades, alimentado por el canal SSE; baja al cerrar y degradación si el canal no está.

## Capabilities

### New Capabilities

- `registros`: el contrato del canal SSE de logs (formato de eventos, cola inicial, `follow`, cierre) y del puerto que lo alimenta desde la API de Podman.

### Modified Capabilities

- `config`: variable del socket de Podman (`PODPOT_PODMAN_SOCKET`, con defecto) y modo de fuente para pruebas.
- `api-shell`: nueva ruta `GET /api/v1/units/{name}/logs/stream` (bajo la mitad protegida, como el resto de negocio).
- `ui`: visor de logs en el panel.

## Impact

- `backend/Cargo.toml`: `hyper` (client,http1) + `hyperlocal` (+ `http-body-util` a dep normal). Sin bollard.
- `backend/src/`: nuevo módulo de registros (cliente Podman por socket + demux + puerto + doble) y `api/logs.rs` (handler SSE); `config.rs` (socket); `state.rs` (fuente de registros); `api/mod.rs` (ruta).
- Frontend (`frontend/src/`): cliente/hook del canal de logs y drawer del visor.
- Contrato de API: **aditivo**.
- Despliegue: sin cambios (el socket ya es accesible bajo `/run/user/<uid>`); se documenta.

## Fuera de alcance

- Progreso de acciones por job (el panel en vivo ya refleja las transiciones de `ActiveState`).
- Descarga a fichero; búsqueda/filtrado en el visor.
- Logs de unidades que no son contenedores.
