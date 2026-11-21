import { act, renderHook, waitFor } from '@testing-library/react'
import { EventSourceFalso } from '../../tests/eventSourceFalso'
import { MAXIMO_DE_LINEAS, useRegistrosDeUnidad } from './useRegistrosDeUnidad'
import type { LineaEnVisor } from './useRegistrosDeUnidad'

beforeEach(() => {
  EventSourceFalso.reset()
})

/** Emite líneas `log` de golpe; el hook las consolida en el siguiente volcado. */
function emitir(
  fuente: EventSourceFalso,
  lineas: Array<{ line: string; stream: 'stdout' | 'stderr' }>,
): void {
  act(() => {
    for (const linea of lineas) fuente.emitir('log', { data: linea })
  })
}

/** Proyecta las líneas a lo que importa al contrato: texto y origen. */
function sinClave(lineas: LineaEnVisor[]): Array<{ line: string; stream: string }> {
  return lineas.map(({ line, stream }) => ({ line, stream }))
}

describe('useRegistrosDeUnidad', () => {
  it('no abre ninguna suscripción mientras el visor está cerrado', () => {
    renderHook(() => useRegistrosDeUnidad(null))

    expect(EventSourceFalso.instancias).toHaveLength(0)
  })

  it('abre el canal de logs de la unidad con la URL correcta', () => {
    const { unmount } = renderHook(() => useRegistrosDeUnidad('web.service'))

    expect(EventSourceFalso.instancias).toHaveLength(1)
    expect(EventSourceFalso.instancias[0]!.url).toBe('/api/v1/units/web.service/logs/stream')

    unmount()
  })

  it('acumula las líneas que llegan por el evento `log`, con su origen', async () => {
    const { result, unmount } = renderHook(() => useRegistrosDeUnidad('web.service'))

    emitir(EventSourceFalso.instancias[0]!, [
      { line: 'una', stream: 'stdout' },
      { line: 'algo falló', stream: 'stderr' },
    ])

    await waitFor(() =>
      expect(sinClave(result.current.lineas)).toEqual([
        { line: 'una', stream: 'stdout' },
        { line: 'algo falló', stream: 'stderr' },
      ]),
    )
    // La clave es estable y creciente, para que la lista no reutilice nodos.
    expect(result.current.lineas[0]!.clave).toBeLessThan(result.current.lineas[1]!.clave)

    unmount()
  })

  it('consolida varias líneas seguidas en un único volcado, no un re-render por línea', async () => {
    const renders = { actual: 0 }
    const { result, unmount } = renderHook(() => {
      renders.actual += 1
      return useRegistrosDeUnidad('web.service')
    })
    const trasMontar = renders.actual

    emitir(
      EventSourceFalso.instancias[0]!,
      Array.from({ length: 50 }, (_, i) => ({ line: `línea ${i}`, stream: 'stdout' as const })),
    )

    await waitFor(() => expect(result.current.lineas).toHaveLength(50))

    // 50 líneas en el mismo lote: un puñado de renders, nunca uno por línea.
    expect(renders.actual - trasMontar).toBeLessThan(5)

    unmount()
  })

  it('conserva solo las últimas MAXIMO_DE_LINEAS cuando se supera el tope', async () => {
    const { result, unmount } = renderHook(() => useRegistrosDeUnidad('web.service'))
    const total = MAXIMO_DE_LINEAS + 5

    emitir(
      EventSourceFalso.instancias[0]!,
      Array.from({ length: total }, (_, i) => ({ line: `línea ${i}`, stream: 'stdout' as const })),
    )

    await waitFor(() => expect(result.current.lineas).toHaveLength(MAXIMO_DE_LINEAS))

    // Se descartan las más antiguas: la primera conservada es la línea 5.
    expect(result.current.lineas[0]!.line).toBe('línea 5')
    expect(result.current.lineas.at(-1)!.line).toBe(`línea ${total - 1}`)

    unmount()
  })

  it('ignora un evento sin la envoltura de la API, sin romper el canal', async () => {
    const { result, unmount } = renderHook(() => useRegistrosDeUnidad('web.service'))
    const fuente = EventSourceFalso.instancias[0]!

    act(() => {
      fuente.emitir('log', { inesperado: true })
    })
    emitir(fuente, [{ line: 'buena', stream: 'stdout' }])

    await waitFor(() => expect(sinClave(result.current.lineas)).toEqual([
      { line: 'buena', stream: 'stdout' },
    ]))

    unmount()
  })

  it('solo escucha el evento `log`, no el `message` por defecto', async () => {
    const { result, unmount } = renderHook(() => useRegistrosDeUnidad('web.service'))

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('message', {
        data: { line: 'por el canal equivocado', stream: 'stdout' },
      })
    })

    // Un evento por el canal equivocado no llega ni al volcado.
    await new Promise((resolver) => setTimeout(resolver, 200))
    expect(result.current.lineas).toEqual([])

    unmount()
  })

  it('expone un aviso cuando el canal falla y lo limpia al reconectar (`open`)', async () => {
    const { result, unmount } = renderHook(() => useRegistrosDeUnidad('web.service'))
    const fuente = EventSourceFalso.instancias[0]!

    act(() => {
      fuente.emitir('error', {})
    })
    expect(result.current.error).not.toBeNull()

    // `EventSource` dispara `open` al (re)establecer la conexión: el aviso sobra.
    act(() => {
      fuente.emitir('open', {})
    })
    await waitFor(() => expect(result.current.error).toBeNull())

    unmount()
  })

  it('cierra la suscripción al desmontar', () => {
    const { unmount } = renderHook(() => useRegistrosDeUnidad('web.service'))
    const fuente = EventSourceFalso.instancias[0]!

    expect(fuente.cerrado).toBe(false)

    unmount()

    expect(fuente.cerrado).toBe(true)
  })

  it('al cerrar el visor (nombre a `null`) da de baja el canal y vacía lo mostrado', async () => {
    const { result, rerender } = renderHook(
      ({ nombre }: { nombre: string | null }) => useRegistrosDeUnidad(nombre),
      { initialProps: { nombre: 'web.service' as string | null } },
    )
    const fuente = EventSourceFalso.instancias[0]!

    emitir(fuente, [{ line: 'una', stream: 'stdout' }])
    await waitFor(() => expect(result.current.lineas).toHaveLength(1))

    rerender({ nombre: null })

    expect(fuente.cerrado).toBe(true)
    expect(result.current.lineas).toEqual([])
  })
})
