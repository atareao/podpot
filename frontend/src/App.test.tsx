/**
 * Contrato que fijan estos tests para GREEN:
 * - `App` pinta una cabecera fija con la marca «podpot», el usuario de la sesión
 *   (`Sesion.email`/`sub`) y un control «Cerrar sesión».
 * - La cabecera no lleva aparejada una barra lateral (`aside`/`complementary`).
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { App as AppDeAntd } from 'antd'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router-dom'
import App from './App'

const { cerrarSesion } = vi.hoisted(() => ({ cerrarSesion: vi.fn() }))

vi.mock('./hooks/useSesion', () => ({
  useSesion: () => ({
    modo: 'oidc',
    sesion: { sub: 'ana', email: 'ana@example.com' },
    cargando: false,
    sinSesion: false,
    error: null,
    refrescar: vi.fn(),
  }),
  useCerrarSesion: () => ({ mutate: cerrarSesion }),
}))

// La pantalla se dobla: estos tests son de cabecera, no de datos ni de SSE.
vi.mock('./pages/unidades', () => ({ default: () => 'Panel de quadlets' }))

function montar() {
  const cliente = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return render(
    <QueryClientProvider client={cliente}>
      <AppDeAntd>
        <MemoryRouter>
          <App />
        </MemoryRouter>
      </AppDeAntd>
    </QueryClientProvider>,
  )
}

beforeEach(() => {
  cerrarSesion.mockClear()
})

describe('App — cabecera del panel', () => {
  it('muestra la marca, el usuario autenticado y el control de salida', async () => {
    const usuario = userEvent.setup()
    montar()

    const marca = screen.getByText('podpot')
    expect(marca.closest('header')).not.toBeNull()
    expect(screen.getByText(/ana@example\.com/)).toBeInTheDocument()

    const salidas = screen.getAllByRole('button', { name: /cerrar sesión|salir/i })
    await usuario.click(salidas[0]!)
    expect(cerrarSesion).toHaveBeenCalled()
  })

  it('no muestra barra lateral', () => {
    montar()

    expect(screen.queryByRole('complementary')).not.toBeInTheDocument()
    expect(document.querySelector('aside')).toBeNull()
  })
})
