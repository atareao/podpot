import { cerrarLaSesion, obtenerSalud, obtenerSesion } from './sesion'

function respuesta(cuerpo: unknown, status = 200): Response {
  return {
    status,
    ok: status >= 200 && status < 300,
    text: async () => JSON.stringify(cuerpo),
  } as unknown as Response
}

beforeEach(() => {
  vi.unstubAllGlobals()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('api de sesión', () => {
  it('obtenerSalud pide /api/v1/health y devuelve el modo de autenticación', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(respuesta({ data: { version: '0.1.0', auth_mode: 'oidc' } }))
    vi.stubGlobal('fetch', fetch)

    const salud = await obtenerSalud()

    expect(salud.auth_mode).toBe('oidc')
    expect(String(fetch.mock.calls[0]?.[0])).toBe('/api/v1/health')
  })

  it('obtenerSesion pide /api/v1/auth/session y devuelve la identidad', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(respuesta({ data: { sub: 'ana', email: 'ana@example.org' } }))
    vi.stubGlobal('fetch', fetch)

    const sesion = await obtenerSesion()

    expect(sesion.sub).toBe('ana')
    expect(sesion.email).toBe('ana@example.org')
    expect(String(fetch.mock.calls[0]?.[0])).toBe('/api/v1/auth/session')
  })

  it('cerrarLaSesion hace POST a /api/v1/auth/logout', async () => {
    const fetch = vi.fn().mockResolvedValue(respuesta({ data: null }))
    vi.stubGlobal('fetch', fetch)

    await cerrarLaSesion()

    const [url, init] = fetch.mock.calls[0] as [string, RequestInit]
    expect(url).toBe('/api/v1/auth/logout')
    expect(init.method).toBe('POST')
  })
})
