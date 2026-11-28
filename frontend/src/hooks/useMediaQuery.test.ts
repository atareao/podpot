/**
 * Contrato que fijan estos tests para GREEN:
 * - `useMediaQuery(consulta)` devuelve si la consulta CSS coincide y reacciona a
 *   eventos `change` de `matchMedia`, dándose de baja al desmontar.
 * - El panel usa `(max-width: 767px)` para considerar vista móvil.
 */

import { act, renderHook } from '@testing-library/react'
import { useMediaQuery } from './useMediaQuery'

const CONSULTA_MOVIL = '(max-width: 767px)'
const innerWidthOriginal = window.innerWidth
const matchMediaOriginal = window.matchMedia

function fijarAncho(px: number): void {
  Object.defineProperty(window, 'innerWidth', {
    configurable: true,
    writable: true,
    value: px,
  })
}

afterEach(() => {
  fijarAncho(innerWidthOriginal)
  window.matchMedia = matchMediaOriginal
  vi.restoreAllMocks()
})

describe('useMediaQuery', () => {
  it('es true con un viewport inferior a 768 px', () => {
    fijarAncho(375)

    const { result } = renderHook(() => useMediaQuery(CONSULTA_MOVIL))

    expect(result.current).toBe(true)
  })

  it('es false con un viewport igual o superior a 768 px', () => {
    fijarAncho(768)

    const { result } = renderHook(() => useMediaQuery(CONSULTA_MOVIL))

    expect(result.current).toBe(false)
  })

  it('sigue los cambios de la consulta y se da de baja al desmontar', () => {
    const oyentes = new Set<(evento: MediaQueryListEvent) => void>()
    const alta = vi.fn((_tipo: string, oyente: (evento: MediaQueryListEvent) => void) => {
      oyentes.add(oyente)
    })
    const baja = vi.fn((_tipo: string, oyente: (evento: MediaQueryListEvent) => void) => {
      oyentes.delete(oyente)
    })
    let coincide = false
    window.matchMedia = vi.fn().mockImplementation((consulta: string) => ({
      get matches() {
        return coincide
      },
      media: consulta,
      addEventListener: alta,
      removeEventListener: baja,
      addListener: alta,
      removeListener: baja,
    })) as unknown as typeof window.matchMedia

    const vista = renderHook(() => useMediaQuery(CONSULTA_MOVIL))
    expect(vista.result.current).toBe(false)

    coincide = true
    act(() => {
      for (const oyente of oyentes) oyente({ matches: true } as MediaQueryListEvent)
    })
    expect(vista.result.current).toBe(true)

    vista.unmount()
    expect(baja).toHaveBeenCalled()
  })
})
