/**
 * Contrato que fijan estos tests para GREEN:
 * - `useTema()` devuelve `{ tema, establecerTema, alternarTema }`, con
 *   `tema: 'light' | 'dark'`, persistido en `localStorage['podpot-tema']`.
 * - Sin elección previa respeta `(prefers-color-scheme: dark)`.
 */

import { act, renderHook } from '@testing-library/react'
import { useTema } from './useTema'

const CLAVE = 'podpot-tema'

const matchMediaOriginal = window.matchMedia

afterEach(() => {
  localStorage.clear()
  window.matchMedia = matchMediaOriginal
})

describe('useTema', () => {
  it('persiste el tema elegido en localStorage', () => {
    localStorage.setItem(CLAVE, 'dark')
    const { result } = renderHook(() => useTema())
    expect(result.current.tema).toBe('dark')

    act(() => {
      result.current.establecerTema('light')
    })
    expect(result.current.tema).toBe('light')
    expect(localStorage.getItem(CLAVE)).toBe('light')

    act(() => {
      result.current.alternarTema()
    })
    expect(result.current.tema).toBe('dark')
    expect(localStorage.getItem(CLAVE)).toBe('dark')
  })

  it('recupera el tema guardado al volver a montar el hook', () => {
    const primero = renderHook(() => useTema())
    act(() => {
      primero.result.current.establecerTema('dark')
    })
    primero.unmount()

    const segundo = renderHook(() => useTema())
    expect(segundo.result.current.tema).toBe('dark')
  })

  it('respeta prefers-color-scheme cuando no hay elección previa', () => {
    window.matchMedia = vi.fn().mockImplementation((consulta: string) => ({
      matches: consulta === '(prefers-color-scheme: dark)',
      media: consulta,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
    })) as typeof window.matchMedia

    const { result } = renderHook(() => useTema())

    expect(result.current.tema).toBe('dark')
    expect(localStorage.getItem(CLAVE)).toBeNull()
  })

  it('sigue los cambios del sistema sin elegir tema y se da de baja al desmontar', () => {
    const oyentes = new Set<(evento: MediaQueryListEvent) => void>()
    const baja = vi.fn((_tipo: string, oyente: (evento: MediaQueryListEvent) => void) => {
      oyentes.delete(oyente)
    })
    let oscuro = false
    window.matchMedia = vi.fn().mockImplementation((consulta: string) => ({
      get matches() {
        return oscuro
      },
      media: consulta,
      addEventListener: (_tipo: string, oyente: (evento: MediaQueryListEvent) => void) => {
        oyentes.add(oyente)
      },
      removeEventListener: baja,
      addListener: (_tipo: string, oyente: (evento: MediaQueryListEvent) => void) => {
        oyentes.add(oyente)
      },
      removeListener: baja,
    })) as unknown as typeof window.matchMedia

    const vista = renderHook(() => useTema())
    expect(vista.result.current.tema).toBe('light')

    const notificar = (matches: boolean) => {
      act(() => {
        for (const oyente of oyentes) oyente({ matches } as MediaQueryListEvent)
      })
    }

    oscuro = true
    notificar(true)
    expect(vista.result.current.tema).toBe('dark')
    expect(localStorage.getItem(CLAVE)).toBeNull()

    // Una elección explícita deja de seguir al sistema, incluso dentro del mismo mount.
    act(() => {
      vista.result.current.establecerTema('light')
    })
    notificar(true)
    expect(vista.result.current.tema).toBe('light')

    vista.unmount()
    expect(baja).toHaveBeenCalled()
    expect(oyentes.size).toBe(0)
  })
})
