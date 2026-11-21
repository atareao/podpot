import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { unidad } from '../../../tests/fixtures'
import { TablaDeUnidades } from './TablaDeUnidades'

describe('TablaDeUnidades', () => {
  it('llama a onAccion con la unidad y la acción al pulsar «Arrancar»', async () => {
    const usuario = userEvent.setup()
    const onAccion = vi.fn()

    render(
      <TablaDeUnidades
        unidades={[unidad({ name: 'web.container', active_state: 'inactive', sub_state: 'dead' })]}
        cargando={false}
        onAccion={onAccion}
      />,
    )

    await usuario.click(screen.getByRole('button', { name: 'Arrancar' }))

    expect(onAccion).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'web.container' }),
      'start',
    )
  })

  it('deshabilita «Arrancar» en una unidad activa y «Parar» en una parada', () => {
    render(
      <TablaDeUnidades
        unidades={[
          unidad({ name: 'activa.container', active_state: 'active' }),
          unidad({ name: 'parada.container', active_state: 'inactive' }),
        ]}
        cargando={false}
        onAccion={vi.fn()}
      />,
    )

    expect(screen.getAllByRole('button', { name: 'Arrancar' })[0]).toBeDisabled()
    expect(screen.getAllByRole('button', { name: 'Parar' })[1]).toBeDisabled()
  })

  it('deshabilita todas las acciones de la unidad con una acción en curso', () => {
    render(
      <TablaDeUnidades
        unidades={[unidad({ name: 'web.container', active_state: 'inactive' })]}
        cargando={false}
        enCurso="web.container"
        onAccion={vi.fn()}
      />,
    )

    expect(screen.getByRole('button', { name: 'Arrancar' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Reiniciar' })).toBeDisabled()
  })

  it('muestra el estado vacío cuando no hay quadlets', () => {
    render(<TablaDeUnidades unidades={[]} cargando={false} onAccion={vi.fn()} />)

    expect(screen.getByText('No hay quadlets')).toBeInTheDocument()
  })
})
