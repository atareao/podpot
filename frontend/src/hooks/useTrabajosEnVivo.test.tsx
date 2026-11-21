import { act, renderHook } from '@testing-library/react'
import { EventSourceFalso } from '../../tests/eventSourceFalso'
import { progresoDeJob } from '../../tests/fixtures'
import { useTrabajosEnVivo } from './useTrabajosEnVivo'

beforeEach(() => {
  EventSourceFalso.reset()
})

describe('useTrabajosEnVivo', () => {
  it('abre una única suscripción al canal SSE de trabajos', () => {
    const { unmount } = renderHook(() => useTrabajosEnVivo())

    expect(EventSourceFalso.instancias).toHaveLength(1)
    expect(EventSourceFalso.instancias[0]!.url).toBe('/api/v1/jobs/stream')

    unmount()
  })

  it('da de alta la unidad al recibir la fase `new`, con su tipo de acción', () => {
    const { result, unmount } = renderHook(() => useTrabajosEnVivo())

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'new' }),
      })
    })

    expect(result.current).toEqual({ 'web.service': ['start'] })

    unmount()
  })

  it('da de baja la unidad al recibir la fase `done`', () => {
    const { result, unmount } = renderHook(() => useTrabajosEnVivo())

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'new' }),
      })
    })
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'done', result: 'done' }),
      })
    })

    expect(result.current).toEqual({})

    unmount()
  })

  it('mantiene el mapa por unidad con varios jobs a la vez', () => {
    const { result, unmount } = renderHook(() => useTrabajosEnVivo())

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'new' }),
      })
    })
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 2, unit: 'db.service', kind: 'restart', phase: 'new' }),
      })
    })

    expect(result.current).toEqual({ 'web.service': ['start'], 'db.service': ['restart'] })

    // Solo se da de baja la unidad del job que termina.
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'done', result: 'done' }),
      })
    })

    expect(result.current).toEqual({ 'db.service': ['restart'] })

    unmount()
  })

  it('con dos jobs sobre la misma unidad, el indicador sigue tras el `done` del primero y desaparece con el del segundo', () => {
    const { result, unmount } = renderHook(() => useTrabajosEnVivo())

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'new' }),
      })
    })
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 2, unit: 'web.service', kind: 'restart', phase: 'new' }),
      })
    })

    expect(result.current).toEqual({ 'web.service': ['start', 'restart'] })

    // Termina uno de los dos: la unidad sigue «en curso» con el que queda.
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'done', result: 'done' }),
      })
    })
    expect(result.current).toEqual({ 'web.service': ['restart'] })

    // Termina el último: la unidad deja de estar «en curso».
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 2, unit: 'web.service', kind: 'restart', phase: 'done', result: 'done' }),
      })
    })
    expect(result.current).toEqual({})

    unmount()
  })

  it('devuelve la misma referencia cuando el progreso no cambia el estado', () => {
    const { result, unmount } = renderHook(() => useTrabajosEnVivo())

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'new' }),
      })
    })
    const antes = result.current

    // Un `new` repetido (mismo job y tipo) y un `done` de un job desconocido no
    // alteran el estado: la referencia se conserva y no se re-renderiza.
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'new' }),
      })
    })
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', {
        data: progresoDeJob({ job: 99, unit: 'web.service', kind: 'start', phase: 'done', result: 'done' }),
      })
    })

    expect(result.current).toBe(antes)

    unmount()
  })

  it('ignora un evento sin la envoltura de la API', () => {
    const { result, unmount } = renderHook(() => useTrabajosEnVivo())

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('job', { inesperado: true })
    })

    expect(result.current).toEqual({})

    unmount()
  })

  it('ignora el evento por defecto `message` (solo escucha `job`)', () => {
    const { result, unmount } = renderHook(() => useTrabajosEnVivo())

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('message', {
        data: progresoDeJob({ unit: 'web.service' }),
      })
    })

    expect(result.current).toEqual({})

    unmount()
  })

  it('cierra la suscripción al desmontar', () => {
    const { unmount } = renderHook(() => useTrabajosEnVivo())
    const fuente = EventSourceFalso.instancias[0]!

    expect(fuente.cerrado).toBe(false)

    unmount()

    expect(fuente.cerrado).toBe(true)
  })
})
