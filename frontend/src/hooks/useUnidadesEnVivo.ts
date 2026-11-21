import { useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { RUTA_BASE } from '../api/client'
import { desenvolver } from '../api/envelope'
import { claves } from './claves'
import type { QuadletUnit } from '../types/api'

/** Canal SSE de estado de quadlets, en el mismo origen que el resto de la API. */
export const RUTA_DEL_CANAL = `${RUTA_BASE}/units/stream`

/**
 * Mantiene el listado de quadlets en vivo.
 *
 * Abre una suscripción `EventSource` al canal SSE y, por cada evento `units`
 * —un evento **con nombre**, de ahí `addEventListener` y no `onmessage`—,
 * desenvuelve su `{data}` y reemplaza la caché de `['units']` con la instantánea
 * recibida. El evento es la lista **completa**, así que el reemplazo es
 * idempotente y no hay deltas que reconciliar.
 *
 * La carga inicial la sigue haciendo `GET /api/v1/units` (react-query): si el
 * canal no está disponible o se corta, el panel no se queda vacío y `EventSource`
 * reintenta la conexión por su cuenta. Un evento ilegible se ignora —no tumba la
 * vista— y la suscripción se cierra al desmontar.
 */
export function useUnidadesEnVivo(): void {
  const cliente = useQueryClient()

  useEffect(() => {
    const fuente = new EventSource(RUTA_DEL_CANAL)

    const alRecibirInstantanea = (evento: MessageEvent<string>): void => {
      let instantanea: QuadletUnit[]
      try {
        instantanea = desenvolver<QuadletUnit[]>({
          http: 200,
          cuerpo: JSON.parse(evento.data) as unknown,
        })
      } catch {
        // Un evento que no cumple el contrato se descarta: la lista que ya hay
        // se queda como está hasta el siguiente evento válido.
        return
      }
      cliente.setQueryData(claves.units, instantanea)
    }

    // El canal emite un evento con nombre (`units`), no el `message` por defecto.
    fuente.addEventListener('units', alRecibirInstantanea as EventListener)

    return () => {
      fuente.close()
    }
  }, [cliente])
}
