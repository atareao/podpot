# FEATURE-002 — creacion-quadlets

> Implementación del change proposal `creacion-quadlets` (`openspec/changes/creacion-quadlets/`).
> Rama `feature/creacion-quadlets`, issue **atareao/podpot#3**.
> Estado: **en cierre**. Plan hermano: `plans/PLAN-002.md`.

## Qué es

El **segundo bloque funcional** de podpot: **crear** quadlets nuevas
(`.container`, `.volume`, `.network`, `.pod`) desde la API y el panel, además de
seguir descubriéndolas y accionándolas.

`bootstrap-core` dejó el sistema completo para **ver y controlar** las quadlets
instaladas, pero sin capacidad de **escribir** ninguna: es la primera vez que
podpot escribe un fichero de quadlet y la primera vez que usa de verdad el pool
de SQLite. La escritura es trabajo nuevo —`qctl` solo **enlaza** ficheros ya
existentes hacia `~/.config/containers/systemd`, nunca **escribe** contenido—.

Una quadlet **es** un fichero de Podman del que el generador de systemd deriva
una unidad, así que crearla son cuatro pasos: **validar** el nombre y el tipo,
**renderizar** el contenido de una plantilla tipada, **escribir** sin pisar y
**recargar** systemd; y solo entonces se **registra** la creación.

## Ficheros implicados

| Ruta | Qué deja |
|---|---|
| `backend/src/quadlet/mod.rs` | el dominio: `QuadletKind`, `QuadletDraft`, `Quadlet`, `QuadletError`, la validación (`stem_valido`), el render de la plantilla por tipo y `valor_de_campo` (sanitización) |
| `backend/src/quadlet/store.rs` | la escritura en disco: `existe` (`symlink_metadata`) y `escribir` (`create_new`, sin pisar, borrón si quedaría a medias) |
| `backend/src/quadlet/service.rs` | la orquestación `crear`: validar → unicidad → escribir → recargar → registrar |
| `backend/src/api/quadlets.rs` | `POST /api/v1/quadlets` y el `QuadletOutcome` de la respuesta |
| `backend/src/error.rs` | `AppError::Quadlet` y `traducir_quadlet`: el mapeo tipado a HTTP |
| `backend/src/config.rs` | `PODPOT_QUADLET_DIR` (`QUADLET_DIR_VAR`) y `Config::quadlet_dir`, validada como directorio |
| `backend/migrations/0002_quadlet_creations.sql` | la tabla `quadlet_creations` (registro de las creadas por podpot) |
| `backend/src/api/mod.rs` | registro de la ruta `POST /quadlets` |
| `backend/src/systemd/mod.rs` | la operación `reload` (`daemon-reload`) en el trait `SystemdClient`, con su variante `SystemdError::ReloadFailed` |
| `frontend/src/api/quadlets.ts` | `crearQuadlet`: el `POST` con cuerpo plano |
| `frontend/src/types/api.ts` | `TipoDeQuadlet`, `PeticionDeCreacion`, `QuadletCreada` |
| `frontend/src/components/FormularioDeCreacion/FormularioDeCreacion.tsx` | el formulario Antd (selector de tipo + campos mínimos por tipo) |
| `frontend/src/hooks/useUnits.ts` | `useCrearQuadlet`: la mutación que invalida el listado al crear |
| `frontend/src/pages/unidades.tsx` | conecta el formulario, avisa del desenlace y refresca la tabla |

## Cómo funciona

### Flujo: validar → render → unicidad → escribir → recargar → registrar

`quadlet/service.rs::crear` pone los pasos en fila y **el orden no es
negociable**:

1. **Validar** nombre y tipo sin tocar el disco (`QuadletDraft::quadlet` →
   `validate` + `kind.unit_name`).
2. **Renderizar** el contenido desde los campos tipados (`QuadletDraft::render`):
   nunca desde un cuerpo que mande el cliente.
3. **Comprobar la unicidad** del `<stem>.<kind>` en el directorio configurado
   (`store::existe`): el destino **no se toca** si el nombre está en uso.
4. **Escribir** sin pisar (`store::escribir`, `create_new`). La vuelta a
   comprobar la da el propio sistema de ficheros.
5. **Recargar** systemd por D-Bus (`SystemdClient::reload`): sin recarga, la
   unidad de la quadlet no existe.
6. **Registrar** la creación en SQLite. Va **el último** a propósito: nunca
   puede quedar registrada una creación que no ocurrió. Un fallo en `reload`
   deja el fichero en disco (borrarlo sería deshacer una escritura real) pero
   **no** registra y lo deja anotado en el log.

### Plantilla tipada y nombres de unidad

El render por tipo es **determinista** y el orden de líneas es fijo (secciones y
claves las pone podpot; de la petición solo salen los **valores** de campos
declarados):

| Tipo | Sección | Claves |
|---|---|---|
| `.container` | `[Container]` | `Image=` (obligatoria), `ContainerName=`, `PublishPort=`, `Volume=`, `Environment=` |
| `.volume` | `[Volume]` | `VolumeName=` |
| `.network` | `[Network]` | `NetworkName=`, `Subnet=` |
| `.pod` | `[Pod]` | `PodName=`, `PublishPort=` |

El nombre de **unidad** que genera systemd no es el mismo para todos:

- `.container` → `<stem>.service` (el generador **no** añade sufijo)
- `volume | network | pod` → `<stem>-<ext>.service`

El nombre se construye y luego pasa por `UnitName::parse`: lo que se devuelve es
un nombre de unidad de systemd de verdad, no un `format!` suelto. Un `name` que
ya traiga la extensión (`web.container`) se recorta, para no acabar en
`web.container.container`.

### Unicidad y colisiones

La unicidad es por **nombre de fichero completo** (`<stem>.<kind>`), no por stem:
`web.container` y `web.volume` **conviven** y cada uno genera su propia unidad.
La comprobación es con `symlink_metadata` (un **symlink** roto también ocupa el
nombre) y la escritura es `create_new`, así que la negativa a pisar no es una
carrera entre un `stat` y un `open`: la da el sistema de ficheros.

## Contrato de la API

`POST /api/v1/quadlets` crea una quadlet de un tipo soportado y responde `201`.

**Request** (JSON **plano**, sin `fields`; el backend usa `deny_unknown_fields`):

```json
{
  "name": "web",
  "kind": "container",
  "image": "docker.io/library/nginx:latest",
  "container_name": "web",
  "publish_ports": ["8080:80"],
  "volumes": ["web-data:/data:Z"],
  "environments": ["TZ=Europe/Madrid"],
  "volume_name": null,
  "network_name": null,
  "subnet": null,
  "pod_name": null
}
```

Solo `.container` exige un campo (`image`); el resto de campos son opcionales y
los que no aplican a un tipo se ignoran. No existe ninguna clave `fields` ni
anidada: un campo de más o un cuerpo de fichero libre dan `400`.

**Response**:

```json
{ "data": { "file_name": "web.container", "unit_name": "web.service", "kind": "container" } }
```

La respuesta **no** publica la ruta escrita: dónde vive la quadlet es un detalle
del sistema de ficheros del usuario.

**Errores** — envoltura única `{ "error": { "code": "<snake_case>", "message": "<texto>" } }`:

| Error de dominio | HTTP | `code` publicado |
|---|---|---|
| `Duplicate` | `409` | `duplicate` |
| `InvalidName` | `400` | `invalid_name` |
| `InvalidKind` | `400` | `invalid_kind` |
| `MissingField` | `400` | `bad_request` |
| `InvalidValue` | `400` | `invalid_value` |
| `WriteFailed` | `500` | `write_failed` |
| `Persistencia` | `500` | `internal` |
| `Systemd::ReloadFailed` | `502` | `systemd_reload_failed` |
| `Systemd::UnitRejected` | `502` | `systemd_unit_rejected` |
| `Systemd::Transport` | `503` | `systemd_unavailable` |

Los `4xx` explican el motivo **nombrando el campo** (nunca repitiendo el valor que
llegó); los `5xx` publican un mensaje genérico y el detalle se queda en el
registro. Se preservan los fallbacks `404`/`405` con la envoltura (un `GET` sobre
`/api/v1/quadlets` da `405`).

## Configuración

| Variable | Obligatoria | Defecto | Qué hace |
|---|---|---|---|
| `PODPOT_QUADLET_DIR` | no | `$HOME/.config/containers/systemd` | directorio donde se escriben las quadlets nuevas. Se **valida como directorio existente** al arrancar |

Una variable **definida pero que no es un directorio** es error de arranque (no
cae al defecto en silencio): quien la puso creía que se iba a usar. Sin la
variable, el destino cuelga de `$HOME` (y sin `$HOME` el arranque falla). El
mensaje de error **nombra** la variable pero nunca su valor.

En el contenedor, ese directorio tiene que estar **montado** (`--userns=keep-id`
+ montar `${HOME}/.config/containers/systemd` y fijar `HOME` al del host),
siguiendo la receta del spike y el patrón de `compose.yaml` de `bootstrap-core`.

## Seguridad

La defensa es **estructural**, no una lista de expresiones regulares de
limpieza:

1. **Sin contenido libre.** `deny_unknown_fields` cierra la puerta a un campo
   `content` o a cualquier otro que no esté declarado: el fichero se **deriva**
   de campos, nunca se copia.
2. **Sanitización de valores** (`valor_de_campo`): un valor acaba en una línea
   `Clave=valor`, así que se rechaza con `InvalidValue { field }` si viene vacío,
   con `\n`, `\r`, `\0` o cualquier **carácter de control** —un salto de línea
   sería una directiva o una sección nueva colada por la puerta del valor—. El
   rechazo nombra el **campo** y nunca el valor.
3. **Charset del stem** (`stem_valido`): empieza por letra o dígito y sigue con
   letras, dígitos, `-`, `_` o `.`; hasta 200 caracteres; ASCII. Deja fuera lo que
   convertiría el nombre en una ruta (`/`, `..`), lo que systemd no leería
   (espacios, controles) y los ficheros ocultos (un stem que empieza por `.`).
4. **Tipo cerrado.** La extensión se lee con `QuadletKind::from_extension`: lo
   que no sea `container`/`volume`/`network`/`pod` es `InvalidKind` y **no** un
   valor por defecto (`.desktop` sale del mismo generador sin ser quadlet; `.kube`,
   `.image` y `.build` se ven pero no se crean).
5. **`create_new` atómico.** No hay ventana entre comprobar y escribir; un
   `AlreadyExists` se traduce a `409` (duplicado) y no a `500`. Y es **inmune a
   symlinks**: escribir sobre un enlace apuntado a un fichero ajeno no ocurre
   porque el nombre ya está ocupado.

## Decisiones

1. **Directorio canónico único, sin symlinks.** Se escribe directamente en el
   directorio que lee el generador de systemd de usuario
   (`PODPOT_QUADLET_DIR`). Es una **divergencia consciente** de `qctl`, que
   separa fuente y *target* y solo crea enlaces. Aquí lo que podpot crea es lo
   que systemd lee, sin una capa de indirección que nadie pidió.
2. **v1 solo crea.** Editar y borrar quedan fuera del alcance: crear es la
   función nueva y las operaciones destructivas merecen su propio diseño.
3. **Unicornidad por `file_name`, no por stem.** `web.container` y `web.volume`
   conviven; cada tipo genera su unidad. Es el ajuste del spec a la decisión de
   directorio canónico único.
4. **`Duplicate` → `409` y `WriteFailed` → `500`.** Un nombre ya en uso es algo
   que quien llama puede arreglar; un fallo de escritura es del servidor. El
   `create_new` garantiza que la carrera se resuelve como `409` y no como `500`.
5. **Registrar el último.** El requisito es que nunca quede registrada una
   creación que no ocurrió; por eso el `INSERT` va después de escribir y de
   recargar, y una recarga fallida no deja apunte (sí deja el fichero y una traza
   en el log).
6. **Migración con prefijo numérico entero** (`0002_…`, no `V002__…`): el
   migrador embebido de `sqlx` corta el nombre por el primer `_` y lee la primera
   parte como número.

## Follow-ups (deuda pendiente)

- **Editar y borrar quadlets** existentes (fuera de alcance de v1).
- **Exponer los campos opcionales restantes en la UI**: hoy el formulario solo
  pide `image` para `.container`; `subnet`, `pod_name`, `volumes`, `environments`,
  `container_name` y `publish_ports` existen en el contrato pero aún no tienen
  control en el formulario.
- **Autenticación OIDC** opcional (patrón kesto) para exponer más allá de
  loopback; escribir ficheros sin auth es delicado.
- **SSE con `PropertiesChanged`**: tras crear, el listado se refresca por
  `invalidateQueries`; un *push* de systemd lo haría en vivo.
- **Chunk de Antd > 500 kB**: el build avisa; conviene *code-splitting*
  (`manualChunks`).
- **Unicidad entre directorios fuente**: hoy se comprueba solo en el directorio
  configurado; `qctl` mira más orígenes.

## Verificación

- `just check-all` en verde (fmt → clippy → test, incluye `ui-*`).
- `openspec validate creacion-quadlets --strict` sin errores.

## Lo que NO trae esta entrega

Sin editar ni borrar quadlets, sin autenticación, sin SSE y sin exponer todos los
campos opcionales en el panel.
