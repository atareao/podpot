import { peticion } from './client'
import type { PeticionDeCreacion, QuadletCreada } from '../types/api'

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
