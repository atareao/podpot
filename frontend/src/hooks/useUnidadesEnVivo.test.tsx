import type { ReactNode } from 'react'
import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { EventSourceFalso } from '../../tests/eventSourceFalso'
import { unidad } from '../../tests/fixtures'
import { claves } from './claves'
import { useUnidadesEnVivo } from './useUnidadesEnVivo'

function montarConCaché() {
  const cliente = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const envoltorio = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={cliente}>{children}</QueryClientProvider>
  )
  const vista = renderHook(() => useUnidadesEnVivo(), { wrapper: envoltorio })
  return { cliente, ...vista }
}

beforeEach(() => {
  EventSourceFalso.reset()
})

describe('useUnidadesEnVivo', () => {
  it('abre una suscripción al canal SSE del mismo origen', () => {
    const { unmount } = montarConCaché()

    expect(EventSourceFalso.instancias).toHaveLength(1)
    expect(EventSourceFalso.instancias[0]!.url).toBe('/api/v1/units/stream')

    unmount()
  })

  it('al recibir un evento `units` reemplaza el listado con la instantánea', () => {
    const { cliente, unmount } = montarConCaché()
    const instantanea = [unidad({ name: 'web.service', active_state: 'active' })]

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('units', { data: instantanea })
    })

    expect(cliente.getQueryData(claves.units)).toEqual(instantanea)

    unmount()
  })

  it('ignora un evento sin la envoltura de la API, sin romper el listado', () => {
    const { cliente, unmount } = montarConCaché()

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('units', { inesperado: true })
    })

    expect(cliente.getQueryData(claves.units)).toBeUndefined()

    unmount()
  })

  it('ante dos eventos encadenados, el listado refleja el último', () => {
    const { cliente, unmount } = montarConCaché()
    const primero = [unidad({ name: 'web.service', active_state: 'inactive' })]
    const segundo = [unidad({ name: 'web.service', active_state: 'failed' })]

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('units', { data: primero })
    })
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('units', { data: segundo })
    })

    expect(cliente.getQueryData(claves.units)).toEqual(segundo)

    unmount()
  })

  it('ignora el evento por defecto `message` (solo escucha `units`)', () => {
    const { cliente, unmount } = montarConCaché()

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('message', {
        data: [unidad({ name: 'web.service' })],
      })
    })

    expect(cliente.getQueryData(claves.units)).toBeUndefined()

    unmount()
  })

  it('cierra la suscripción al desmontar', () => {
    const { unmount } = montarConCaché()
    const fuente = EventSourceFalso.instancias[0]!

    expect(fuente.cerrado).toBe(false)

    unmount()

    expect(fuente.cerrado).toBe(true)
  })
})
