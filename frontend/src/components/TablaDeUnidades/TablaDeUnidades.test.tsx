import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { unidad } from '../../../tests/fixtures'
import { TablaDeUnidades } from './TablaDeUnidades'

/** Props mínimas de una tabla con una sola fila, para no repetirlas en cada caso. */
function tabla(unidades: Parameters<typeof TablaDeUnidades>[0]['unidades']) {
  const onAccion = vi.fn()
  const onEditar = vi.fn()
  const onVerRegistros = vi.fn()
  const onBorrar = vi.fn()
  render(
    <TablaDeUnidades
      unidades={unidades}
      cargando={false}
      onAccion={onAccion}
      onEditar={onEditar}
      onVerRegistros={onVerRegistros}
      onBorrar={onBorrar}
    />,
  )
  return { onAccion, onEditar, onVerRegistros, onBorrar }
}

describe('TablaDeUnidades', () => {
  it('expone el nombre accesible del listado en el <table> real', () => {
    render(
      <TablaDeUnidades
        unidades={[]}
        cargando={false}
        onAccion={vi.fn()}
        onEditar={vi.fn()}
        onVerRegistros={vi.fn()}
        onBorrar={vi.fn()}
      />,
    )

    expect(screen.getByRole('table', { name: 'Quadlets' })).toBeInTheDocument()
  })

  it('llama a onAccion con la unidad y la acción al pulsar «Arrancar»', async () => {
    const usuario = userEvent.setup()
    const { onAccion } = tabla([
      unidad({ name: 'web.service', active_state: 'inactive', sub_state: 'dead' }),
    ])

    await usuario.click(screen.getByRole('button', { name: 'Arrancar web.service' }))

    expect(onAccion).toHaveBeenCalledWith(expect.objectContaining({ name: 'web.service' }), 'start')
  })

  it('identifica cada botón con el nombre de su unidad', () => {
    tabla([
      unidad({ name: 'a.service', active_state: 'active' }),
      unidad({ name: 'b.service', active_state: 'active' }),
    ])

    // Dos filas, dos botones «Arrancar», cada uno con su nombre propio.
    expect(screen.getAllByRole('button', { name: /^Arrancar / })).toHaveLength(2)
    expect(screen.getByRole('button', { name: 'Arrancar a.service' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Arrancar b.service' })).toBeInTheDocument()
  })

  it('deshabilita «Arrancar» en una unidad activa y «Parar» en una parada', () => {
    tabla([
      unidad({ name: 'activa.service', active_state: 'active' }),
      unidad({ name: 'parada.service', active_state: 'inactive' }),
    ])

    expect(screen.getByRole('button', { name: 'Arrancar activa.service' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Parar parada.service' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Arrancar parada.service' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Parar activa.service' })).toBeEnabled()
  })

  it('trata los estados intermedios como su destino: «activating» no se arranca, «deactivating» no se para', () => {
    tabla([
      unidad({ name: 'subiendo.service', active_state: 'activating' }),
      unidad({ name: 'bajando.service', active_state: 'deactivating' }),
    ])

    expect(screen.getByRole('button', { name: 'Arrancar subiendo.service' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Parar bajando.service' })).toBeDisabled()
    // En transición, reiniciar sigue disponible.
    expect(screen.getByRole('button', { name: 'Reiniciar subiendo.service' })).toBeEnabled()
  })

  it('deshabilita todas las acciones de la unidad con una acción en curso', () => {
    render(
      <TablaDeUnidades
        unidades={[unidad({ name: 'web.service', active_state: 'inactive' })]}
        cargando={false}
        enCurso="web.service"
        onAccion={vi.fn()}
        onEditar={vi.fn()}
        onVerRegistros={vi.fn()}
        onBorrar={vi.fn()}
      />,
    )

    expect(screen.getByRole('button', { name: 'Arrancar web.service' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Reiniciar web.service' })).toBeDisabled()
  })

  it('muestra el estado vacío cuando no hay quadlets', () => {
    render(
      <TablaDeUnidades
        unidades={[]}
        cargando={false}
        onAccion={vi.fn()}
        onEditar={vi.fn()}
        onVerRegistros={vi.fn()}
        onBorrar={vi.fn()}
      />,
    )

    expect(screen.getByText('No hay quadlets')).toBeInTheDocument()
  })

  it('emite la edición de una fila con su unidad', async () => {
    const usuario = userEvent.setup()
    const { onEditar } = tabla([unidad({ name: 'web.service' })])

    await usuario.click(screen.getByRole('button', { name: 'Editar web.service' }))

    expect(onEditar).toHaveBeenCalledWith(expect.objectContaining({ name: 'web.service' }))
  })

  it('emite la apertura del visor de logs de una fila con su unidad', async () => {
    const usuario = userEvent.setup()
    const { onVerRegistros } = tabla([unidad({ name: 'web.service' })])

    await usuario.click(screen.getByRole('button', { name: 'Logs web.service' }))

    expect(onVerRegistros).toHaveBeenCalledWith(expect.objectContaining({ name: 'web.service' }))
  })

  it('pide confirmación antes de borrar: no emite nada hasta confirmar', async () => {
    const usuario = userEvent.setup()
    const { onBorrar } = tabla([unidad({ name: 'web.service' })])

    await usuario.click(screen.getByRole('button', { name: 'Borrar web.service' }))

    expect(await screen.findByText('¿Borrar la quadlet «web.service»?')).toBeInTheDocument()
    expect(onBorrar).not.toHaveBeenCalled()

    await usuario.click(screen.getByRole('button', { name: 'Sí, borrar' }))

    expect(onBorrar).toHaveBeenCalledWith(expect.objectContaining({ name: 'web.service' }))
  })

  it('un borrado cancelado no emite nada', async () => {
    const usuario = userEvent.setup()
    const { onBorrar } = tabla([unidad({ name: 'web.service' })])

    await usuario.click(screen.getByRole('button', { name: 'Borrar web.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Cancelar' }))

    expect(onBorrar).not.toHaveBeenCalled()
  })

  it('la confirmación nombra el fichero publicado por la API, no solo la unidad', async () => {
    const usuario = userEvent.setup()
    tabla([unidad({ name: 'web-volume.service', file_name: 'web-volume.container' })])

    await usuario.click(screen.getByRole('button', { name: 'Borrar web-volume.service' }))

    // El fichero que se va a borrar es el que publica la API, no el deducido del
    // nombre de unidad (que aquí resolvería al volumen `web.volume`).
    expect(await screen.findByText(/«web-volume\.container»/)).toBeInTheDocument()
  })

  it('distingue dos quadlets con el mismo nombre de unidad por su file_name', async () => {
    const usuario = userEvent.setup()
    // Un contenedor `web-volume` y un volumen `web` colisionan en la unidad
    // `web-volume.service`: solo el `file_name` publicado las distingue.
    tabla([
      unidad({ name: 'web-volume.service', file_name: 'web-volume.container' }),
      unidad({ name: 'web-volume.service', file_name: 'web.volume' }),
    ])

    const botones = screen.getAllByRole('button', { name: 'Borrar web-volume.service' })
    expect(botones).toHaveLength(2)

    await usuario.click(botones[0]!)
    expect(await screen.findByText(/«web-volume\.container»/)).toBeInTheDocument()
    await usuario.click(screen.getByRole('button', { name: 'Cancelar' }))

    await usuario.click(botones[1]!)
    expect(await screen.findByText(/«web\.volume»/)).toBeInTheDocument()
  })
})
