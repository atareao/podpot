/**
 * Espejo de los tipos de la API de podpot (`backend/`).
 *
 * La envoltura de todas las rutas de `/api/v1/*` es única: en el éxito
 * `{ "data": <payload> }` y en el error `{ "error": { "code", "message" } }`.
 */

/** Estado de actividad de una unidad systemd, tal como lo publica el backend. */
export type ActiveState = 'active' | 'inactive' | 'failed'

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
