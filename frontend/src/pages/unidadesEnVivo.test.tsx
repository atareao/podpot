import { act, render, screen } from '@testing-library/react'
import { App as AppDeAntd } from 'antd'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { EventSourceFalso } from '../../tests/eventSourceFalso'
import { unidad } from '../../tests/fixtures'
import { listarUnidades } from '../api/units'
import Unidades from './unidades'

// La capa de red se dobla entera: esta prueba solo ejercita el canal SSE.
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

beforeEach(() => {
  vi.resetAllMocks()
  EventSourceFalso.reset()
})

describe('panel en vivo', () => {
  it('refleja el cambio de estado de una quadlet sin acción del usuario', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', active_state: 'inactive', sub_state: 'dead' }),
    ])

    montar()

    expect(await screen.findByText('Parada')).toBeInTheDocument()

    // Un cambio hecho fuera del panel llega como instantánea por el canal.
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('units', {
        data: [unidad({ name: 'web.service', active_state: 'active', sub_state: 'running' })],
      })
    })

    expect(await screen.findByText('En ejecución')).toBeInTheDocument()
    expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(1)
  })

  it('con un evento ilegible conserva el listado de la carga inicial', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([unidad({ name: 'web.service' })])

    montar()

    expect(await screen.findByText('web.service')).toBeInTheDocument()

    // Un evento que no cumple el contrato del canal no debe tumbar la vista: la
    // lista de la carga inicial se queda como está y no se repite la petición.
    act(() => {
      EventSourceFalso.instancias[0]!.emitir('units', { inesperado: true })
    })

    expect(screen.getByText('web.service')).toBeInTheDocument()
    expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(1)
  })
})
