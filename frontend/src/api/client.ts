/**
 * Cliente HTTP del panel.
 *
 * Habla siempre con `/api/v1` en el mismo origen —en desarrollo lo reenvía el
 * proxy de Vite (`vite.config.ts`)— y devuelve el `data` ya desenvuelto: quien
 * llama no ve la envoltura.
 *
 * Los desenlaces que cubre:
 *
 * - **OK:** parsea el JSON y desenvuelve.
 * - **Error de red:** `ErrorDeRed`, con un mensaje que se puede enseñar.
 * - **Error de la API:** `ErrorDeApi` con el `code` y el `message` del backend.
 * - **Cuerpo no-JSON:** `ErrorDeProtocolo`.
 */

import { ErrorDeProtocolo, desenvolver } from './envelope'

/** Prefijo de todas las rutas de la API. */
export const RUTA_BASE = '/api/v1'

/** El servidor no está: red caída, proxy apagado o backend sin arrancar. */
export class ErrorDeRed extends Error {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options)
    this.name = 'ErrorDeRed'
  }
}

export interface OpcionesDePeticion {
  method?: 'GET' | 'POST' | 'PUT' | 'DELETE'
  /** Cuerpo de la petición; se serializa a JSON. */
  cuerpo?: unknown
  signal?: AbortSignal
}

function cabeceras(llevaCuerpo: boolean): HeadersInit {
  const cabeceras: Record<string, string> = { accept: 'application/json' }
  if (llevaCuerpo) cabeceras['content-type'] = 'application/json'
  return cabeceras
}

/** Compone `/api/v1` + la ruta. */
function construirUrl(ruta: string): string {
  return `${RUTA_BASE}${ruta.startsWith('/') ? '' : '/'}${ruta}`
}

/**
 * Lanza una petición a la API y devuelve su `data`.
 *
 * @throws {ErrorDeRed} si no se pudo hablar con el servidor.
 * @throws {ErrorDeApi} si la API contestó un error, con su `code` y `message`.
 * @throws {ErrorDeProtocolo} si la respuesta no trae la envoltura.
 */
export async function peticion<T>(ruta: string, opciones: OpcionesDePeticion = {}): Promise<T> {
  const url = construirUrl(ruta)

  let respuesta: Response
  try {
    respuesta = await fetch(url, {
      method: opciones.method ?? 'GET',
      credentials: 'same-origin',
      headers: cabeceras(opciones.cuerpo !== undefined),
      body: opciones.cuerpo === undefined ? undefined : JSON.stringify(opciones.cuerpo),
      signal: opciones.signal,
    })
  } catch (causa) {
    // Un `AbortError` no es «el servidor no está»: se propaga tal cual para que
    // quien cancela lo distinga.
    if (causa instanceof DOMException && causa.name === 'AbortError') throw causa
    throw new ErrorDeRed('no se pudo contactar con el servidor', { cause: causa })
  }

  const texto = await respuesta.text()
  if (texto.trim() === '') {
    throw new ErrorDeProtocolo(respuesta.status, `la respuesta (HTTP ${respuesta.status}) llegó sin cuerpo`)
  }

  let cuerpo: unknown
  try {
    cuerpo = JSON.parse(texto) as unknown
  } catch {
    throw new ErrorDeProtocolo(respuesta.status, `la respuesta (HTTP ${respuesta.status}) no es JSON`)
  }

  return desenvolver<T>({ http: respuesta.status, cuerpo })
}
