/**
 * Espejo de los tipos de la API de podpot (`backend/`).
 *
 * La envoltura de todas las rutas de `/api/v1/*` es única: en el éxito
 * `{ "data": <payload> }` y en el error `{ "error": { "code", "message" } }`.
 */

/**
 * Estado de actividad (`ActiveState`) de una unidad systemd.
 *
 * Son los valores del enum de systemd: `active`, `reloading`, `inactive`,
 * `failed`, `activating`, `deactivating` y `maintenance`. El panel los trata de
 * forma tolerante: un valor desconocido no rompe la vista (ver
 * `components/TablaDeUnidades`), así que ampliar esta unión es barato.
 */
export type ActiveState =
  | 'active'
  | 'reloading'
  | 'inactive'
  | 'failed'
  | 'activating'
  | 'deactivating'
  | 'maintenance'

/** Una quadlet descubierta, con el estado que reporta `systemd --user`. */
export interface QuadletUnit {
  name: string
  /**
   * Nombre de fichero real de la quadlet (`web.container`): su **identidad**,
   * la clave de `PATCH`/`DELETE`. Lo publica la API (basename de la
   * procedencia que informa systemd), así que el panel **no** lo deduce del
   * nombre de unidad —esa regla inversa no es inyectiva—.
   */
  file_name: string
  /**
   * Tipo derivado de la extensión del fichero. `null` cuando la extensión no
   * corresponde a un tipo creable (systemd puede listar quadlets de tipos que
   * el panel no conoce); el panel no lo necesita para operar.
   */
  kind: TipoDeQuadlet | null
  active_state: ActiveState
  sub_state: string
  unit_file_state: string
  /**
   * Stack al que pertenece la unidad, calculado por el backend. El panel solo
   * lo usa para agrupar: no reconstruye esta relación desde el nombre de la
   * unidad ni desde el `file_name`.
   */
  stack: string
}

/** Tipos de quadlet que el panel puede crear. `.desktop` queda excluida. */
export type TipoDeQuadlet = 'container' | 'volume' | 'network' | 'pod'

/**
 * Cuerpo de `POST /api/v1/quadlets`, espejo **plano** de `QuadletDraft`
 * (`backend/src/quadlet/mod.rs`).
 *
 * El backend usa `#[serde(deny_unknown_fields)]`: **no** existe una clave
 * `fields` ni ninguna anidada, y mandar un campo de más da `400`. El cuerpo del
 * fichero **nunca** viaja: se renderiza en el servidor desde estos campos.
 *
 * Solo `.container` exige un campo (`image`, obligatoria); `volume`, `network` y
 * `pod` se conforman con `name` y `kind`. El resto de campos son opcionales.
 */
export interface PeticionDeCreacion {
  /** Stem del fichero, sin extensión (p. ej. `web` para `web.container`). */
  name: string
  kind: TipoDeQuadlet
  /** `Image=` del contenedor (`.container`, obligatoria). */
  image?: string
  /** `ContainerName=` del contenedor. */
  container_name?: string
  /** `PublishPort=` del contenedor o del pod, en orden. */
  publish_ports?: string[]
  /** `Volume=` del contenedor, en orden. */
  volumes?: string[]
  /** `Environment=` del contenedor, en orden. */
  environments?: string[]
  /** `VolumeName=` del volumen. */
  volume_name?: string
  /** `NetworkName=` de la red. */
  network_name?: string
  /** `Subnet=` de la red. */
  subnet?: string
  /** `PodName=` del pod. */
  pod_name?: string
}

/** Quadlet recién creada: el fichero escrito, el nombre de unidad y su tipo. */
export interface QuadletCreada {
  /** Nombre del fichero escrito (`web.container`). */
  file_name: string
  /** Nombre de unidad generado (`<stem>.service` o `<stem>-<ext>.service`). */
  unit_name: string
  kind: TipoDeQuadlet
}

/**
 * Resultado de `PATCH`/`DELETE /api/v1/quadlets/{name}` sobre una quadlet
 * existente.
 *
 * Espejo de `QuadletOutcome` (`backend/src/api/quadlets.rs`), la misma forma que
 * devuelve la creación: el fichero afectado, su nombre de unidad y su tipo.
 */
export interface QuadletAfectada {
  /** Nombre del fichero afectado (`web.container`). */
  file_name: string
  /** Nombre de unidad afectado (`web.service`). */
  unit_name: string
  /** Tipo de la quadlet afectada. */
  kind: TipoDeQuadlet
}

/**
 * Definición leída de una quadlet (`GET /api/v1/quadlets/{name}`).
 *
 * Es el mismo contrato **plano** de la escritura —lo que el servidor lee del
 * fichero y con lo que lo vuelve a renderizar—: el `GET` no añade ni el nombre de
 * fichero ni el de unidad, que el panel ya conoce por la fila. El alias existe
 * para que el sentido de la lectura se lea en las firmas, no para modelar campos
 * que la API no manda.
 */
export type QuadletDetalle = PeticionDeCreacion

/** Acciones de control admitidas por la API. */
export type AccionDeUnidad = 'start' | 'stop' | 'restart'

// ─── Registros (logs) ────────────────────────────────────────────────────────

/** Origen de una línea de registro: salida estándar o error estándar. */
export type OrigenDeRegistro = 'stdout' | 'stderr'

/**
 * Una línea de registro del contenedor de una quadlet.
 *
 * Es el `data` del evento `log` del canal
 * `GET /api/v1/units/{name}/logs/stream`: cada evento llega envuelto como
 * `{data: {line, stream}}`, así que esto es lo que devuelve `desenvolver`.
 */
export interface LineaDeRegistro {
  /** La línea, sin el salto final. */
  line: string
  /** De qué salida procede. */
  stream: OrigenDeRegistro
}

/** Resultado de una acción de control. */
export interface ResultadoDeAccion {
  name: string
  active_state: ActiveState
}

// ─── Progreso de trabajos (jobs) ─────────────────────────────────────────────

/** Fase del ciclo de vida de un job: se crea (`new`) o termina (`done`). */
export type FaseDeJob = 'new' | 'done'

/**
 * Progreso de un job de systemd sobre una unidad.
 *
 * Es el `data` del evento `job` del canal `GET /api/v1/jobs/stream`: cada evento
 * llega envuelto como `{data: {...}}`, así que esto es lo que devuelve
 * `desenvolver`. En la fase `new` el `result` va a `null`; en la fase `done`
 * lleva el desenlace (`done`, `failed`, …).
 *
 * `unit` es el **nombre de unidad** (`web.service`) —lo que alimenta el canal—,
 * no el de fichero: es la clave con la que el panel asocia el progreso a la fila.
 */
export interface ProgresoDeJob {
  /** Identificador del job en systemd. */
  job: number
  /** Nombre de unidad sobre la que corre el job. */
  unit: string
  /** Tipo de job de systemd (`start`, `stop`, `restart`, …). */
  kind: string
  /** Fase: `new` al crearse, `done` al terminar. */
  phase: FaseDeJob
  /** Desenlace en la fase `done`; `null` en `new`. */
  result: string | null
}

// ─── Autenticación ───────────────────────────────────────────────────────────

/**
 * Modo de autenticación activo. `disabled` (dev en loopback) no exige sesión;
 * `oidc` protege la superficie de negocio y exige entrar.
 */
export type ModoDeAutenticacion = 'disabled' | 'oidc'

/** `GET /api/v1/health` (público) — versión y modo de autenticación. */
export interface Health {
  version: string
  auth_mode: ModoDeAutenticacion
}

/**
 * `GET /api/v1/auth/session` (protegida). En modo `disabled` esa ruta **no
 * existe** —contesta `404`—, así que el panel no la llama.
 */
export interface Sesion {
  sub: string
  email: string
}
