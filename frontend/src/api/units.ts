import { peticion } from './client'
import type { AccionDeUnidad, QuadletUnit, ResultadoDeAccion } from '../types/api'

/** `GET /api/v1/units`: todas las quadlets con su estado actual. */
export function listarUnidades(signal?: AbortSignal): Promise<QuadletUnit[]> {
  return peticion<QuadletUnit[]>('/units', { signal })
}

/** `POST /api/v1/units/{name}/{start|stop|restart}`: acciona una quadlet concreta. */
export function accionDeUnidad(
  nombre: string,
  accion: AccionDeUnidad,
): Promise<ResultadoDeAccion> {
  return peticion<ResultadoDeAccion>(`/units/${encodeURIComponent(nombre)}/${accion}`, {
    method: 'POST',
  })
}
