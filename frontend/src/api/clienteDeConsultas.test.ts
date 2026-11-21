import type { QueryClient } from '@tanstack/react-query'
import { ErrorDeApi } from './envelope'
import { crearClienteDeConsultas } from './clienteDeConsultas'
import { claves } from '../hooks/claves'

/**
 * El cliente real, pero sin esperas entre reintentos: conserva la política de
 * reintento de producción (`retry`) y solo acorta el retardo, para que una
 * prueba de reintentos no tarde ~1 s.
 */
function clienteRapido(): QueryClient {
  const cliente = crearClienteDeConsultas()
  cliente.setDefaultOptions({
    queries: {
      retry: cliente.getDefaultOptions().queries?.retry,
      retryDelay: 0,
      gcTime: 0,
    },
  })
  return cliente
}

/** Lanza una consulta que falla siempre, contando cuántas veces se intentó. */
async function consultaQueFalla(cliente: QueryClient, clave: readonly unknown[], error: Error) {
  let intentos = 0
  await expect(
    cliente.fetchQuery({
      queryKey: clave,
      queryFn: async () => {
        intentos += 1
        throw error
      },
    }),
  ).rejects.toBe(error)
  return intentos
}

describe('clienteDeConsultas', () => {
  it('un 401 de una consulta cualquiera invalida la sesión', async () => {
    const cliente = clienteRapido()
    const espia = vi.spyOn(cliente, 'invalidateQueries')

    await consultaQueFalla(cliente, claves.units, new ErrorDeApi(401, 'unauthorized', 'sin sesión'))

    expect(espia).toHaveBeenCalledWith({ queryKey: claves.sesion })
  })

  it('un 401 de la propia sesión no la invalida otra vez (sin bucle)', async () => {
    const cliente = clienteRapido()
    const espia = vi.spyOn(cliente, 'invalidateQueries')

    await consultaQueFalla(cliente, claves.sesion, new ErrorDeApi(401, 'unauthorized', 'sin sesión'))

    expect(espia).not.toHaveBeenCalled()
  })

  it('un error que no es de sesión no dispara la reacción', async () => {
    const cliente = clienteRapido()
    const espia = vi.spyOn(cliente, 'invalidateQueries')

    await consultaQueFalla(cliente, claves.units, new ErrorDeApi(500, 'boom', 'fallo'))

    expect(espia).not.toHaveBeenCalled()
  })

  it('un 401 no se reintenta: una sola petición antes de volver a la portada', async () => {
    const cliente = clienteRapido()

    const intentos = await consultaQueFalla(
      cliente,
      claves.units,
      new ErrorDeApi(401, 'unauthorized', 'sin sesión'),
    )

    expect(intentos).toBe(1)
  })

  it('un error que no es de sesión sí se reintenta una vez', async () => {
    const cliente = clienteRapido()

    const intentos = await consultaQueFalla(cliente, claves.units, new ErrorDeApi(500, 'boom', 'fallo'))

    expect(intentos).toBe(2)
  })
})
