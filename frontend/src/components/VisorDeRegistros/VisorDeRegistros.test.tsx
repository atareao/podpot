import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
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

/**
 * Fija las medidas de scroll del registro y simula el desplazamiento del usuario.
 *
 * Deja el contenedor a `scrollHeight - scrollTop - clientHeight` px del fondo, que
 * es la distancia con la que el visor decide si el usuario está «pegado al fondo».
 * jsdom no calcula layout, así que estas medidas hay que fijarlas a mano.
 */
function simularScroll(
  registro: HTMLElement,
  medidas: { scrollHeight: number; clientHeight: number; scrollTop: number },
): void {
  Object.defineProperty(registro, 'scrollHeight', { value: medidas.scrollHeight, configurable: true })
  Object.defineProperty(registro, 'clientHeight', { value: medidas.clientHeight, configurable: true })
  Object.defineProperty(registro, 'scrollTop', {
    value: medidas.scrollTop,
    configurable: true,
    writable: true,
  })
  fireEvent.scroll(registro)
}

/** Abre el visor de una unidad, emite una primera línea y devuelve lo necesario. */
async function abrirConPrimeraLinea(unidad = 'web.service') {
  const vista = render(<VisorDeRegistros unidad={unidad} onCerrar={vi.fn()} />)
  const fuente = EventSourceFalso.instancias.at(-1)!
  emitir(fuente, [{ line: 'primera', stream: 'stdout' }])
  // El contenedor del log solo existe cuando hay líneas: esperarlo deja el volcado
  // por lote ya aplicado y sus efectos, corridos.
  const registro = await screen.findByRole('log')
  return { ...vista, fuente, registro }
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
      const { fuente } = await abrirConPrimeraLinea()

      desplazar.mockClear()
      emitir(fuente, [{ line: 'segunda', stream: 'stdout' }])
      await screen.findByText('segunda')

      // El autodesplazamiento lo dispara un efecto tras el volcado por lote: se
      // espera a él en vez de afirmar a renglón seguido (sin depender del timing).
      await waitFor(() => expect(desplazar).toHaveBeenCalled())
    } finally {
      restaurar()
    }
  })

  it('no arrastra el scroll al final si el usuario subió a leer', async () => {
    const { desplazar, restaurar } = doblarScrollIntoView()
    try {
      const { fuente, registro } = await abrirConPrimeraLinea()

      // El usuario subió: queda mucho contenido por debajo del viewport.
      simularScroll(registro, { scrollHeight: 1000, clientHeight: 100, scrollTop: 0 })

      desplazar.mockClear()
      emitir(fuente, [{ line: 'segunda', stream: 'stdout' }])
      await screen.findByText('segunda')

      expect(desplazar).not.toHaveBeenCalled()
    } finally {
      restaurar()
    }
  })

  it('trata el umbral de 24 px como frontera: 25 px no desplaza, 24 px sí', async () => {
    const { desplazar, restaurar } = doblarScrollIntoView()
    try {
      const { fuente, registro } = await abrirConPrimeraLinea()

      // 25 px hasta el fondo: por encima del umbral, no arrastra.
      simularScroll(registro, { scrollHeight: 125, clientHeight: 100, scrollTop: 0 })
      desplazar.mockClear()
      emitir(fuente, [{ line: 'segunda', stream: 'stdout' }])
      await screen.findByText('segunda')
      expect(desplazar).not.toHaveBeenCalled()

      // 24 px hasta el fondo: justo en el límite, sí arrastra.
      simularScroll(registro, { scrollHeight: 124, clientHeight: 100, scrollTop: 0 })
      desplazar.mockClear()
      emitir(fuente, [{ line: 'tercera', stream: 'stdout' }])
      await screen.findByText('tercera')
      await waitFor(() => expect(desplazar).toHaveBeenCalled())
    } finally {
      restaurar()
    }
  })

  it('al reabrir el visor vuelve a considerarse pegado al fondo', async () => {
    const { desplazar, restaurar } = doblarScrollIntoView()
    try {
      const { rerender, fuente, registro } = await abrirConPrimeraLinea()

      // El usuario sube a leer antes de cerrar.
      simularScroll(registro, { scrollHeight: 1000, clientHeight: 100, scrollTop: 0 })

      // Cierra y reabre la misma unidad: el cuerpo se remonta y parte de «pegado».
      rerender(<VisorDeRegistros unidad={null} onCerrar={vi.fn()} />)
      rerender(<VisorDeRegistros unidad="web.service" onCerrar={vi.fn()} />)

      const fuenteReabierta = EventSourceFalso.instancias.at(-1)!
      expect(fuenteReabierta).not.toBe(fuente)

      desplazar.mockClear()
      emitir(fuenteReabierta, [{ line: 'tras reabrir', stream: 'stdout' }])
      await screen.findByText('tras reabrir')

      await waitFor(() => expect(desplazar).toHaveBeenCalled())
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
