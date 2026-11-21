import { ErrorDeApi, ErrorDeProtocolo, SIN_SESION, esErrorDeSesion } from './envelope'

describe('esErrorDeSesion', () => {
  it('un ErrorDeApi 401 es un error de sesión', () => {
    expect(esErrorDeSesion(new ErrorDeApi(401, 'unauthorized', 'sin sesión'))).toBe(true)
  })

  it('un ErrorDeApi de otro estado no lo es', () => {
    expect(esErrorDeSesion(new ErrorDeApi(500, 'boom', 'fallo'))).toBe(false)
    expect(esErrorDeSesion(new ErrorDeApi(404, 'not_found', 'no existe'))).toBe(false)
  })

  it('un ErrorDeProtocolo no lo es, aunque su status sea 401', () => {
    expect(esErrorDeSesion(new ErrorDeProtocolo(401, 'protocolo'))).toBe(false)
  })

  it('un valor que no es de la API no lo es', () => {
    expect(esErrorDeSesion(null)).toBe(false)
    expect(esErrorDeSesion(new Error('genérico'))).toBe(false)
    expect(esErrorDeSesion(undefined)).toBe(false)
  })

  it('SIN_SESION es 401', () => {
    expect(SIN_SESION).toBe(401)
  })
})
