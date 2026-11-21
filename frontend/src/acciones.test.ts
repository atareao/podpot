import { etiquetaDeAccionEnCurso, VERBOS } from './acciones'

describe('etiquetaDeAccionEnCurso', () => {
  it('traduce las acciones conocidas a su participio en español', () => {
    expect(etiquetaDeAccionEnCurso('start')).toBe('arrancada')
    expect(etiquetaDeAccionEnCurso('stop')).toBe('parada')
    expect(etiquetaDeAccionEnCurso('restart')).toBe('reiniciada')
  })

  it('cae al tipo crudo de systemd si no lo conoce', () => {
    expect(etiquetaDeAccionEnCurso('reload')).toBe('reload')
  })

  it('los participios salen del mapa compartido `VERBOS`', () => {
    for (const [accion, verbo] of Object.entries(VERBOS)) {
      expect(etiquetaDeAccionEnCurso(accion)).toBe(verbo)
    }
  })
})
