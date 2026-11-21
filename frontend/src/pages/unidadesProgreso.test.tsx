import { act, render, screen, waitFor } from '@testing-library/react'
import { App as AppDeAntd } from 'antd'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { EventSourceFalso } from '../../tests/eventSourceFalso'
import { progresoDeJob, unidad } from '../../tests/fixtures'
import { listarUnidades } from '../api/units'
import Unidades from './unidades'

// La capa de red se dobla entera: esta prueba solo ejercita el canal de trabajos.
vi.mock('../api/units', () => ({
  listarUnidades: vi.fn(),
  accionDeUnidad: vi.fn(),
}))

vi.mock('../api/quadlets', () => ({
  crearQuadlet: vi.fn(),
  editarQuadlet: vi.fn(),
  borrarQuadlet: vi.fn(),
  obtenerQuadlet: vi.fn(),
}))

function montar() {
  const cliente = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
    },
  })
  return render(
    <QueryClientProvider client={cliente}>
      <AppDeAntd>
        <Unidades />
      </AppDeAntd>
    </QueryClientProvider>,
  )
}

/** La suscripción al canal de trabajos entre las abiertas por la pantalla. */
function canalDeTrabajos(): EventSourceFalso {
  return EventSourceFalso.instancias.find((fuente) => fuente.url === '/api/v1/jobs/stream')!
}

beforeEach(() => {
  vi.resetAllMocks()
  EventSourceFalso.reset()
})

describe('progreso de acciones por job', () => {
  it('marca la unidad «en curso» con el tipo de acción al llegar un job `new`', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', active_state: 'inactive', sub_state: 'dead' }),
    ])

    montar()
    expect(await screen.findByText('web.service')).toBeInTheDocument()

    act(() => {
      canalDeTrabajos().emitir('job', {
        data: progresoDeJob({ job: 7, unit: 'web.service', kind: 'start', phase: 'new' }),
      })
    })

    expect(await screen.findByText('arrancada')).toBeInTheDocument()
  })

  it('deja de marcar la unidad al llegar la fase `done`', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([unidad({ name: 'web.service' })])

    montar()
    expect(await screen.findByText('web.service')).toBeInTheDocument()

    act(() => {
      canalDeTrabajos().emitir('job', {
        data: progresoDeJob({ job: 7, unit: 'web.service', kind: 'restart', phase: 'new' }),
      })
    })
    expect(await screen.findByText('reiniciada')).toBeInTheDocument()

    act(() => {
      canalDeTrabajos().emitir('job', {
        data: progresoDeJob({ job: 7, unit: 'web.service', kind: 'restart', phase: 'done', result: 'done' }),
      })
    })

    await waitFor(() => expect(screen.queryByText('reiniciada')).not.toBeInTheDocument())
  })

  it('con dos jobs sobre la misma unidad, mantiene el indicador hasta que terminan los dos', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([unidad({ name: 'web.service' })])

    montar()
    expect(await screen.findByText('web.service')).toBeInTheDocument()

    act(() => {
      canalDeTrabajos().emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'new' }),
      })
    })
    act(() => {
      canalDeTrabajos().emitir('job', {
        data: progresoDeJob({ job: 2, unit: 'web.service', kind: 'restart', phase: 'new' }),
      })
    })
    expect(await screen.findByText('arrancada')).toBeInTheDocument()
    expect(screen.getByText('reiniciada')).toBeInTheDocument()

    // Termina el primero: la unidad sigue «en curso» con el job que queda.
    act(() => {
      canalDeTrabajos().emitir('job', {
        data: progresoDeJob({ job: 1, unit: 'web.service', kind: 'start', phase: 'done', result: 'done' }),
      })
    })
    await waitFor(() => expect(screen.queryByText('arrancada')).not.toBeInTheDocument())
    expect(screen.getByText('reiniciada')).toBeInTheDocument()

    // Termina el segundo: desaparece el indicador.
    act(() => {
      canalDeTrabajos().emitir('job', {
        data: progresoDeJob({ job: 2, unit: 'web.service', kind: 'restart', phase: 'done', result: 'done' }),
      })
    })
    await waitFor(() => expect(screen.queryByLabelText(/Acción en curso/)).not.toBeInTheDocument())
  })

  it('sigue funcionando sin el canal de trabajos (degradación a la carga inicial)', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([unidad({ name: 'web.service' })])

    montar()

    // Sin ningún evento de job, la pantalla pinta el listado y no muestra indicador.
    expect(await screen.findByText('web.service')).toBeInTheDocument()
    expect(screen.queryByLabelText(/Acción en curso/)).not.toBeInTheDocument()
  })

  it('cierra la suscripción al canal de trabajos al desmontar', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([unidad({ name: 'web.service' })])

    const { unmount } = montar()
    expect(await screen.findByText('web.service')).toBeInTheDocument()

    const fuente = canalDeTrabajos()
    expect(fuente.cerrado).toBe(false)

    unmount()

    expect(fuente.cerrado).toBe(true)
  })
})
