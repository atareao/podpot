import { QueryClient } from '@tanstack/react-query'

/**
 * El cliente de consultas del panel.
 *
 * Una relectura silenciosa al volver a la ventana (`refetchOnWindowFocus`) y un
 * `staleTime` corto: el estado de una quadlet cambia por fuera del panel, así
 * que la lista no puede quedarse pegada a una foto vieja. Un `retry` único
 * absorbe un parpadeo de red sin dejar la pantalla esperando.
 */
export function crearClienteDeConsultas(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: 1,
        staleTime: 5_000,
        refetchOnWindowFocus: true,
      },
      mutations: {
        retry: false,
      },
    },
  })
}
