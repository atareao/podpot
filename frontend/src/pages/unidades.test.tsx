import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { App as AppDeAntd } from 'antd'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { unidad } from '../../tests/fixtures'
import { ErrorDeApi } from '../api/envelope'
import { accionDeUnidad, listarUnidades } from '../api/units'
import Unidades from './unidades'

// La capa de red se dobla entera: las pruebas de pantalla no hablan con la API.
vi.mock('../api/units', () => ({
  listarUnidades: vi.fn(),
  accionDeUnidad: vi.fn(),
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
})

describe('pantalla de quadlets', () => {
  it('lista cada quadlet con su estado', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', active_state: 'active', sub_state: 'running' }),
      unidad({ name: 'db.service', active_state: 'failed', sub_state: 'failed' }),
    ])

    montar()

    expect(await screen.findByText('web.service')).toBeInTheDocument()
    expect(screen.getByText('db.service')).toBeInTheDocument()
    expect(screen.getByText('En ejecución')).toBeInTheDocument()
    expect(screen.getByText('Fallida')).toBeInTheDocument()
  })

  it('muestra un aviso cuando no hay quadlets', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([])

    montar()

    expect(await screen.findByText('No hay quadlets')).toBeInTheDocument()
  })

  it('ante un error de carga lo explica y permite reintentar', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades)
      .mockRejectedValueOnce(new ErrorDeApi(500, 'internal_error', 'no se pudo hablar con systemd'))
      .mockResolvedValueOnce([unidad({ name: 'web.service' })])

    montar()

    expect(await screen.findByText('No se pudieron cargar las quadlets')).toBeInTheDocument()
    expect(screen.getByText('no se pudo hablar con systemd')).toBeInTheDocument()

    await usuario.click(screen.getByRole('button', { name: 'Reintentar' }))

    expect(await screen.findByText('web.service')).toBeInTheDocument()
  })

  it('arranca una quadlet, avisa del resultado y refresca el estado', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades)
      .mockResolvedValueOnce([
        unidad({ name: 'web.service', active_state: 'inactive', sub_state: 'dead' }),
      ])
      .mockResolvedValueOnce([
        unidad({ name: 'web.service', active_state: 'active', sub_state: 'running' }),
      ])
    vi.mocked(accionDeUnidad).mockResolvedValue({ name: 'web.service', active_state: 'active' })

    montar()

    expect(await screen.findByText('Parada')).toBeInTheDocument()

    await usuario.click(screen.getByRole('button', { name: 'Arrancar web.service' }))

    expect(await screen.findByText(/«web.service» arrancada/i)).toBeInTheDocument()
    expect(vi.mocked(accionDeUnidad)).toHaveBeenCalledWith('web.service', 'start')
    // El refresco vuelve a pedir la lista y termina mostrando el estado nuevo.
    expect(await screen.findByText('En ejecución')).toBeInTheDocument()
    await waitFor(() => expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(2))
  })

  it('un error de acción avisa y mantiene el estado real de la unidad', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', active_state: 'inactive', sub_state: 'dead' }),
    ])
    vi.mocked(accionDeUnidad).mockRejectedValue(
      new ErrorDeApi(409, 'unit_conflict', 'la unidad está en transición'),
    )

    montar()

    expect(await screen.findByText('Parada')).toBeInTheDocument()

    await usuario.click(screen.getByRole('button', { name: 'Arrancar web.service' }))

    expect(await screen.findByText('la unidad está en transición')).toBeInTheDocument()
    // Sin refresco: la tabla sigue enseñando el estado real de la unidad.
    expect(screen.getByText('Parada')).toBeInTheDocument()
    expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(1)
  })
})
