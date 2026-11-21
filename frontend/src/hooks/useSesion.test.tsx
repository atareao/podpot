import { render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { useSesion } from './useSesion'

function respuestaFalsa(status: number, data: unknown): Response {
  const cuerpo = JSON.stringify(
    status >= 400 ? { error: { code: 'unauthorized', message: 'no hay sesión' } } : { data },
  )
  return { status, ok: status < 400, text: async () => cuerpo } as unknown as Response
}

const fetchFalso = vi.fn<typeof fetch>()

/** Rutas llamadas, para afirmar que /auth/* no se toca en modo disabled. */
function rutasLlamadas(): string[] {
  return fetchFalso.mock.calls.map(([entrada]) => String(entrada))
}

function Sonda() {
  const { modo, sesion, cargando, sinSesion } = useSesion()
  return (
    <div>
      <span data-testid="modo">{modo ?? 'cargando'}</span>
      <span data-testid="cargando">{String(cargando)}</span>
      <span data-testid="sin-sesion">{String(sinSesion)}</span>
      <span data-testid="sub">{sesion?.sub ?? '—'}</span>
    </div>
  )
}

function montar(nodo: ReactNode, { retry = false }: { retry?: number | false } = {}) {
  const cliente = new QueryClient({
    defaultOptions: {
      queries: { retry, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
    },
  })
  return render(<QueryClientProvider client={cliente}>{nodo}</QueryClientProvider>)
}

/** Enrutador de mentira: /health siempre responde oidc; el resto, según el caso. */
function enrutar(manejador: (url: string) => Response | undefined) {
  fetchFalso.mockImplementation(async (entrada) => {
    const url = String(entrada)
    if (url.endsWith('/health')) {
      return respuestaFalsa(200, { version: '0.1.0', auth_mode: 'oidc' })
    }
    const respuesta = manejador(url)
    if (respuesta === undefined) throw new Error(`ruta inesperada: ${url}`)
    return respuesta
  })
}

beforeEach(() => {
  fetchFalso.mockReset()
  vi.stubGlobal('fetch', fetchFalso)
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('useSesion', () => {
  it('lee el modo de autenticación de /health', async () => {
    fetchFalso.mockResolvedValue(
      respuestaFalsa(200, { version: '0.1.0', auth_mode: 'disabled' }),
    )

    montar(<Sonda />)

    await waitFor(() => expect(screen.getByTestId('modo')).toHaveTextContent('disabled'))
  })

  it('en modo disabled no llama a ninguna ruta de /auth', async () => {
    fetchFalso.mockResolvedValue(
      respuestaFalsa(200, { version: '0.1.0', auth_mode: 'disabled' }),
    )

    montar(<Sonda />)

    await waitFor(() => expect(screen.getByTestId('cargando')).toHaveTextContent('false'))
    expect(rutasLlamadas()).toEqual(['/api/v1/health'])
    expect(screen.getByTestId('sub')).toHaveTextContent('—')
    expect(screen.getByTestId('sin-sesion')).toHaveTextContent('false')
  })

  it('en modo oidc pide la sesión y la publica', async () => {
    enrutar(() => respuestaFalsa(200, { sub: 'ana', email: 'ana@example.org' }))

    montar(<Sonda />)

    await waitFor(() => expect(screen.getByTestId('sub')).toHaveTextContent('ana'))
    expect(rutasLlamadas()).toContain('/api/v1/auth/session')
    expect(screen.getByTestId('sin-sesion')).toHaveTextContent('false')
  })

  it('un 401 es «sin sesión» y no se reintenta en bucle', async () => {
    enrutar(() => respuestaFalsa(401, null))

    // El cliente se construye **con** reintentos a propósito: si el hook no los
    // desactivase para la sesión, habría una segunda llamada en cuanto pasase el
    // primer retardo.
    montar(<Sonda />, { retry: 2 })

    await waitFor(() => expect(screen.getByTestId('sin-sesion')).toHaveTextContent('true'))
    await new Promise((listo) => setTimeout(listo, 1200))

    const llamadasASesion = rutasLlamadas().filter((url) => url.endsWith('/auth/session'))
    expect(llamadasASesion).toHaveLength(1)
  })
})
