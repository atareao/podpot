import { ErrorDeApi } from './envelope'
import { borrarQuadlet, crearQuadlet, editarQuadlet, obtenerQuadlet } from './quadlets'

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

describe('api de edición y borrado de quadlets', () => {
  it('obtenerQuadlet pide GET a /api/v1/quadlets/{fichero} y devuelve su definición', async () => {
    const fetch = vi.fn().mockResolvedValue(
      respuesta({
        data: {
          name: 'web',
          kind: 'container',
          image: 'docker.io/library/nginx',
          publish_ports: ['8080:80'],
        },
      }),
    )
    vi.stubGlobal('fetch', fetch)

    const detalle = await obtenerQuadlet('web.container')

    expect(detalle.kind).toBe('container')
    expect(detalle.image).toBe('docker.io/library/nginx')
    expect(detalle.publish_ports).toEqual(['8080:80'])
    const [url, init] = fetch.mock.calls[0] as [string, RequestInit]
    expect(url).toBe('/api/v1/quadlets/web.container')
    expect(init.method).toBe('GET')
  })

  it('una consulta de inexistente (404) llega desenvuelta como ErrorDeApi', async () => {
    const fetch = vi.fn().mockResolvedValue(
      respuesta(
        { error: { code: 'not_found', message: 'no existe la quadlet «x.container»' } },
        404,
      ),
    )
    vi.stubGlobal('fetch', fetch)

    await expect(obtenerQuadlet('x.container')).rejects.toMatchObject({
      status: 404,
      code: 'not_found',
      message: 'no existe la quadlet «x.container»',
    })
  })

  it('editarQuadlet hace PATCH a /api/v1/quadlets/{fichero} con el cuerpo plano', async () => {
    const fetch = vi.fn().mockResolvedValue(
      respuesta({
        data: { file_name: 'web.container', unit_name: 'web.service', kind: 'container' },
      }),
    )
    vi.stubGlobal('fetch', fetch)

    const afectada = await editarQuadlet('web.container', {
      name: 'web',
      kind: 'container',
      image: 'docker.io/library/nginx',
    })

    expect(afectada.file_name).toBe('web.container')
    const [url, init] = fetch.mock.calls[0] as [string, RequestInit]
    // El identificador es el nombre de fichero, no el de unidad: se codifica en la ruta.
    expect(url).toBe('/api/v1/quadlets/web.container')
    expect(init.method).toBe('PATCH')
    expect(JSON.parse(init.body as string)).toEqual({
      name: 'web',
      kind: 'container',
      image: 'docker.io/library/nginx',
    })
  })

  it('borrarQuadlet hace DELETE a /api/v1/quadlets/{fichero} sin cuerpo ni content-type', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(
        respuesta({
          data: { file_name: 'web.container', unit_name: 'web.service', kind: 'container' },
        }),
      )
    vi.stubGlobal('fetch', fetch)

    const afectada = await borrarQuadlet('web.container')

    expect(afectada.file_name).toBe('web.container')
    const [url, init] = fetch.mock.calls[0] as [string, RequestInit]
    expect(url).toBe('/api/v1/quadlets/web.container')
    expect(init.method).toBe('DELETE')
    expect(init.body).toBeUndefined()
    expect((init.headers as Record<string, string>)['content-type']).toBeUndefined()
  })

  it('un fallo de edición (404) llega desenvuelto como ErrorDeApi', async () => {
    const fetch = vi.fn().mockResolvedValue(
      respuesta(
        { error: { code: 'not_found', message: 'no existe la quadlet «web.container»' } },
        404,
      ),
    )
    vi.stubGlobal('fetch', fetch)

    await expect(
      editarQuadlet('web.container', { name: 'web', kind: 'container', image: 'x' }),
    ).rejects.toMatchObject({
      status: 404,
      code: 'not_found',
      message: 'no existe la quadlet «web.container»',
    })
  })

  it('un fallo de borrado (404) llega desenvuelto como ErrorDeApi', async () => {
    const fetch = vi.fn().mockResolvedValue(
      respuesta({ error: { code: 'not_found', message: 'no existe la quadlet «x.volume»' } }, 404),
    )
    vi.stubGlobal('fetch', fetch)

    await expect(borrarQuadlet('x.volume')).rejects.toMatchObject({
      status: 404,
      code: 'not_found',
      message: 'no existe la quadlet «x.volume»',
    })
  })
})
