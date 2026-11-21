# Design — exponer-identidad-quadlet

## Contexto

`GET /api/v1/units` publica `name`, `active_state`, `sub_state` y `unit_file_state`. La identidad con la que se opera una quadlet (`PATCH`/`DELETE /api/v1/quadlets/{name}`) es el **nombre de fichero** `<stem>.<kind>`, pero ese nombre no viaja al cliente: el frontend lo reconstruye invirtiendo `QuadletKind::unit_name`, una regla **no inyectiva** (`web-volume.container` y `web.volume` colisionan en `web-volume.service`).

El dominio ya dispone del dato real: `UnitStatus.origin: UnitOrigin` envuelve el `SourcePath` que systemd informa (leído en `DbusSystemdClient::origen`, `dbus.rs:147`). El basename de esa ruta es el nombre de fichero de la quadlet.

## Decisiones

### Publicar el basename, no la ruta

`UnitSummary` gana `file_name: String`, calculado como el **basename** de `UnitOrigin`. No se publica la ruta completa: el requirement de `api-shell` dice que el listado no publica procedencia del sistema de ficheros, y un basename no es una ruta. Se preserva la intención de privacidad (`UnitOrigin` sigue sin serializarse: `#[serde(skip)]`).

### El dato derivado vive en el dominio

El basename se expone como accesor en el dominio (`UnitOrigin::file_name()` → `Option<&str>`, o `UnitStatus::file_name()`), no se calcula en la capa API. Así se prueba con el doble, sin D-Bus ni HTTP, y el doble y el cliente real comparten la regla —como el resto del proyecto—. El accesor devuelve `None` si el origen es desconocido (no debería ocurrir: el listado ya filtra a quadlets, que **siempre** tienen fichero).

### `kind` opcional

`kind` se deriva de la extensión del `file_name` con `QuadletKind::from_extension`. Como esa función solo conoce los cuatro tipos **creables** (`container`/`volume`/`network`/`pod`), y el listado puede incluir quadlets con extensiones desconocidas (systemd las considera quadlets por prudencia), `kind` se publica como `Option<QuadletKind>` (`null` cuando no se reconoce). El frontend no necesita `kind` para operar —la edición obtiene el tipo del `GET /api/v1/quadlets/{name}`—, pero su publicación completa la identidad y elimina toda deducción.

### El frontend deja de deducir

`identidadDeUnidad` se elimina. `QuadletUnit` gana `file_name` (y `kind: TipoDeQuadlet | null`). `TablaDeUnidades` y `pages/unidades.tsx` usan `unit.file_name` directamente. La precarga de edición ya lee el tipo real vía `GET /api/v1/quadlets/{name}`, así que no cambia.

## Alternativas descartadas

- **Publicar `source_path` completo**: filtra rutas del usuario y contraviene la decisión documentada de no publicar procedencia. El basename basta.
- **Mantener la deducción como fallback permanente**: perpetúa la ambigüedad no inyectiva; se elimina en lugar de arrastrarla.
- **Derivar el basename en la capa API**: duplica la regla fuera del dominio y la deja sin cobertura por el doble.
