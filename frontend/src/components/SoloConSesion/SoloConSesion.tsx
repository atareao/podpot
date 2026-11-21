import type { ReactNode } from 'react'
import { Alert, Spin } from 'antd'
import { useSesion } from '../../hooks/useSesion'
import Portada from '../../pages/portada'

export interface SoloConSesionProps {
  children: ReactNode
}

/**
 * La guarda de sesión: envuelve las rutas que exigen sesión.
 *
 * - **`disabled`** (desarrollo): no hay capa de sesión; deja pasar y **no** llama
 *   a ninguna ruta `/auth/*`.
 * - **`oidc` con sesión:** deja pasar.
 * - **`oidc` sin sesión:** muestra la portada pública, donde se ofrece entrar. Un
 *   `401` de cualquier petición descarta la caché de la sesión
 *   (`clienteDeConsultas`), su relectura vuelve a dar `401` y esta guarda
 *   devuelve al visitante a la portada.
 * - **`/health` no contesta:** sin saber el modo no se puede decidir, así que se
 *   muestra un error explícito —no la portada, cuyo botón sería inútil—.
 *
 * Mientras `/health` y la sesión están en el aire se muestra la carga: enseñar el
 * panel «por si acaso» sería enseñarlo sin sesión.
 */
export function SoloConSesion({ children }: SoloConSesionProps) {
  const { modo, sesion, cargando, error } = useSesion()

  if (cargando) {
    return (
      <div
        style={{ display: 'flex', alignItems: 'center', gap: 8, padding: 24 }}
      >
        <Spin size="small" />
        <span role="status">Comprobando la sesión…</span>
      </div>
    )
  }

  // Un fallo de `/health` deja el modo sin determinar: no se puede decidir si
  // exigir sesión. Se comunica el error en vez de enseñar la portada (cuyo botón
  // de login no serviría si el servidor no responde).
  if (error !== null) {
    return (
      <Alert
        type="error"
        showIcon
        message="No se pudo contactar con el servidor"
        description="Revisa tu conexión y vuelve a intentarlo."
      />
    )
  }

  if (modo === 'disabled') {
    return <>{children}</>
  }

  if (modo === 'oidc' && sesion !== null) {
    return <>{children}</>
  }

  return <Portada />
}
