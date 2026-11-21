/**
 * Sesión del panel.
 *
 * El modo lo decide el backend: `GET /api/v1/health` publica `auth_mode`.
 *
 * - **`disabled`** (desarrollo, sin proveedor de identidad): no hay capa de
 *   sesión. Las rutas `/api/v1/auth/*` **no existen** —contestan `404`—, así que
 *   el panel no debe llamarlas. Estas funciones no se invocan en ese modo.
 * - **`oidc`**: `GET /api/v1/auth/session` devuelve `{sub, email}` o `401` si no
 *   hay sesión.
 *
 * El login y el callback son **navegaciones** (`302`), no peticiones de datos:
 * se enlazan, no se hacen con `fetch`. La salida sí es una petición: `POST`.
 */

import { peticion } from './client'
import type { Health, Sesion } from '../types/api'

/** `GET /api/v1/health` — versión y modo de autenticación. */
export function obtenerSalud(): Promise<Health> {
  return peticion<Health>('/health')
}

/** `GET /api/v1/auth/session` — solo tiene sentido en modo `oidc`. */
export function obtenerSesion(): Promise<Sesion> {
  return peticion<Sesion>('/auth/session')
}

/** `POST /api/v1/auth/logout` — la salida; la siguiente lectura dará `401`. */
export function cerrarLaSesion(): Promise<null> {
  return peticion<null>('/auth/logout', { method: 'POST' })
}
