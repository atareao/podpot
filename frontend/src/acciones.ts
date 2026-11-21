import type { AccionDeUnidad } from './types/api'

/**
 * Participio de cada acción de control, para los avisos del panel.
 *
 * Es la fuente única de las etiquetas en español de las acciones: la usan tanto
 * la notificación al accionar como el indicador de progreso (ver
 * `etiquetaDeAccionEnCurso`).
 */
export const VERBOS: Record<AccionDeUnidad, string> = {
  start: 'arrancada',
  stop: 'parada',
  restart: 'reiniciada',
}

/**
 * Etiqueta legible del tipo de job en curso.
 *
 * Traduce las acciones conocidas con `VERBOS` y cae al `kind` crudo de systemd
 * (`reload`, `verify`, …) si no está en el mapa: mejor un texto sin traducir que
 * un hueco en blanco.
 */
export function etiquetaDeAccionEnCurso(kind: string): string {
  return VERBOS[kind as AccionDeUnidad] ?? kind
}
