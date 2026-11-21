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
  active_state: ActiveState
  sub_state: string
  unit_file_state: string
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

/** Acciones de control admitidas por la API. */
export type AccionDeUnidad = 'start' | 'stop' | 'restart'

/** Resultado de una acción de control. */
export interface ResultadoDeAccion {
  name: string
  active_state: ActiveState
}
