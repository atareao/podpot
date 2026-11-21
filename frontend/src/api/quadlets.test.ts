import { ErrorDeApi } from './envelope'
import { crearQuadlet } from './quadlets'

function respuesta(cuerpo: unknown, status = 201): Response {
  return {
    status,
    text: async () => JSON.stringify(cuerpo),
  } as unknown as Response
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('api de creación de quadlets', () => {
  it('crearQuadlet hace POST a /api/v1/quadlets con el cuerpo plano (`name`, `kind`, `image`)', async () => {
    const fetch = vi.fn().mockResolvedValue(
      respuesta({
        data: { file_name: 'web.container', unit_name: 'web.service', kind: 'container' },
      }),
    )
    vi.stubGlobal('fetch', fetch)

    const creada = await crearQuadlet({
      name: 'web',
      kind: 'container',
      image: 'docker.io/library/nginx',
    })

    expect(creada.unit_name).toBe('web.service')
    expect(creada.file_name).toBe('web.container')
    const [url, init] = fetch.mock.calls[0] as [string, RequestInit]
    expect(url).toBe('/api/v1/quadlets')
    expect(init.method).toBe('POST')
    expect(JSON.parse(init.body as string)).toEqual({
      name: 'web',
      kind: 'container',
      image: 'docker.io/library/nginx',
    })
  })

  it('un 409 de colisión se traduce en ErrorDeApi con su código y mensaje', async () => {
    const fetch = vi.fn().mockResolvedValue(
      respuesta({ error: { code: 'duplicate', message: 'ya existe una quadlet «web»' } }, 409),
    )
    vi.stubGlobal('fetch', fetch)

    const promesa = crearQuadlet({ name: 'web', kind: 'volume' })

    await expect(promesa).rejects.toBeInstanceOf(ErrorDeApi)
    await expect(promesa).rejects.toMatchObject({
      status: 409,
      code: 'duplicate',
      message: 'ya existe una quadlet «web»',
    })
  })

  it('un 400 de nombre inválido se traduce en ErrorDeApi con su mensaje', async () => {
    const fetch = vi.fn().mockResolvedValue(
      respuesta(
        { error: { code: 'invalid_name', message: 'el nombre solo admite letras y guiones' } },
        400,
      ),
    )
    vi.stubGlobal('fetch', fetch)

    await expect(
      crearQuadlet({ name: '??', kind: 'container', image: 'x' }),
    ).rejects.toMatchObject({
      status: 400,
      code: 'invalid_name',
      message: 'el nombre solo admite letras y guiones',
    })
  })
})
