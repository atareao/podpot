import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { QueryClientProvider } from '@tanstack/react-query'
import { App as AppDeAntd } from 'antd'
import App from '../../App'
import { crearClienteDeConsultas } from '../../api/clienteDeConsultas'
import { unidad } from '../../../tests/fixtures'

/**
 * La guarda de sesión en las rutas: se monta `App` entero (guard + pantalla),
 * porque es la integración lo que estas pruebas afirman.
 */

const SALUD_OIDC = { version: '0.1.0', auth_mode: 'oidc' } as const
const SALUD_DISABLED = { version: '0.1.0', auth_mode: 'disabled' } as const
const SESION = { sub: 'ana', email: 'ana@example.org' } as const

function respuesta(status: number, cuerpo: unknown): Response {
  return {
    status,
    ok: status >= 200 && status < 300,
    text: async () => JSON.stringify(cuerpo),
  } as unknown as Response
}

const SIN_SESION = { error: { code: 'unauthorized', message: 'no hay sesión' } }

function instalarFetch(manejador: (url: string) => Response) {
  const fetch = vi.fn(async (entrada: RequestInfo | URL) => manejador(String(entrada)))
  vi.stubGlobal('fetch', fetch)
  return fetch
}

function rutasLlamadas(fetch: ReturnType<typeof instalarFetch>): string[] {
  return fetch.mock.calls.map(([entrada]) => String(entrada))
}

function montar(entrada = '/') {
  const cliente = crearClienteDeConsultas()
  cliente.setDefaultOptions({
    queries: { retry: false, gcTime: 0, staleTime: 0 },
    mutations: { retry: false },
  })
  return render(
    <QueryClientProvider client={cliente}>
      <AppDeAntd>
        <MemoryRouter initialEntries={[entrada]}>
          <App />
        </MemoryRouter>
      </AppDeAntd>
    </QueryClientProvider>,
  )
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('guarda de sesión', () => {
  it('en modo disabled muestra las unidades sin exigir sesión', async () => {
    const fetch = instalarFetch((url) => {
      if (url.endsWith('/api/v1/health')) return respuesta(200, { data: SALUD_DISABLED })
      if (url.endsWith('/api/v1/units')) {
        return respuesta(200, { data: [unidad({ name: 'web.service' })] })
      }
      throw new Error(`ruta inesperada: ${url}`)
    })

    montar('/')

    expect(
      await screen.findByRole('heading', { level: 1, name: 'Quadlets' }),
    ).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: /entrar/i })).not.toBeInTheDocument()
    expect(rutasLlamadas(fetch).some((url) => url.startsWith('/api/v1/auth'))).toBe(false)
  })

  it('en modo oidc sin sesión muestra la portada con el botón, no las unidades', async () => {
    instalarFetch((url) => {
      if (url.endsWith('/api/v1/health')) return respuesta(200, { data: SALUD_OIDC })
      if (url.endsWith('/api/v1/auth/session')) return respuesta(401, SIN_SESION)
      throw new Error(`ruta inesperada: ${url}`)
    })

    montar('/')

    const entrar = await screen.findByRole('link', { name: /entrar/i })
    expect(entrar).toHaveAttribute('href', '/api/v1/auth/login')
    expect(screen.queryByRole('heading', { level: 1, name: 'Quadlets' })).not.toBeInTheDocument()
  })

  it('en modo oidc con sesión muestra las unidades', async () => {
    instalarFetch((url) => {
      if (url.endsWith('/api/v1/health')) return respuesta(200, { data: SALUD_OIDC })
      if (url.endsWith('/api/v1/auth/session')) return respuesta(200, { data: SESION })
      if (url.endsWith('/api/v1/units')) {
        return respuesta(200, { data: [unidad({ name: 'web.service' })] })
      }
      throw new Error(`ruta inesperada: ${url}`)
    })

    montar('/')

    expect(
      await screen.findByRole('heading', { level: 1, name: 'Quadlets' }),
    ).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: /entrar/i })).not.toBeInTheDocument()
  })

  it('si /health falla muestra un error, no la portada ni las unidades', async () => {
    instalarFetch((url) => {
      if (url.endsWith('/api/v1/health')) {
        return respuesta(500, { error: { code: 'internal', message: 'boom' } })
      }
      throw new Error(`ruta inesperada: ${url}`)
    })

    montar('/')

    expect(
      await screen.findByText(/no se pudo contactar con el servidor/i),
    ).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: /entrar/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('heading', { level: 1, name: 'Quadlets' })).not.toBeInTheDocument()
  })

  it('un 401 durante el uso devuelve a la portada pública', async () => {
    let sesionValida = true
    instalarFetch((url) => {
      if (url.endsWith('/api/v1/health')) return respuesta(200, { data: SALUD_OIDC })
      if (url.endsWith('/api/v1/auth/session')) {
        return sesionValida ? respuesta(200, { data: SESION }) : respuesta(401, SIN_SESION)
      }
      if (url.endsWith('/api/v1/units')) {
        // La sesión vale al cargar; el servidor la invalida al contestar el listado.
        sesionValida = false
        return respuesta(401, SIN_SESION)
      }
      throw new Error(`ruta inesperada: ${url}`)
    })

    montar('/')

    // Primero se pinta la pantalla protegida con la sesión todavía válida.
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Quadlets' }),
    ).toBeInTheDocument()

    // El 401 del listado descarta la sesión y devuelve a la portada pública.
    expect(await screen.findByRole('link', { name: /entrar/i })).toHaveAttribute(
      'href',
      '/api/v1/auth/login',
    )
    expect(screen.queryByRole('heading', { level: 1, name: 'Quadlets' })).not.toBeInTheDocument()
  })
})
