import { peticion } from './client'
import type {
  PeticionDeCreacion,
  QuadletAfectada,
  QuadletCreada,
  QuadletDetalle,
} from '../types/api'

/**
 * `POST /api/v1/quadlets`: crea una quadlet a partir de su nombre, tipo y campos.
 *
 * El cuerpo va **plano** (sin `fields`): el backend usa `deny_unknown_fields`, así
 * que un campo anidado o de más daría `400`. Nunca se manda contenido de fichero:
 * el backend renderiza el fichero desde la plantilla del tipo. Un fallo llega ya
 * desenvuelto como `ErrorDeApi` (con el 409 de colisión incluido).
 */
export function crearQuadlet(datos: PeticionDeCreacion): Promise<QuadletCreada> {
  return peticion<QuadletCreada>('/quadlets', { method: 'POST', cuerpo: datos })
}

/**
 * `GET /api/v1/quadlets/{name}`: lee la definición de una quadlet existente.
 *
 * `nombre` es el nombre de fichero completo (`web.container`) —la identidad—,
 * no el de unidad. La respuesta trae el tipo y los campos con los que se
 * precarga el formulario; un fichero que no existe da `404` (`not_found`).
 */
export function obtenerQuadlet(nombre: string, signal?: AbortSignal): Promise<QuadletDetalle> {
  return peticion<QuadletDetalle>(`/quadlets/${encodeURIComponent(nombre)}`, { signal })
}

/**
 * `PATCH /api/v1/quadlets/{name}`: reescribe una quadlet existente.
 *
 * `nombre` es el **nombre de fichero completo** (`web.container`) —la identidad
 * de la quadlet—, no el nombre de unidad (`web.service`). El cuerpo es el mismo
 * contrato tipado de la creación, plano y sin contenido de fichero.
 */
export function editarQuadlet(nombre: string, datos: PeticionDeCreacion): Promise<QuadletAfectada> {
  return peticion<QuadletAfectada>(`/quadlets/${encodeURIComponent(nombre)}`, {
    method: 'PATCH',
    cuerpo: datos,
  })
}

/**
 * `DELETE /api/v1/quadlets/{name}`: borra una quadlet existente.
 *
 * Como en la edición, `nombre` es el nombre de fichero completo. La petición no
 * lleva cuerpo.
 */
export function borrarQuadlet(nombre: string): Promise<QuadletAfectada> {
  return peticion<QuadletAfectada>(`/quadlets/${encodeURIComponent(nombre)}`, {
    method: 'DELETE',
  })
}
