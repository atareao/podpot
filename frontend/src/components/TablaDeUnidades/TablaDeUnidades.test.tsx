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
  it('expone el nombre accesible del listado de tarjetas', () => {
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

    // El render pasa de <table> a grupos de tarjetas: se conserva el nombre
    // accesible del listado, ahora en un contenedor con role="list".
    expect(screen.getByRole('list', { name: 'Quadlets' })).toBeInTheDocument()
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
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

  it('muestra Spin y deshabilita las acciones mientras haya un job en curso', () => {
    render(
      <TablaDeUnidades
        unidades={[unidad({ name: 'web.service', file_name: 'web.container', active_state: 'inactive' })]}
        cargando={false}
        progresoPorUnidad={{ 'web.service': ['start'] }}
        onAccion={vi.fn()}
        onEditar={vi.fn()}
        onVerRegistros={vi.fn()}
        onBorrar={vi.fn()}
      />,
    )

    const estado = screen.getByRole('status', {
      name: 'Acción en curso en web.container: arrancada',
    })
    expect(estado.querySelector('.ant-spin')).not.toBeNull()
    // El progreso SSE, sin `enCurso` de la mutación, también bloquea la fila.
    expect(screen.getByRole('button', { name: 'Arrancar web.container' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Editar web.container' })).toBeDisabled()
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

    expect(await screen.findByText('¿Borrar la quadlet «web.container»?')).toBeInTheDocument()
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
    expect(
      await screen.findByText('¿Borrar la quadlet «web-volume.container»?'),
    ).toBeInTheDocument()
    expect(
      await screen.findByText('Se borrará el fichero «web-volume.container». No se puede deshacer.'),
    ).toBeInTheDocument()
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
    expect(
      await screen.findByText('¿Borrar la quadlet «web-volume.container»?'),
    ).toBeInTheDocument()
    await usuario.click(screen.getByRole('button', { name: 'Cancelar' }))

    await usuario.click(botonVolumen)
    expect(await screen.findByText('¿Borrar la quadlet «web.volume»?')).toBeInTheDocument()
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

      const celdaIdentidad = screen.getByText('web.container').closest('td')
      expect(celdaIdentidad).not.toBeNull()

      const nodoFichero = screen.getByText('web.container')
      const nodoUnidad = screen.getByText('web.service')

      // Ambos viven en la misma celda «Unidad»…
      expect(celdaIdentidad).toContainElement(nodoFichero)
      expect(celdaIdentidad).toContainElement(nodoUnidad)

      // …y el `file_name` precede al nombre de unidad en el orden del documento.
      expect(nodoFichero.compareDocumentPosition(nodoUnidad)).toBe(
        Node.DOCUMENT_POSITION_FOLLOWING,
      )
    })

    it('usa el file_name en el título del Popconfirm de borrado', async () => {
      const usuario = userEvent.setup()
      tabla([unidad({ name: 'web.service', file_name: 'web.container' })])

      await usuario.click(screen.getByRole('button', { name: 'Borrar web.container' }))

      // El título identifica la quadlet por el fichero publicado por la API…
      const titulo = await screen.findByText('¿Borrar la quadlet «web.container»?')
      expect(titulo).toBeInTheDocument()
      // …y no por el nombre de unidad.
      expect(titulo).not.toHaveTextContent('web.service')
    })

    it('cae al nombre de unidad en el Popconfirm cuando el file_name está vacío', async () => {
      const usuario = userEvent.setup()
      // Contrato: `file_name` es una cadena no nula; ante una cadena vacía la UI
      // cae al nombre de unidad como identidad defensiva.
      tabla([unidad({ name: 'web.service', file_name: '' })])

      await usuario.click(screen.getByRole('button', { name: 'Borrar web.service' }))

      expect(await screen.findByText('¿Borrar la quadlet «web.service»?')).toBeInTheDocument()
      expect(
        await screen.findByText('Se borrará el fichero «web.service». No se puede deshacer.'),
      ).toBeInTheDocument()

      // Ni el literal `null` ni comillas vacías.
      expect(screen.queryByText(/null/)).not.toBeInTheDocument()
      expect(screen.queryByText(/«»/)).not.toBeInTheDocument()
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

    it('cae al nombre de unidad como identidad cuando el file_name está vacío', () => {
      // Contrato: `file_name` es una cadena no nula; si llega vacía, la fila cae
      // al nombre de unidad para seguir siendo operable y accesible. Nunca se
      // renderiza el literal «null».
      const sinFichero = unidad({ name: 'web.service', file_name: '', active_state: 'inactive' })

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
      expect(screen.queryByText(/null/)).not.toBeInTheDocument()
    })
  })
})

describe('TablaDeUnidades — controles de acción con iconos', () => {
  const ACCIONES = ['Arrancar', 'Parar', 'Reiniciar', 'Editar', 'Logs', 'Borrar']

  function botonDeAccion(accion: string) {
    return screen.getByRole('button', { name: `${accion} web.container` })
  }

  it('no muestra la palabra de la acción como texto visible dentro del control', () => {
    tabla([unidad({ name: 'web.service', file_name: 'web.container' })])

    for (const accion of ACCIONES) {
      const boton = botonDeAccion(accion)

      // No debe quedar texto visible de la acción dentro del botón.
      expect(boton.textContent).not.toContain(accion)
      // `getByText` tampoco debe localizar la etiqueta textual.
      expect(screen.queryByText(accion)).not.toBeInTheDocument()
    }
  })

  it('expone el nombre accesible de cada acción por su file_name', () => {
    tabla([unidad({ name: 'web.service', file_name: 'web.container' })])

    for (const accion of ACCIONES) {
      expect(botonDeAccion(accion)).toBeInTheDocument()
    }
  })

  it('presenta cada acción con un icono en lugar de texto', () => {
    tabla([unidad({ name: 'web.service', file_name: 'web.container' })])

    for (const accion of ACCIONES) {
      const boton = botonDeAccion(accion)
      const icono = boton.querySelector('svg, [role="img"], .anticon')
      expect(icono).not.toBeNull()
    }
  })

  // rc-tooltip inyecta el `title` en un nodo con role="tooltip" al hacer hover;
  // en jsdom funciona sin temporizadores ficticios, así que se comprueba el
  // texto de cada tooltip (la etiqueta centralizada del módulo de mapeo).
  it.each(ACCIONES)('muestra el tooltip «%s» al pasar el cursor', async (accion) => {
    const usuario = userEvent.setup()
    tabla([unidad({ name: 'web.service', file_name: 'web.container' })])

    await usuario.hover(botonDeAccion(accion))

    expect(await screen.findByRole('tooltip')).toHaveTextContent(accion)
  })

  it('conserva la llamada a onAccion al parar desde el botón con icono', async () => {
    const usuario = userEvent.setup()
    const objetivo = unidad({ name: 'web.service', file_name: 'web.container' })
    const { onAccion } = tabla([objetivo])

    await usuario.click(botonDeAccion('Parar'))

    expect(onAccion).toHaveBeenCalledWith(objetivo, 'stop')
  })

  it('conserva la llamada a onAccion al reiniciar desde el botón con icono', async () => {
    const usuario = userEvent.setup()
    const objetivo = unidad({ name: 'web.service', file_name: 'web.container' })
    const { onAccion } = tabla([objetivo])

    await usuario.click(botonDeAccion('Reiniciar'))

    expect(onAccion).toHaveBeenCalledWith(objetivo, 'restart')
  })
})

describe('resaltado tras un cambio en vivo', () => {
  const props = {
    cargando: false,
    onAccion: vi.fn(),
    onEditar: vi.fn(),
    onVerRegistros: vi.fn(),
    onBorrar: vi.fn(),
  }
  const antes = [
    unidad({ name: 'web.service', file_name: 'web.container', active_state: 'active' }),
    unidad({ name: 'db.service', file_name: 'db.container', active_state: 'inactive' }),
  ]

  it('no resalta al cambiar de filtros si la instantánea completa no cambia', () => {
    const vista = render(<TablaDeUnidades {...props} unidades={[antes[0]]} instantanea={antes} />)

    vista.rerender(<TablaDeUnidades {...props} unidades={[antes[1]]} instantanea={antes} />)

    expect(vista.container.querySelectorAll('.podpot-tarjeta--resaltada')).toHaveLength(0)
  })

  it('resalta cuando cambia el estado en la instantánea, no al filtrar', () => {
    const despues = [unidad({ ...antes[0], active_state: 'inactive' }), antes[1]]
    const vista = render(<TablaDeUnidades {...props} unidades={[antes[0]]} instantanea={antes} />)

    vista.rerender(<TablaDeUnidades {...props} unidades={[despues[0]]} instantanea={despues} />)

    expect(screen.getByRole('listitem', { name: 'Unidad web.container' }).className).toContain(
      'podpot-tarjeta--resaltada',
    )
  })

  it('no resalta una unidad que aparece por primera vez en la instantánea', () => {
    const vista = render(<TablaDeUnidades {...props} unidades={[]} instantanea={[]} />)

    vista.rerender(<TablaDeUnidades {...props} unidades={[antes[1]]} instantanea={antes} />)

    expect(vista.container.querySelectorAll('.podpot-tarjeta--resaltada')).toHaveLength(0)
  })

  it('apaga el resaltado al llegar otra instantánea sin cambios de estado', () => {
    const despues = [unidad({ ...antes[0], active_state: 'inactive' }), antes[1]]
    const vista = render(<TablaDeUnidades {...props} unidades={[antes[0]]} instantanea={antes} />)

    vista.rerender(<TablaDeUnidades {...props} unidades={[despues[0]]} instantanea={despues} />)
    expect(screen.getByRole('listitem', { name: 'Unidad web.container' }).className).toContain(
      'podpot-tarjeta--resaltada',
    )

    // El cleanup canceló el temporizador: sin un nuevo cambio que lo reinicie,
    // una instantánea posterior debe apagar el resaltado.
    vista.rerender(
      <TablaDeUnidades {...props} unidades={[despues[0]]} instantanea={[...despues]} />,
    )

    expect(vista.container.querySelectorAll('.podpot-tarjeta--resaltada')).toHaveLength(0)
  })
})
