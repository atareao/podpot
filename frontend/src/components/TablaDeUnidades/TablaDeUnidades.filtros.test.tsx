import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { unidad } from '../../../tests/fixtures'
import { TablaDeUnidades } from './TablaDeUnidades'

type Unidades = Parameters<typeof TablaDeUnidades>[0]['unidades']

function tabla(unidades: Unidades) {
  const props = {
    unidades,
    cargando: false,
    onAccion: vi.fn(),
    onEditar: vi.fn(),
    onVerRegistros: vi.fn(),
    onBorrar: vi.fn(),
  }
  return { ...props, render: () => render(<TablaDeUnidades {...props} />) }
}

function ordenDeFilas() {
  return screen
    .getAllByRole('button', { name: /^Arrancar / })
    .map((boton) => boton.getAttribute('aria-label'))
}

describe('TablaDeUnidades — agrupación y filtros', () => {
  it('muestra una cabecera por stack, con las unidades juntas y su propio estado', () => {
    tabla([
      unidad({
        name: 'zeta.service',
        file_name: 'zeta.container',
        stack: 'zeta',
        active_state: 'active',
      }),
      unidad({
        name: 'alpha.service',
        file_name: 'alpha.container',
        stack: 'alpha',
        active_state: 'inactive',
      }),
      unidad({
        name: 'alpha-volume.service',
        file_name: 'alpha-volume.volume',
        stack: 'alpha',
        kind: 'volume',
        active_state: 'failed',
      }),
    ]).render()

    expect(screen.getByText('alpha').compareDocumentPosition(screen.getByText('zeta'))).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    )
    expect(screen.getByText('alpha').closest('td')).toHaveAttribute('rowspan', '2')
    expect(ordenDeFilas()).toEqual([
      'Arrancar alpha.container',
      'Arrancar alpha-volume.volume',
      'Arrancar zeta.container',
    ])

    // Cada fila conserva su estado y sus acciones: la cabecera del grupo no las sustituye.
    const filaZeta = screen.getByRole('button', { name: 'Arrancar zeta.container' }).closest('tr')
    expect(filaZeta).not.toBeNull()
    expect(within(filaZeta as HTMLElement).getByText('En ejecución')).toBeInTheDocument()
    const filaVolumen = screen
      .getByRole('button', { name: 'Arrancar alpha-volume.volume' })
      .closest('tr')
    expect(filaVolumen).not.toBeNull()
    expect(within(filaVolumen as HTMLElement).getByText('Fallida')).toBeInTheDocument()
  })

  it('filtra por tipo y mantiene la agrupación por stack', async () => {
    const usuario = userEvent.setup()
    tabla([
      unidad({ name: 'web.service', file_name: 'web.container', stack: 'web' }),
      unidad({
        name: 'web-volume.service',
        file_name: 'web-volume.volume',
        stack: 'web',
        kind: 'volume',
        active_state: 'inactive',
      }),
      unidad({ name: 'otro.service', file_name: 'otro.container', stack: 'otro' }),
    ]).render()

    await usuario.click(screen.getByRole('combobox', { name: 'Filtrar por tipo' }))
    // Antd pint una opción visible (con `title`) y un clon oculto con
    // `role="option"` para lectores de pantalla: hay que pulsar la visible.
    await usuario.click(await screen.findByTitle('volume'))

    expect(ordenDeFilas()).toEqual(['Arrancar web-volume.volume'])
    expect(screen.getByText('web')).toBeInTheDocument()
    expect(screen.queryByText('otro')).not.toBeInTheDocument()
  })

  it('combina los filtros por tipo y por estado', async () => {
    const usuario = userEvent.setup()
    tabla([
      unidad({
        name: 'web-volume.service',
        file_name: 'web-volume.volume',
        stack: 'web',
        kind: 'volume',
        active_state: 'inactive',
      }),
      unidad({
        name: 'web-network.service',
        file_name: 'web-network.network',
        stack: 'web',
        kind: 'network',
        active_state: 'inactive',
      }),
      unidad({
        name: 'otro-volume.service',
        file_name: 'otro-volume.volume',
        stack: 'otro',
        kind: 'volume',
        active_state: 'active',
      }),
    ]).render()

    await usuario.click(screen.getByRole('combobox', { name: 'Filtrar por tipo' }))
    await usuario.click(await screen.findByTitle('volume'))
    await usuario.click(screen.getByRole('combobox', { name: 'Filtrar por estado' }))
    await usuario.click(await screen.findByTitle('inactive'))

    expect(ordenDeFilas()).toEqual(['Arrancar web-volume.volume'])
  })

  it('muestra un estado vacío explícito y sin grupos vacíos cuando no hay coincidencias', async () => {
    const usuario = userEvent.setup()
    tabla([
      unidad({ name: 'web.service', file_name: 'web.container', stack: 'web', active_state: 'active' }),
      unidad({
        name: 'otro-volume.service',
        file_name: 'otro-volume.volume',
        stack: 'otro',
        kind: 'volume',
        active_state: 'inactive',
      }),
    ]).render()

    await usuario.click(screen.getByRole('combobox', { name: 'Filtrar por tipo' }))
    await usuario.click(await screen.findByTitle('container'))
    await usuario.click(screen.getByRole('combobox', { name: 'Filtrar por estado' }))
    await usuario.click(await screen.findByTitle('inactive'))

    expect(screen.getByText('No hay quadlets que coincidan con los filtros')).toBeInTheDocument()
    expect(screen.queryByText('web')).not.toBeInTheDocument()
    expect(screen.queryByText('otro')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^Arrancar / })).not.toBeInTheDocument()
  })

  it('conserva posiciones y filtros al llegar un cambio en vivo del estado', async () => {
    const usuario = userEvent.setup()
    const escenario = tabla([
      unidad({ name: 'web.service', file_name: 'web.container', stack: 'web', active_state: 'inactive' }),
      unidad({
        name: 'web-volume.service',
        file_name: 'web-volume.volume',
        stack: 'web',
        kind: 'volume',
        active_state: 'active',
      }),
    ])
    const { render: pintar, ...props } = escenario
    const vista = pintar()

    await usuario.click(screen.getByRole('combobox', { name: 'Filtrar por tipo' }))
    await usuario.click(await screen.findByTitle('volume'))

    vista.rerender(
      <TablaDeUnidades
        {...props}
        unidades={[
          unidad({ name: 'web.service', file_name: 'web.container', stack: 'web', active_state: 'active' }),
          unidad({
            name: 'web-volume.service',
            file_name: 'web-volume.volume',
            stack: 'web',
            kind: 'volume',
            active_state: 'failed',
          }),
        ]}
      />,
    )

    expect(ordenDeFilas()).toEqual(['Arrancar web-volume.volume'])
    const fila = screen.getByRole('button', { name: 'Arrancar web-volume.volume' }).closest('tr')
    expect(within(fila as HTMLElement).getByText('Fallida')).toBeInTheDocument()
  })
})
