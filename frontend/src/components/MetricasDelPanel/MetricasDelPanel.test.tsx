/**
 * Contrato que fijan estos tests para GREEN:
 * - `MetricasDelPanel({ unidades })` recibe la instantánea de `QuadletUnit[]`.
 * - Cada tarjeta es un `role="group"` con `aria-label` igual a su título
 *   (`Total`, `En ejecución`, `Paradas`, `Fallidas`) y el número dentro.
 */

import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { act, render, screen, within } from '@testing-library/react'
import { ConfigProvider, theme } from 'antd'
import { unidad } from '../../../tests/fixtures'
import { MetricasDelPanel } from './MetricasDelPanel'

const CONSULTA_MOVIL = '(max-width: 767px)'
const TARJETAS = ['Total', 'En ejecución', 'Paradas', 'Fallidas'] as const
const ESTILOS =
  [resolve(process.cwd(), 'src/estilos.css'), resolve(process.cwd(), 'frontend/src/estilos.css')].find(
    existsSync,
  )!

const matchMediaOriginal = window.matchMedia

type Token = ReturnType<typeof theme.useToken>['token']

let tokensCapturados: Token | undefined

function CapturarTokens() {
  tokensCapturados = theme.useToken().token
  return null
}

function valorEn(tarjeta: (typeof TARJETAS)[number]): string {
  return within(screen.getByRole('group', { name: tarjeta })).getAllByText(/^\d+$/)[0]!.textContent ?? ''
}

afterEach(() => {
  window.matchMedia = matchMediaOriginal
  tokensCapturados = undefined
})

/** Doble de `matchMedia` que solo conmuta la consulta móvil y notifica cambios. */
function simularMovil(coincide: boolean) {
  const oyentes = new Set<(evento: MediaQueryListEvent) => void>()
  const alta = (tipo: string, oyente: (evento: MediaQueryListEvent) => void) => {
    if (tipo === 'change') oyentes.add(oyente)
  }
  const baja = vi.fn((tipo: string, oyente: (evento: MediaQueryListEvent) => void) => {
    if (tipo === 'change') oyentes.delete(oyente)
  })
  window.matchMedia = vi.fn().mockImplementation((consulta: string) => {
    if (consulta !== CONSULTA_MOVIL) return matchMediaOriginal(consulta)
    return {
      get matches() {
        return coincide
      },
      media: consulta,
      addEventListener: alta,
      removeEventListener: baja,
      addListener: alta,
      removeListener: baja,
    } as unknown as MediaQueryList
  }) as typeof window.matchMedia
  return {
    cambiar(nuevo: boolean) {
      coincide = nuevo
      act(() => {
        for (const oyente of oyentes) {
          oyente({ matches: nuevo } as MediaQueryListEvent)
        }
      })
    },
    baja,
  }
}

describe('MetricasDelPanel', () => {
  it('muestra cuatro tarjetas con el recuento de la instantánea', () => {
    render(
      <MetricasDelPanel
        unidades={[
          unidad({ name: 'web.service', active_state: 'active' }),
          unidad({ name: 'api.service', active_state: 'active' }),
          unidad({ name: 'worker.service', active_state: 'active' }),
          unidad({ name: 'db.service', active_state: 'inactive' }),
          unidad({ name: 'cache.service', active_state: 'inactive' }),
          unidad({ name: 'vieja.service', active_state: 'failed' }),
        ]}
      />,
    )

    expect(valorEn('Total')).toBe('6')
    expect(valorEn('En ejecución')).toBe('3')
    expect(valorEn('Paradas')).toBe('2')
    expect(valorEn('Fallidas')).toBe('1')
  })

  it('muestra cero en las cuatro tarjetas cuando la instantánea está vacía', () => {
    render(<MetricasDelPanel unidades={[]} />)

    for (const tarjeta of TARJETAS) expect(valorEn(tarjeta)).toBe('0')
  })

  it('actualiza los recuentos con la siguiente instantánea', () => {
    const vista = render(
      <MetricasDelPanel unidades={[unidad({ name: 'web.service', active_state: 'active' })]} />,
    )
    expect(valorEn('Total')).toBe('1')

    vista.rerender(
      <MetricasDelPanel
        unidades={[
          unidad({ name: 'web.service', active_state: 'failed' }),
          unidad({ name: 'db.service', active_state: 'inactive' }),
        ]}
      />,
    )

    expect(valorEn('Total')).toBe('2')
    expect(valorEn('En ejecución')).toBe('0')
    expect(valorEn('Paradas')).toBe('1')
    expect(valorEn('Fallidas')).toBe('1')
  })

  it('no inventa la métrica de actualizaciones', () => {
    render(<MetricasDelPanel unidades={[unidad({ name: 'web.service' })]} />)

    expect(screen.queryByText(/actualizaciones/i)).not.toBeInTheDocument()
    expect(screen.getAllByRole('group')).toHaveLength(4)
  })

  it('en tema oscuro deriva los colores de los tokens y nunca usa blanco fijo', () => {
    render(
      <ConfigProvider theme={{ algorithm: theme.darkAlgorithm }}>
        <CapturarTokens />
        <MetricasDelPanel unidades={[]} />
      </ConfigProvider>,
    )

    const token = tokensCapturados!
    const tarjeta = screen.getByRole('group', { name: 'Total' })
    const etiqueta = within(tarjeta).getByText('Total')

    expect(token.colorBgContainer).not.toBe('#ffffff')
    expect(tarjeta).toHaveStyle({ backgroundColor: token.colorBgContainer })
    expect(tarjeta).toHaveStyle({ borderTopColor: token.colorPrimary })
    expect(etiqueta).toHaveStyle({ color: token.colorTextSecondary })
    expect(tarjeta.getAttribute('style')).not.toMatch(/#fff\b/i)

    const css = readFileSync(ESTILOS, 'utf8')
    const cabecera = css.match(/\.podpot-cabecera\s*\{[^}]*\}/)?.[0] ?? ''
    const tarjetaCss = css.match(/\.podpot-tarjeta\s*\{[^}]*\}/)?.[0] ?? ''
    expect(cabecera).toContain('var(--ant-color-bg-container')
    expect(cabecera).toContain('var(--ant-color-text')
    expect(tarjetaCss).toContain('var(--ant-color-bg-container')
    expect(tarjetaCss).toContain('var(--ant-color-text')
    expect(cabecera + tarjetaCss).not.toMatch(/255,\s*255,\s*255|#fff\b/i)
  })

  it('reduce padding y etiquetas por debajo de 768 px y reacciona a cambios de matchMedia', () => {
    const movil = simularMovil(true)
    const vista = render(<MetricasDelPanel unidades={[]} />)

    const tarjeta = screen.getByRole('group', { name: 'Total' })
    const etiqueta = within(tarjeta).getByText('Total')
    expect(tarjeta.style.padding).toBe('8px 16px')
    expect(etiqueta.style.fontSize).toBe('12px')

    movil.cambiar(false)
    expect(tarjeta.style.padding).toBe('16px 24px')
    expect(etiqueta.style.fontSize).toBe('14px')

    vista.unmount()
    expect(movil.baja).toHaveBeenCalled()
  })
})
