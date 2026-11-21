import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { unidad } from '../../../tests/fixtures'
import { TablaDeUnidades } from './TablaDeUnidades'

describe('TablaDeUnidades', () => {
  it('expone el nombre accesible del listado en el <table> real', () => {
    render(<TablaDeUnidades unidades={[]} cargando={false} onAccion={vi.fn()} />)

    expect(screen.getByRole('table', { name: 'Quadlets' })).toBeInTheDocument()
  })

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

    await usuario.click(screen.getByRole('button', { name: 'Arrancar web.container' }))

    expect(onAccion).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'web.container' }),
      'start',
    )
  })

  it('identifica cada botón con el nombre de su unidad', () => {
    render(
      <TablaDeUnidades
        unidades={[
          unidad({ name: 'a.container', active_state: 'active' }),
          unidad({ name: 'b.container', active_state: 'active' }),
        ]}
        cargando={false}
        onAccion={vi.fn()}
      />,
    )

    // Dos filas, dos botones «Arrancar», cada uno con su nombre propio.
    expect(screen.getAllByRole('button', { name: /^Arrancar / })).toHaveLength(2)
    expect(screen.getByRole('button', { name: 'Arrancar a.container' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Arrancar b.container' })).toBeInTheDocument()
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

    expect(screen.getByRole('button', { name: 'Arrancar activa.container' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Parar parada.container' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Arrancar parada.container' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Parar activa.container' })).toBeEnabled()
  })

  it('trata los estados intermedios como su destino: «activating» no se arranca, «deactivating» no se para', () => {
    render(
      <TablaDeUnidades
        unidades={[
          unidad({ name: 'subiendo.container', active_state: 'activating' }),
          unidad({ name: 'bajando.container', active_state: 'deactivating' }),
        ]}
        cargando={false}
        onAccion={vi.fn()}
      />,
    )

    expect(screen.getByRole('button', { name: 'Arrancar subiendo.container' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Parar bajando.container' })).toBeDisabled()
    // En transición, reiniciar sigue disponible.
    expect(screen.getByRole('button', { name: 'Reiniciar subiendo.container' })).toBeEnabled()
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

    expect(screen.getByRole('button', { name: 'Arrancar web.container' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Reiniciar web.container' })).toBeDisabled()
  })

  it('muestra el estado vacío cuando no hay quadlets', () => {
    render(<TablaDeUnidades unidades={[]} cargando={false} onAccion={vi.fn()} />)

    expect(screen.getByText('No hay quadlets')).toBeInTheDocument()
  })
})
