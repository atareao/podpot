/**
 * Contrato que fijan estos tests para GREEN:
 * - `ModalDeRegistros({ abierto, unidad, onCerrar })`. Con `abierto` y `unidad`
 *   abre `useRegistrosDeUnidad(unidad.name)`; al cerrar se da de baja el canal.
 * - Muestra cada línea con su origen (`stdout`/`stderr`), busca con
 *   `aria-label="Buscar en registros"` y autodesplaza solo si el usuario está
 *   pegado al fondo del contenedor `role="log"`.
 * - Un evento `error` del canal se comunica sin tumbar el panel.
 */

import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { EventSourceFalso } from '../../../tests/eventSourceFalso'
import { unidad } from '../../../tests/fixtures'
import { ModalDeRegistros } from './ModalDeRegistros'

const unidadDePrueba = unidad({ name: 'web.service', file_name: 'web.container' })

function montar(abierto = true) {
  const onCerrar = vi.fn()
  const vista = render(
    <ModalDeRegistros abierto={abierto} unidad={unidadDePrueba} onCerrar={onCerrar} />,
  )
  return { ...vista, onCerrar }
}

function emitir(fuente: EventSourceFalso, lineas: Array<{ line: string; stream: 'stdout' | 'stderr' }>) {
  act(() => {
    for (const linea of lineas) fuente.emitir('log', { data: linea })
  })
}

function medirScroll(registro: HTMLElement, distanciaAlFondo: number): void {
  Object.defineProperty(registro, 'scrollHeight', { value: 1000, configurable: true })
  Object.defineProperty(registro, 'clientHeight', { value: 100, configurable: true })
  Object.defineProperty(registro, 'scrollTop', {
    value: 900 - distanciaAlFondo,
    configurable: true,
    writable: true,
  })
  fireEvent.scroll(registro)
}

function doblarScroll(): { desplazar: ReturnType<typeof vi.fn>; restaurar: () => void } {
  const original = Element.prototype.scrollIntoView
  const desplazar = vi.fn()
  Element.prototype.scrollIntoView = desplazar as unknown as Element['scrollIntoView']
  return { desplazar, restaurar: () => (Element.prototype.scrollIntoView = original) }
}

beforeEach(() => {
  EventSourceFalso.reset()
})

describe('ModalDeRegistros', () => {
  it('abre el canal de la unidad y muestra las líneas con su origen', async () => {
    montar()
    const fuente = EventSourceFalso.instancias.at(-1)!
    expect(fuente.url).toBe('/api/v1/units/web.service/logs/stream')

    emitir(fuente, [
      { line: 'servidor listo', stream: 'stdout' },
      { line: 'fallo de red', stream: 'stderr' },
    ])

    expect(await screen.findByText(/servidor listo/)).toBeInTheDocument()
    expect(screen.getByText(/fallo de red/)).toBeInTheDocument()
    const registro = screen.getByRole('log')
    expect(registro).toHaveTextContent(/stdout/)
    expect(registro).toHaveTextContent(/stderr/)
  })

  it('añade líneas nuevas mientras está abierto', async () => {
    montar()
    const fuente = EventSourceFalso.instancias.at(-1)!
    emitir(fuente, [{ line: 'primera', stream: 'stdout' }])
    expect(await screen.findByText(/primera/)).toBeInTheDocument()

    emitir(fuente, [{ line: 'segunda', stream: 'stdout' }])
    expect(await screen.findByText(/segunda/)).toBeInTheDocument()
    expect(screen.getByText(/primera/)).toBeInTheDocument()
  })

  it('busca líneas por texto', async () => {
    const usuario = userEvent.setup()
    montar()
    const fuente = EventSourceFalso.instancias.at(-1)!
    emitir(fuente, [
      { line: 'conexion aceptada', stream: 'stdout' },
      { line: 'sin disco', stream: 'stderr' },
    ])
    expect(await screen.findByText(/conexion aceptada/)).toBeInTheDocument()

    await usuario.type(screen.getByRole('textbox', { name: 'Buscar en registros' }), 'disco')

    expect(screen.getByText(/sin disco/)).toBeInTheDocument()
    expect(screen.queryByText(/conexion aceptada/)).not.toBeInTheDocument()
  })

  it('autodesplaza al llegar líneas solo si el usuario está al fondo', async () => {
    const { desplazar, restaurar } = doblarScroll()
    try {
      montar()
      const fuente = EventSourceFalso.instancias.at(-1)!
      emitir(fuente, [{ line: 'inicio', stream: 'stdout' }])
      const registro = await screen.findByRole('log')
      desplazar.mockClear()

      medirScroll(registro, 0)
      emitir(fuente, [{ line: 'al fondo', stream: 'stdout' }])
      await waitFor(() => expect(desplazar).toHaveBeenCalled())

      desplazar.mockClear()
      medirScroll(registro, 100)
      emitir(fuente, [{ line: 'leyendo arriba', stream: 'stdout' }])
      await screen.findByText(/leyendo arriba/)
      expect(desplazar).not.toHaveBeenCalled()
    } finally {
      restaurar()
    }
  })

  it('no abre el canal si el modal está cerrado y se da de baja al cerrarlo', async () => {
    const vista = montar(false)
    expect(EventSourceFalso.instancias).toHaveLength(0)

    vista.rerender(
      <ModalDeRegistros abierto unidad={unidadDePrueba} onCerrar={vista.onCerrar} />,
    )
    expect(EventSourceFalso.instancias).toHaveLength(1)
    const fuente = EventSourceFalso.instancias[0]!

    vista.rerender(
      <ModalDeRegistros abierto={false} unidad={unidadDePrueba} onCerrar={vista.onCerrar} />,
    )
    expect(fuente.cerrado).toBe(true)
  })

  it('muestra el error del canal sin afectar al resto del panel', async () => {
    const vista = montar()
    const fuente = EventSourceFalso.instancias.at(-1)!

    act(() => {
      fuente.emitir('error', new Event('error'))
    })

    expect(await screen.findByText(/no se pudieron leer los registros/i)).toBeInTheDocument()
    expect(vista.onCerrar).not.toHaveBeenCalled()
  })
})
