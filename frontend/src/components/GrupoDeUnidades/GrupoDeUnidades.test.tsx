/**
 * Contrato que fijan estos tests para GREEN:
 * - `GrupoDeUnidades({ stack, unidades, onAccion?, onEditar?, onVerRegistros?, onBorrar? })`.
 * - La cabecera del grupo es un botón con `aria-expanded`, abierto por defecto.
 * - Dentro hay una tarjeta por unidad, con `file_name` visible, estado propio y
 *   acciones accesibles por `aria-label` (`Arrancar <file_name>`, `Editar …`, …).
 * - Orden determinista: principales (`container`/`pod`) antes que auxiliares
 *   (`volume`/`network`/`null`) y, dentro, por nombre.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { unidad } from '../../../tests/fixtures'
import { GrupoDeUnidades } from './GrupoDeUnidades'

function montar(unidades: Parameters<typeof GrupoDeUnidades>[0]['unidades']) {
  const props = {
    stack: 'web',
    unidades,
    onAccion: vi.fn(),
    onEditar: vi.fn(),
    onVerRegistros: vi.fn(),
    onBorrar: vi.fn(),
  }
  return { ...props, ...render(<GrupoDeUnidades {...props} />) }
}

function ordenDeAcciones(): string[] {
  return screen
    .getAllByRole('button', { name: /^Arrancar / })
    .map((boton) => boton.getAttribute('aria-label') ?? '')
}

describe('GrupoDeUnidades', () => {
  it('pinta una tarjeta por unidad, con su file_name, su estado y sus acciones', () => {
    montar([
      unidad({ name: 'web.service', file_name: 'web.container', active_state: 'active' }),
      unidad({
        name: 'web-volume.service',
        file_name: 'web-volume.volume',
        kind: 'volume',
        active_state: 'failed',
      }),
    ])

    expect(screen.getByText('web.container')).toBeInTheDocument()
    expect(screen.getByText('web-volume.volume')).toBeInTheDocument()
    expect(screen.getByText('En ejecución')).toBeInTheDocument()
    expect(screen.getByText('Fallida')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Editar web.container' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Logs web.container' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Borrar web.container' })).toBeInTheDocument()
  })

  it('ordena las principales antes que las auxiliares, y luego por nombre', () => {
    montar([
      unidad({ name: 'zeta.service', file_name: 'zeta.container', stack: 'web' }),
      unidad({
        name: 'alpha-volume.service',
        file_name: 'alpha-volume.volume',
        kind: 'volume',
        stack: 'web',
      }),
      unidad({ name: 'beta.service', file_name: 'beta.container', stack: 'web' }),
    ])

    expect(ordenDeAcciones()).toEqual([
      'Arrancar beta.container',
      'Arrancar zeta.container',
      'Arrancar alpha-volume.volume',
    ])
  })

  it('expande y colapsa con el ratón, reflejando aria-expanded', async () => {
    const usuario = userEvent.setup()
    montar([unidad({ name: 'web.service', file_name: 'web.container', stack: 'web' })])

    const cabecera = screen.getByRole('button', { name: /web/i, expanded: true })
    expect(cabecera).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('button', { name: 'Arrancar web.container' })).toBeInTheDocument()

    await usuario.click(cabecera)

    expect(cabecera).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('button', { name: 'Arrancar web.container' })).not.toBeInTheDocument()

    await usuario.click(cabecera)

    expect(cabecera).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('button', { name: 'Arrancar web.container' })).toBeInTheDocument()
  })

  it('expande y colapsa con el teclado (Enter y Espacio)', async () => {
    const usuario = userEvent.setup()
    montar([unidad({ name: 'web.service', file_name: 'web.container', stack: 'web' })])
    const cabecera = screen.getByRole('button', { name: /web/i, expanded: true })

    cabecera.focus()
    await usuario.keyboard('{Enter}')
    expect(cabecera).toHaveAttribute('aria-expanded', 'false')

    await usuario.keyboard(' ')
    expect(cabecera).toHaveAttribute('aria-expanded', 'true')
  })

  it('mantiene la tarjeta en su sitio cuando solo cambia el estado por SSE', () => {
    const vista = montar([
      unidad({ name: 'web.service', file_name: 'web.container', active_state: 'active' }),
      unidad({
        name: 'web-volume.service',
        file_name: 'web-volume.volume',
        kind: 'volume',
        active_state: 'inactive',
      }),
    ])

    vista.rerender(
      <GrupoDeUnidades
        stack="web"
        unidades={[
          unidad({ name: 'web.service', file_name: 'web.container', active_state: 'failed' }),
          unidad({
            name: 'web-volume.service',
            file_name: 'web-volume.volume',
            kind: 'volume',
            active_state: 'inactive',
          }),
        ]}
        onAccion={vista.onAccion}
        onEditar={vista.onEditar}
        onVerRegistros={vista.onVerRegistros}
        onBorrar={vista.onBorrar}
      />,
    )

    expect(ordenDeAcciones()).toEqual([
      'Arrancar web.container',
      'Arrancar web-volume.volume',
    ])
    expect(screen.getByText('Fallida')).toBeInTheDocument()
  })

  it('deja las tarjetas como hijos accesibles directos del listado', () => {
    render(
      <div role="list" aria-label="Quadlets">
        <GrupoDeUnidades
          stack="web"
          unidades={[unidad({ name: 'web.service', file_name: 'web.container' })]}
          onAccion={vi.fn()}
        />
      </div>,
    )

    const lista = screen.getByRole('list', { name: 'Quadlets' })
    const tarjeta = screen.getByRole('listitem', { name: 'Unidad web.container' })
    const intermedios: Element[] = []
    for (
      let nodo: Element | null = tarjeta.parentElement;
      nodo !== null && nodo !== lista;
      nodo = nodo.parentElement
    ) {
      intermedios.push(nodo)
    }

    expect(intermedios.length).toBeGreaterThan(0)
    expect(intermedios.every((nodo) => nodo.getAttribute('role') === 'presentation')).toBe(true)
  })
})
