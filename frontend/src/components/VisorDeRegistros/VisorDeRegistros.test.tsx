import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { EventSourceFalso } from '../../../tests/eventSourceFalso'
import { VisorDeRegistros } from './VisorDeRegistros'

beforeEach(() => {
  EventSourceFalso.reset()
})

/** Emite líneas `log` de golpe; el hook las consolida en el siguiente volcado. */
function emitir(
  fuente: EventSourceFalso,
  lineas: Array<{ line: string; stream: 'stdout' | 'stderr' }>,
): void {
  act(() => {
    for (const linea of lineas) fuente.emitir('log', { data: linea })
  })
}

/** Dobla `scrollIntoView` (jsdom no lo trae) y devuelve el espía y su restaurador. */
function doblarScrollIntoView(): { desplazar: ReturnType<typeof vi.fn>; restaurar: () => void } {
  const original = Element.prototype.scrollIntoView
  const desplazar = vi.fn()
  Element.prototype.scrollIntoView = desplazar as unknown as Element['scrollIntoView']
  return { desplazar, restaurar: () => (Element.prototype.scrollIntoView = original) }
}

describe('VisorDeRegistros', () => {
  it('al abrirse se suscribe al canal de logs de esa unidad', () => {
    render(<VisorDeRegistros unidad="web.service" onCerrar={vi.fn()} />)

    expect(EventSourceFalso.instancias).toHaveLength(1)
    expect(EventSourceFalso.instancias[0]!.url).toBe('/api/v1/units/web.service/logs/stream')
  })

  it('muestra las líneas que llegan y distingue el origen `stderr`', async () => {
    render(<VisorDeRegistros unidad="web.service" onCerrar={vi.fn()} />)

    emitir(EventSourceFalso.instancias[0]!, [
      { line: 'hola mundo', stream: 'stdout' },
      { line: 'algo falló', stream: 'stderr' },
    ])

    expect(await screen.findByText('hola mundo')).toBeInTheDocument()
    const enError = await screen.findByText('algo falló')
    expect(enError).toHaveClass('podpot-registro--stderr')
  })

  it('apaga `aria-live` en el log: un log denso no debe inundar al lector de pantalla', async () => {
    render(<VisorDeRegistros unidad="web.service" onCerrar={vi.fn()} />)

    emitir(EventSourceFalso.instancias[0]!, [{ line: 'una', stream: 'stdout' }])
    await screen.findByText('una')

    expect(screen.getByRole('log')).toHaveAttribute('aria-live', 'off')
  })

  it('arrastra el scroll al final al llegar líneas si el usuario está pegado al fondo', async () => {
    const { desplazar, restaurar } = doblarScrollIntoView()
    try {
      render(<VisorDeRegistros unidad="web.service" onCerrar={vi.fn()} />)
      const fuente = EventSourceFalso.instancias[0]!

      emitir(fuente, [{ line: 'primera', stream: 'stdout' }])
      await screen.findByText('primera')

      desplazar.mockClear()
      emitir(fuente, [{ line: 'segunda', stream: 'stdout' }])
      await screen.findByText('segunda')

      expect(desplazar).toHaveBeenCalled()
    } finally {
      restaurar()
    }
  })

  it('no arrastra el scroll al final si el usuario subió a leer', async () => {
    const { desplazar, restaurar } = doblarScrollIntoView()
    try {
      render(<VisorDeRegistros unidad="web.service" onCerrar={vi.fn()} />)
      const fuente = EventSourceFalso.instancias[0]!

      emitir(fuente, [{ line: 'primera', stream: 'stdout' }])
      await screen.findByText('primera')

      // El usuario subió: queda mucho contenido por debajo del viewport.
      const registro = screen.getByRole('log')
      Object.defineProperty(registro, 'scrollHeight', { value: 1000, configurable: true })
      Object.defineProperty(registro, 'clientHeight', { value: 100, configurable: true })
      Object.defineProperty(registro, 'scrollTop', { value: 0, configurable: true, writable: true })
      fireEvent.scroll(registro)

      desplazar.mockClear()
      emitir(fuente, [{ line: 'segunda', stream: 'stdout' }])
      await screen.findByText('segunda')

      expect(desplazar).not.toHaveBeenCalled()
    } finally {
      restaurar()
    }
  })

  it('al cerrar el visor da de baja el canal', () => {
    const { rerender } = render(<VisorDeRegistros unidad="web.service" onCerrar={vi.fn()} />)
    const fuente = EventSourceFalso.instancias[0]!

    expect(fuente.cerrado).toBe(false)

    rerender(<VisorDeRegistros unidad={null} onCerrar={vi.fn()} />)

    expect(fuente.cerrado).toBe(true)
  })

  it('avisa al cerrar desde el propio drawer', async () => {
    const usuario = userEvent.setup()
    const onCerrar = vi.fn()
    render(<VisorDeRegistros unidad="web.service" onCerrar={onCerrar} />)

    await usuario.click(screen.getByRole('button', { name: /close|cerrar/i }))

    expect(onCerrar).toHaveBeenCalled()
  })

  it('muestra un aviso si el canal falla, sin romper el visor', () => {
    render(<VisorDeRegistros unidad="web.service" onCerrar={vi.fn()} />)

    act(() => {
      EventSourceFalso.instancias[0]!.emitir('error', {})
    })

    expect(screen.getByText(/no se pudieron leer los registros/i)).toBeInTheDocument()
  })

  it('limpia el aviso al reconectar', async () => {
    render(<VisorDeRegistros unidad="web.service" onCerrar={vi.fn()} />)
    const fuente = EventSourceFalso.instancias[0]!

    act(() => {
      fuente.emitir('error', {})
    })
    expect(screen.getByText(/no se pudieron leer los registros/i)).toBeInTheDocument()

    act(() => {
      fuente.emitir('open', {})
    })

    expect(screen.queryByText(/no se pudieron leer los registros/i)).not.toBeInTheDocument()
  })

  it('sin líneas aún muestra el estado vacío', () => {
    render(<VisorDeRegistros unidad="web.service" onCerrar={vi.fn()} />)

    expect(screen.getByText(/sin registros/i)).toBeInTheDocument()
  })
})
