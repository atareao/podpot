import { ErrorDeApi } from './envelope'
import { accionDeUnidad, listarUnidades } from './units'

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

describe('api de unidades', () => {
  it('listarUnidades pide /api/v1/units y devuelve el array de `data`', async () => {
    const fetch = vi.fn().mockResolvedValue(
      respuesta({
        data: [
          {
            name: 'web.service',
            active_state: 'active',
            sub_state: 'running',
            unit_file_state: 'generated',
          },
        ],
      }),
    )
    vi.stubGlobal('fetch', fetch)

    const unidades = await listarUnidades()

    expect(unidades).toHaveLength(1)
    expect(String(fetch.mock.calls[0]?.[0])).toBe('/api/v1/units')
  })

  it('accionDeUnidad hace POST a la ruta de la acción', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(respuesta({ data: { name: 'web.service', active_state: 'active' } }))
    vi.stubGlobal('fetch', fetch)

    const resultado = await accionDeUnidad('web.service', 'start')

    expect(resultado.active_state).toBe('active')
    const [url, init] = fetch.mock.calls[0] as [string, RequestInit]
    expect(url).toBe('/api/v1/units/web.service/start')
    expect(init.method).toBe('POST')
  })

  it('un error de la API se traduce en ErrorDeApi con su código y mensaje', async () => {
    const fetch = vi.fn().mockResolvedValue(
      respuesta({ error: { code: 'unit_not_found', message: 'no existe' } }, 404),
    )
    vi.stubGlobal('fetch', fetch)

    const promesa = accionDeUnidad('nope', 'start')

    await expect(promesa).rejects.toBeInstanceOf(ErrorDeApi)
    await expect(promesa).rejects.toMatchObject({
      status: 404,
      code: 'unit_not_found',
      message: 'no existe',
    })
  })
})
