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

/** Acciones de control admitidas por la API. */
export type AccionDeUnidad = 'start' | 'stop' | 'restart'

/** Resultado de una acción de control. */
export interface ResultadoDeAccion {
  name: string
  active_state: ActiveState
}
