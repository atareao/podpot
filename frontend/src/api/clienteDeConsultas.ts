import { MutationCache, QueryCache, QueryClient } from '@tanstack/react-query'
import { esErrorDeSesion } from './envelope'
import { claves } from '../hooks/claves'

/**
 * El cliente de consultas del panel.
 *
 * Una relectura silenciosa al volver a la ventana (`refetchOnWindowFocus`) y un
 * `staleTime` corto: el estado de una quadlet cambia por fuera del panel, así
 * que la lista no puede quedarse pegada a una foto vieja. Un reintento único
 * absorbe un parpadeo de red sin dejar la pantalla esperando.
 *
 * Además lleva cableada la reacción global a la pérdida de sesión: un `401` de
 * **cualquier** petición significa «la sesión ya no vale», así que se descarta la
 * caché de la sesión, su relectura da `401` y la guarda devuelve al visitante a
 * la portada pública. La capa de sesión no puede depender de que cada pantalla
 * recuerde tratar un `401`: se reacciona una sola vez y en un solo sitio.
 */

/** ¿La clave es la de la sesión? `queryKey` se compara por su primer elemento. */
function esLaClaveDeSesion(clave: readonly unknown[] | undefined): boolean {
  return clave !== undefined && clave[0] === claves.sesion[0]
}

/**
 * Un `401` descarta la caché de la sesión. El `401` de la **propia** sesión se
 * deja pasar sin invalidarla otra vez: eso sería releerla en bucle.
 */
function anteUnError(
  error: unknown,
  clave: readonly unknown[] | undefined,
  cliente: QueryClient,
): void {
  if (!esErrorDeSesion(error)) return
  if (esLaClaveDeSesion(clave)) return
  void cliente.invalidateQueries({ queryKey: claves.sesion })
}

/**
 * Reintento único para un fallo pasajero de red, **salvo** un `401`: no hay
 * sesión que reintentar. Un `401` es una respuesta definitiva, no un fallo
 * transitorio, así que reintentarlo solo retrasaría la vuelta a la portada y
 * gastaría una petición de más.
 */
function mereceReintento(intentos: number, error: unknown): boolean {
  return !esErrorDeSesion(error) && intentos < 1
}

export function crearClienteDeConsultas(): QueryClient {
  // `cliente` se referencia dentro de los manejadores, que solo corren cuando ya
  // está asignado: no hay uso antes de tiempo.
  const cliente: QueryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: mereceReintento,
        staleTime: 5_000,
        refetchOnWindowFocus: true,
      },
      mutations: {
        retry: false,
      },
    },
    queryCache: new QueryCache({
      onError: (error, query) => {
        anteUnError(error, query.queryKey, cliente)
      },
    }),
    mutationCache: new MutationCache({
      onError: (error, _variables, _contexto, mutation) => {
        anteUnError(error, mutation.options.mutationKey, cliente)
      },
    }),
  })

  return cliente
}
