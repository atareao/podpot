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

    await usuario.click(screen.getByRole('button', { name: 'Arrancar web.container' }))

    expect(onAccion).toHaveBeenCalledWith(expect.objectContaining({ name: 'web.service' }), 'start')
  })

  it('identifica cada botón con el file_name de su fila', () => {
    tabla([
      unidad({ name: 'a.service', file_name: 'a.container', active_state: 'active' }),
      unidad({ name: 'b.service', file_name: 'b.container', active_state: 'active' }),
    ])

    // Dos filas, dos botones «Arrancar», cada uno con la identidad de su fila.
    expect(screen.getAllByRole('button', { name: /^Arrancar / })).toHaveLength(2)
    expect(screen.getByRole('button', { name: 'Arrancar a.container' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Arrancar b.container' })).toBeInTheDocument()
  })

  it('deshabilita «Arrancar» en una unidad activa y «Parar» en una parada', () => {
    tabla([
      unidad({ name: 'activa.service', active_state: 'active' }),
      unidad({ name: 'parada.service', active_state: 'inactive' }),
    ])

    expect(screen.getByRole('button', { name: 'Arrancar activa.container' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Parar parada.container' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Arrancar parada.container' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Parar activa.container' })).toBeEnabled()
  })

  it('trata los estados intermedios como su destino: «activating» no se arranca, «deactivating» no se para', () => {
    tabla([
      unidad({ name: 'subiendo.service', active_state: 'activating' }),
      unidad({ name: 'bajando.service', active_state: 'deactivating' }),
    ])

    expect(screen.getByRole('button', { name: 'Arrancar subiendo.container' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Parar bajando.container' })).toBeDisabled()
    // En transición, reiniciar sigue disponible.
    expect(screen.getByRole('button', { name: 'Reiniciar subiendo.container' })).toBeEnabled()
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

    expect(screen.getByRole('button', { name: 'Arrancar web.container' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Reiniciar web.container' })).toBeDisabled()
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

    await usuario.click(screen.getByRole('button', { name: 'Editar web.container' }))

    expect(onEditar).toHaveBeenCalledWith(expect.objectContaining({ name: 'web.service' }))
  })

  it('emite la apertura del visor de logs de una fila con su unidad', async () => {
    const usuario = userEvent.setup()
    const { onVerRegistros } = tabla([unidad({ name: 'web.service' })])

    await usuario.click(screen.getByRole('button', { name: 'Logs web.container' }))

    expect(onVerRegistros).toHaveBeenCalledWith(expect.objectContaining({ name: 'web.service' }))
  })

  it('pide confirmación antes de borrar: no emite nada hasta confirmar', async () => {
    const usuario = userEvent.setup()
    const { onBorrar } = tabla([unidad({ name: 'web.service' })])

    await usuario.click(screen.getByRole('button', { name: 'Borrar web.container' }))

    expect(await screen.findByText('¿Borrar la quadlet «web.service»?')).toBeInTheDocument()
    expect(onBorrar).not.toHaveBeenCalled()

    await usuario.click(screen.getByRole('button', { name: 'Sí, borrar' }))

    expect(onBorrar).toHaveBeenCalledWith(expect.objectContaining({ name: 'web.service' }))
  })

  it('un borrado cancelado no emite nada', async () => {
    const usuario = userEvent.setup()
    const { onBorrar } = tabla([unidad({ name: 'web.service' })])

    await usuario.click(screen.getByRole('button', { name: 'Borrar web.container' }))
    await usuario.click(await screen.findByRole('button', { name: 'Cancelar' }))

    expect(onBorrar).not.toHaveBeenCalled()
  })

  it('la confirmación nombra el fichero publicado por la API, no solo la unidad', async () => {
    const usuario = userEvent.setup()
    tabla([unidad({ name: 'web-volume.service', file_name: 'web-volume.container' })])

    await usuario.click(screen.getByRole('button', { name: 'Borrar web-volume.container' }))

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

    // Cada fila lleva su propio `file_name` en el nombre accesible del botón: ya
    // no comparten «Borrar web-volume.service».
    const botonContenedor = screen.getByRole('button', { name: 'Borrar web-volume.container' })
    const botonVolumen = screen.getByRole('button', { name: 'Borrar web.volume' })

    await usuario.click(botonContenedor)
    expect(await screen.findByText(/«web-volume\.container»/)).toBeInTheDocument()
    await usuario.click(screen.getByRole('button', { name: 'Cancelar' }))

    await usuario.click(botonVolumen)
    expect(await screen.findByText(/«web\.volume»/)).toBeInTheDocument()
  })

  it('muestra el indicador «en curso» con el tipo de acción en español', () => {
    render(
      <TablaDeUnidades
        unidades={[unidad({ name: 'web.service' })]}
        cargando={false}
        progresoPorUnidad={{ 'web.service': ['restart'] }}
        onAccion={vi.fn()}
        onEditar={vi.fn()}
        onVerRegistros={vi.fn()}
        onBorrar={vi.fn()}
      />,
    )

    expect(screen.getByText('reiniciada')).toBeInTheDocument()
    expect(
      screen.getByRole('status', { name: 'Acción en curso en web.container: reiniciada' }),
    ).toBeInTheDocument()
  })

  it('muestra un indicador por cada job en curso de la unidad', () => {
    render(
      <TablaDeUnidades
        unidades={[unidad({ name: 'web.service' })]}
        cargando={false}
        progresoPorUnidad={{ 'web.service': ['start', 'restart'] }}
        onAccion={vi.fn()}
        onEditar={vi.fn()}
        onVerRegistros={vi.fn()}
        onBorrar={vi.fn()}
      />,
    )

    expect(screen.getByText('arrancada')).toBeInTheDocument()
    expect(screen.getByText('reiniciada')).toBeInTheDocument()
  })

  it('cae al tipo de acción crudo cuando no conoce su etiqueta', () => {
    render(
      <TablaDeUnidades
        unidades={[unidad({ name: 'web.service' })]}
        cargando={false}
        progresoPorUnidad={{ 'web.service': ['reload'] }}
        onAccion={vi.fn()}
        onEditar={vi.fn()}
        onVerRegistros={vi.fn()}
        onBorrar={vi.fn()}
      />,
    )

    expect(screen.getByText('reload')).toBeInTheDocument()
  })

  it('no muestra indicador en las unidades sin job en curso', () => {
    render(
      <TablaDeUnidades
        unidades={[unidad({ name: 'web.service' })]}
        cargando={false}
        progresoPorUnidad={{ 'otra.service': ['start'] }}
        onAccion={vi.fn()}
        onEditar={vi.fn()}
        onVerRegistros={vi.fn()}
        onBorrar={vi.fn()}
      />,
    )

    expect(screen.queryByLabelText(/Acción en curso/)).not.toBeInTheDocument()
  })

  // La identidad de una quadlet es el `file_name` que publica la API (basename
  // del fichero), no el nombre de unidad systemd. La fila se identifica por él y
  // los nombres accesibles de acciones y progreso lo usan; el nombre de unidad
  // queda como dato secundario. Las **claves internas** (`enCurso`,
  // `progresoPorUnidad`) siguen por nombre de unidad: es lo que viaja en la
  // acción HTTP (`{ name }`) y en el campo `unit` del SSE de jobs.
  describe('identidad por file_name', () => {
    it('identifica la fila por su file_name y deja el nombre de unidad como dato secundario', () => {
      tabla([unidad({ name: 'web.service', file_name: 'web.container' })])

      // El `file_name` es la identidad primaria: vive en la celda «Unidad»…
      const celdaIdentidad = screen.getByText('web.container').closest('td')
      expect(celdaIdentidad).not.toBeNull()
      expect(celdaIdentidad).toHaveTextContent('web.container')

      // …y el nombre de unidad acompaña como dato secundario, después del `file_name`.
      expect(celdaIdentidad).toHaveTextContent('web.service')
      expect(celdaIdentidad!.textContent!.indexOf('web.container')).toBeLessThan(
        celdaIdentidad!.textContent!.indexOf('web.service'),
      )
    })

    it('nombra cada botón de acción con el file_name de la quadlet', () => {
      tabla([
        unidad({ name: 'a.service', file_name: 'a.container', active_state: 'active' }),
        unidad({ name: 'b.service', file_name: 'b.container', active_state: 'active' }),
      ])

      expect(screen.getByRole('button', { name: 'Arrancar a.container' })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Parar a.container' })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Reiniciar a.container' })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Editar a.container' })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Logs a.container' })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Borrar a.container' })).toBeInTheDocument()
      // Cada fila con la suya: la segunda usa su propio `file_name`.
      expect(screen.getByRole('button', { name: 'Arrancar b.container' })).toBeInTheDocument()
    })

    it('usa el file_name en el nombre accesible de la región de progreso', () => {
      render(
        <TablaDeUnidades
          unidades={[unidad({ name: 'web.service', file_name: 'web.container' })]}
          cargando={false}
          // El índice del progreso sigue por **nombre de unidad** (lo que publica
          // el SSE de jobs en `unit`); solo el `aria-label` usa el `file_name`.
          progresoPorUnidad={{ 'web.service': ['restart'] }}
          onAccion={vi.fn()}
          onEditar={vi.fn()}
          onVerRegistros={vi.fn()}
          onBorrar={vi.fn()}
        />,
      )

      expect(
        screen.getByRole('status', { name: 'Acción en curso en web.container: reiniciada' }),
      ).toBeInTheDocument()
    })

    it('mantiene el bloqueo por enCurso (nombre de unidad) y nombra el botón con el file_name', () => {
      render(
        <TablaDeUnidades
          unidades={[
            unidad({ name: 'web.service', file_name: 'web.container', active_state: 'inactive' }),
          ]}
          cargando={false}
          // `enCurso` sigue siendo el **nombre de unidad**: es lo que se envía en
          // la acción HTTP (`{ name }`). Solo el `aria-label` usa el `file_name`.
          enCurso="web.service"
          onAccion={vi.fn()}
          onEditar={vi.fn()}
          onVerRegistros={vi.fn()}
          onBorrar={vi.fn()}
        />,
      )

      expect(screen.getByRole('button', { name: 'Arrancar web.container' })).toBeDisabled()
      expect(screen.getByRole('button', { name: 'Reiniciar web.container' })).toBeDisabled()
    })

    it('cae al nombre de unidad como identidad cuando la API no publica file_name (null)', () => {
      // Contrato: la identidad de la fila es el `file_name`. Si la API no lo
      // publica (`null`), la fila cae al nombre de unidad para seguir siendo
      // operable y accesible; nunca se renderiza el literal «null».
      const sinFichero = {
        ...unidad({ name: 'web.service', active_state: 'inactive' }),
        file_name: null as unknown as string,
      }

      render(
        <TablaDeUnidades
          unidades={[sinFichero]}
          cargando={false}
          onAccion={vi.fn()}
          onEditar={vi.fn()}
          onVerRegistros={vi.fn()}
          onBorrar={vi.fn()}
        />,
      )

      expect(screen.getByRole('button', { name: 'Arrancar web.service' })).toBeInTheDocument()
      expect(screen.queryByText('null')).not.toBeInTheDocument()
    })
  })
})
