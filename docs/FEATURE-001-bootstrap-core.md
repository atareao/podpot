# FEATURE-001 — bootstrap-core

> Implementación del change proposal `bootstrap-core` (`openspec/changes/bootstrap-core/`).
> Rama `feature/bootstrap-core`, issue **atareao/podpot#1**.
> Estado: **en cierre**. Plan hermano: `plans/PLAN-001.md`.

## Qué es

El **esqueleto caminante** de podpot: un panel web para ver el estado de las
**quadlets de Podman** de la instancia **rootless** del systemd de usuario y
controlarlas (`start` / `stop` / `restart`).

Esta entrega deja el sistema completo de extremo a extremo —backend Rust+Axum,
capa D-Bus contra systemd, API REST, persistencia SQLite, panel React+AntD y
contenedor— mostrando el estado real y accionando unidades. La **creación** de
quadlets queda para un cambio posterior.

## Ficheros implicados

| Ruta | Qué deja |
|---|---|
| `backend/src/main.rs` | arranque: configuración, tracing, panel, escucha y apagado ordenado (`SIGTERM`/`SIGINT`) |
| `backend/src/lib.rs` | raíz del crate (lib + bin) |
| `backend/src/config.rs` | `Config`, `SystemdBackend`, `ConfigError`, constantes (`DEFAULT_BIND_ADDRESS`, `STATIC_DIR_VAR`, `SYSTEMD_VAR`…) y `confirmar_panel` |
| `backend/src/db.rs` | pool SQLite (`preparar`), WAL, `foreign_keys`, migraciones embebidas |
| `backend/src/error.rs` | `AppError` + `IntoResponse` y la traducción de `SystemdError` a HTTP |
| `backend/src/state.rs` | `AppState` (config + cliente systemd) |
| `backend/src/server.rs` | `serve` (con `with_graceful_shutdown`) y `shutdown_signal` |
| `backend/src/api/mod.rs` | router de `/api/v1`, fallbacks de ruta y método, y el servicio del panel |
| `backend/src/api/response.rs` | envoltura `ApiResponse` / `ErrorResponse` |
| `backend/src/api/health.rs` | `GET /api/v1/health` |
| `backend/src/api/units.rs` | `GET /api/v1/units` y `POST /api/v1/units/{name}/{action}` |
| `backend/src/systemd/mod.rs` | contrato del dominio: `UnitName`, `ActiveState`, `UnitFileState`, `UnitOrigin`, `UnitStatus`, `UnitAction`, trait `SystemdClient`, `SystemdError` |
| `backend/src/systemd/dbus.rs` | cliente real (`zbus`) contra `org.freedesktop.systemd1` |
| `backend/src/systemd/fake.rs` | doble en memoria (demo y pruebas de proceso) |
| `backend/src/test_support.rs` | arnés común de pruebas (`#[cfg(test)]`) |
| `backend/migrations/0001_init.sql` | esquema mínimo: `settings` y `action_log` |
| `frontend/src/*` | panel: `App.tsx`, `pages/unidades.tsx`, `components/` (`TablaDeUnidades`, `EstadoDeUnidad`), `hooks/useUnits.ts`, `api/` (cliente + envoltura + errores) |
| `Containerfile` | imagen multi-stage (panel → Rust → runtime no-root) |
| `compose.yaml` | despliegue con los montajes validados |
| `.justfile` | ciclo de calidad y operación (`check`, `check-all`, `ui-*`, `build`, `run`, …) |

## Cómo funciona

### Backend ↔ systemd por D-Bus

El backend habla con **`org.freedesktop.systemd1` por D-Bus** (`zbus`) contra el
bus **de usuario** (`--user`), nunca el de sistema. Una quadlet **es** una unidad
de systemd, así que el estado autoritativo vive en systemd y la API de Podman no
puede accionarla: sólo systemd puede. Por eso no hay `shell-out` a `systemctl`.

**Descubrimiento de quadlets** (`dbus.rs::quadlets`), en tres pasos:

1. `ListUnits` trae todas las unidades del gestor de usuario.
2. `GetUnitFileState` filtra por `UnitFileState=generated` (las quadlets aparecen
   así, no como `enabled`/`disabled`).
3. `SourcePath` aporta la procedencia: una unidad cuenta como quadlet sólo si
   `generated` **y** tiene fichero de origen con extensión de quadlet
   (`.container`, `.volume`, `.network`, `.pod`, `.kube`, …), **excluyendo**
   `.desktop` y sin fichero (unidades transitorias).

**Estado**: `ActiveState`/`SubState` se leen por las propiedades del objeto de la
unidad (o se recortan del `ListUnits`).

**Acciones**: `StartUnit`/`StopUnit`/`RestartUnit` son **asíncronas** —devuelven
un *job*—, así que `action` se suscribe a `JobRemoved` y espera al job concreto
antes de contestar; el estado que se publica es el que quedó, no el que se pidió.

### API REST (envoltura única)

Toda respuesta de `/api/v1/` tiene una de estas dos formas:

- éxito: `{ "data": <payload> }`
- error: `{ "error": { "code": "<snake_case>", "message": "<texto>" } }`

El código HTTP viaja en la cabecera y **no** se duplica en el cuerpo.

| Método y ruta | Cuerpo (`data`) |
|---|---|
| `GET /api/v1/health` | `{ "status": "ok", "bind_address": "…" }` — pública, no toca bus ni base |
| `GET /api/v1/units` | lista de `{ name, active_state, sub_state, unit_file_state }` |
| `POST /api/v1/units/{name}/{start\|stop\|restart}` | `{ name, active_state }` |

La respuesta de una acción es **más estrecha** que la del listado: quien accionó
ya sabe de qué unidad se trata y no necesita el subestado.

### Apagado ordenado

`server::serve` ata el listener y usa `with_graceful_shutdown`; `shutdown_signal`
espera `SIGTERM` (contenedor) o `SIGINT` (desarrollo). `main` devuelve un
`ExitCode` en vez de matar el proceso.

## Variables / configuración

`BIND_ADDRESS`, `DATABASE_URL` y `RUST_LOG` van **sin prefijo**; todo lo demás que
inventa podpot lleva el prefijo `PODPOT_`.

| Variable | Obligatoria | Defecto | Qué hace |
|---|---|---|---|
| `BIND_ADDRESS` | no | `127.0.0.1:8080` | dónde escucha. El defecto es **loopback** a propósito (sin auth en esta entrega) |
| `DATABASE_URL` | **sí** | — | DSN de SQLite (`sqlite://…`) |
| `RUST_LOG` | no | `info` | filtro de registro |
| `PODPOT_SYSTEMD` | no | `dbus` | `dbus` (estado real) o `fake` (doble en memoria, para demo/pruebas sin bus) |
| `PODPOT_STATIC_DIR` | no | — | directorio del panel estático; con reserva a `index.html` para la SPA |

Una variable **definida pero vacía** es error de arranque (no cae al defecto), y
los mensajes de error **nombran** la variable pero nunca su valor.

## Cómo desplegar

```sh
# 1. (host, una vez) deja vivo el bus de usuario sin sesión abierta
loginctl enable-linger $USER

# 2. construye la imagen (si tu uid ≠ 1000, añade
#    --build-arg PODPOT_UID=$(id -u) --build-arg PODPOT_GID=$(id -g))
podman build --format docker -t localhost/podpot:dev .

# 3. levanta con los montajes del spike
podman compose up -d
# o, para un contenedor de prueba suelto:
just run
```

**Crítico** (receta del spike, ninguna pieza es opcional):

- **`--userns=keep-id`** (compose: `userns_mode: keep-id`). Sin él el bus montado
  existe pero **no responde** (`Did not receive a reply`): el uid del contenedor
  debe coincidir con el del host. Es el fallo más fácil de no diagnosticar.
- Montar `/run/user/<uid>` (contiene el bus de sesión y el socket de Podman
  rootless).
- Montar `${HOME}/.config/containers/systemd` y fijar `HOME` al del host (systemd
  reporta los quadlets por su ruta **absoluta**, y el panel necesita poder abrir
  esos ficheros).
- La imagen se construye con ese mismo uid y con TODAS las dependencias
  *baked*: con `keep-id` el proceso corre como usuario no-root, así que instalar
  paquetes en runtime falla.

En el contenedor el bind se abre a `0.0.0.0:8080` y `DATABASE_URL` apunta a
`sqlite:///app/data/podpot.db` (volumen `podpot_data`).

## Decisiones

1. **D-Bus directo, no shell-out a `systemctl`.** Estado **tipado** (sin parsear
   texto), *push* con `PropertiesChanged` para el panel en vivo (SSE, futuro) y
   no cargar el binario `systemctl` en la imagen.
2. **Descubrimiento por D-Bus** (`ListUnits` + `GetUnitFileState` + `SourcePath`)
   en vez de leer ficheros: el gestor ya sabe qué unidades existen. `generated`
   **no basta** por sí solo (systemd llama así a todo lo que sale de un
   generador), de ahí el filtro por procedencia y extensión.
3. **`UnitRejected` → `502` vs `Transport` → `503`.** Una negativa del gestor
   (la unidad no arranca) es un `502` (*bad gateway*: el fallo está en el gestor,
   no en podpot); un bus que no contesta es un `503` (*service unavailable*). El
   cuerpo de error **nunca** filtra el object path de D-Bus ni el texto de la
   biblioteca; sólo el log lo ve.
4. **Nombres de unidad escapados en D-Bus** (`.` → `_2e`, `-` → `_2d`, `_` →
   `_5f`). El nombre crudo jamás se pasa a las rutas
   `/org/freedesktop/systemd1/unit/*`.
5. **Respuesta de acción más estrecha que `UnitStatus`.** Se publica sólo
   `name` + `active_state`: lo que el panel necesita tras pulsar un botón.
6. **`DATABASE_URL` obligatoria y sin defecto.** Un podpot sin base no arranca;
   fingir una base por defecto escondería un error de despliegue.

## Follow-ups (deuda pendiente)

- **Creación de quadlets** (siguiente change): hoy el panel sólo lista y acciona.
- **Autenticación OIDC** opcional (patrón kesto) para exponer más allá de
  loopback; hoy la API es abierta pero atada a loopback por defecto.
- **SSE con `PropertiesChanged`** para que el panel refleje el estado en vivo sin
  *polling*.
- **Autorización más estricta**: hoy la API acciona cualquier unidad que el
  gestor de usuario conozca, no sólo las que podpot reconoce como quadlets.
- **Chunk de Antd > 500 kB**: el build avisa; conviene *code-splitting*
  (`manualChunks`).
- Las **5 pruebas de `backend/tests/systemd_real.rs`** se saltan (`ignored`) si no
  hay bus de usuario: comprueban el D-Bus real y se ejecutan en la máquina de
  desarrollo, no en el CI sin bus.

## Lo que NO trae esta entrega

Sin creación de quadlets, sin autenticación, sin SSE y sin más autorización que
la que da el propio gestor de usuario.
