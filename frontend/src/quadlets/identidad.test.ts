import { identidadDeUnidad } from './identidad'

describe('identidad de la quadlet a partir del nombre de unidad', () => {
  it('un `.service` sin sufijo de tipo es un contenedor', () => {
    expect(identidadDeUnidad('web.service')).toEqual({
      stem: 'web',
      kind: 'container',
      file_name: 'web.container',
    })
  })

  it('traduce los sufijos de tipo de systemd a su extensión', () => {
    expect(identidadDeUnidad('web-data-volume.service')).toEqual({
      stem: 'web-data',
      kind: 'volume',
      file_name: 'web-data.volume',
    })
    expect(identidadDeUnidad('web-network.service')).toEqual({
      stem: 'web',
      kind: 'network',
      file_name: 'web.network',
    })
    expect(identidadDeUnidad('web-pod.service')).toEqual({
      stem: 'web',
      kind: 'pod',
      file_name: 'web.pod',
    })
  })

  it('admite puntos en el stem', () => {
    expect(identidadDeUnidad('voicebox.prod.service')?.file_name).toBe('voicebox.prod.container')
  })

  it('no inventa identidad para algo que no es una unidad de systemd', () => {
    expect(identidadDeUnidad('no-soy-una-unidad')).toBeNull()
    expect(identidadDeUnidad('.service')).toBeNull()
  })
})
