# Proposal

## Why

podpot ya crea quadlets nuevas (issue #3) pero no puede modificarlas ni eliminarlas: el ciclo de vida queda incompleto y obliga a editar o borrar el fichero de la quadlet a mano. Completar edición y borrado cierra la gestión de quadlets desde el panel.

## What Changes

- Nuevo endpoint `PATCH /api/v1/quadlets/{name}` para actualizar una quadlet existente reescribiendo su fichero.
- Nuevo endpoint `DELETE /api/v1/quadlets/{name}` para borrar una quadlet existente eliminando su fichero.
- Reutilización del struct tipado de creación para renderizar el contenido; NUNCA se acepta contenido libre de fichero del cliente.
- Validación/sanitización de valores idéntica a la creación (`\n`, `\r`, `\0` y controles prohibidos → `invalid_value`, `400`) sin tocar el fichero.
- Recarga por D-Bus (`daemon-reload`) tras editar y tras borrar, sin shell-out.
- Errores tipados con el mismo mapeo HTTP que la creación: `not_found` → `404`, `invalid_*` → `400`, `write_failed` → `500`, `reload_failed`/`unit_rejected` → `502`, `transport` → `503`.
- UI: acciones de editar y borrar sobre una quadlet, con confirmación en el borrado (acción destructiva) y refresco del listado tras la operación.
- Persistencia: registrar ediciones y borrados, además de las creaciones ya registradas.

## Capabilities

### New Capabilities

- `quadlet-edition`: edición y borrado de quadlets existentes (fichero + registro), con validación y sanitización de valores, inmutabilidad del nombre, recarga por D-Bus y errores tipados con su mapeo HTTP.

### Modified Capabilities

- `api-shell`: se añaden las rutas `PATCH /api/v1/quadlets/{name}` y `DELETE /api/v1/quadlets/{name}` con la envoltura existente.
- `ui`: se añaden las acciones de editar y borrar, con confirmación y refresco del listado.
- `persistencia`: se añade el registro de ediciones y borrados en una tabla propia.

## Impact

- Backend (`backend/src/`): rutas API nuevas, dominio de edición/borrado, reutilización del render tipado y del cliente D-Bus; nueva migración `0003_*.sql`.
- Frontend (`frontend/src/`): hooks de API y acciones de editar/borrar con confirmación (Ant Design).
- Contrato de API: se extiende `/api/v1/quadlets/{name}` con `PATCH` y `DELETE`.
- Sin cambios en el despliegue ni en la configuración de la quadlet: `PODPOT_QUADLET_DIR` sigue siendo el directorio canónico único.
