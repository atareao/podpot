/**
 * Contrato que fijan estos tests para GREEN:
 * - `BarraDeBusqueda({ unidades, onResultados, filtros? })`.
 * - El campo tiene `aria-label="Buscar unidades"`.
 * - El texto se aplica con ~250 ms de debounce a `file_name`, `name`, `stack` y
 *   `kind`; `filtros.kind` y `filtros.active_state` se aplican de inmediato.
 * - `onResultados` recibe la lista filtrada; con cero coincidencias se muestra
 *   «No hay unidades que coincidan con los filtros».
 */

import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { unidad } from '../../../tests/fixtures'
import { BarraDeBusqueda } from './BarraDeBusqueda'
import type { ActiveState, QuadletUnit, TipoDeQuadlet } from '../../types/api'

const ESTILOS =
  [resolve(process.cwd(), 'src/estilos.css'), resolve(process.cwd(), 'frontend/src/estilos.css')].find(
    existsSync,
  )!

function luminancia(hex: string): number {
  const canales = [1, 3, 5].map((inicio) => parseInt(hex.slice(inicio, inicio + 2), 16) / 255)
  const [rojo, verde, azul] = canales.map((canal) =>
    canal <= 0.03928 ? canal / 12.92 : ((canal + 0.055) / 1.055) ** 2.4,
  )
  return 0.2126 * rojo! + 0.7152 * verde! + 0.0722 * azul!
}

function contraste(uno: string, dos: string): number {
  const clara = Math.max(luminancia(uno), luminancia(dos))
  const oscura = Math.min(luminancia(uno), luminancia(dos))
  return (clara + 0.05) / (oscura + 0.05)
}

interface Filtros {
  kind?: TipoDeQuadlet
  active_state?: ActiveState
}

function montar(unidades: QuadletUnit[], filtros?: Filtros) {
  const onResultados = vi.fn()
  const vista = render(
    <BarraDeBusqueda unidades={unidades} filtros={filtros} onResultados={onResultados} />,
  )
  // La lista inicial ya se notifica al montar: el test solo mide los cambios
  // provocados por lo que escriba a partir de aquí.
  onResultados.mockClear()
  return { ...vista, onResultados }
}

function escribir(texto: string): void {
  act(() => {
    fireEvent.change(screen.getByRole('textbox', { name: 'Buscar unidades' }), {
      target: { value: texto },
    })
    vi.advanceTimersByTime(300)
  })
}

function identidades(resultados: QuadletUnit[]): string[] {
  return resultados.map((unidad) => unidad.file_name)
}

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('BarraDeBusqueda', () => {
  it('filtra por file_name tras unos 250 ms, no antes', () => {
    const { onResultados } = montar([
      unidad({ name: 'web.service', file_name: 'web.container', stack: 'web' }),
      unidad({ name: 'db.service', file_name: 'db.container', stack: 'db' }),
    ])

    act(() => {
      fireEvent.change(screen.getByRole('textbox', { name: 'Buscar unidades' }), {
        target: { value: 'web' },
      })
    })
    act(() => {
      vi.advanceTimersByTime(249)
    })
    expect(onResultados).not.toHaveBeenCalled()

    act(() => {
      vi.advanceTimersByTime(1)
    })
    expect(onResultados).toHaveBeenCalledWith([
      expect.objectContaining({ file_name: 'web.container' }),
    ])
  })

  it('busca por name, stack y kind, no solo por file_name', () => {
    const { onResultados } = montar([
      unidad({ name: 'web.service', file_name: 'web.container', stack: 'mcp' }),
      unidad({
        name: 'backup.service',
        file_name: 'backup.volume',
        stack: 'db',
        kind: 'volume',
      }),
    ])

    escribir('web.service')
    expect(identidades(onResultados.mock.lastCall![0] as QuadletUnit[])).toEqual(['web.container'])

    escribir('mcp')
    expect(identidades(onResultados.mock.lastCall![0] as QuadletUnit[])).toEqual(['web.container'])

    escribir('volume')
    expect(identidades(onResultados.mock.lastCall![0] as QuadletUnit[])).toEqual(['backup.volume'])
  })

  it('combina la búsqueda con los filtros de tipo y estado ya existentes', () => {
    const { onResultados } = montar(
      [
        unidad({ name: 'web.service', file_name: 'web.container', active_state: 'active' }),
        unidad({ name: 'web-stop.service', file_name: 'web-stop.container', active_state: 'inactive' }),
        unidad({
          name: 'web-volume.service',
          file_name: 'web-volume.volume',
          kind: 'volume',
          active_state: 'active',
        }),
      ],
      { active_state: 'active', kind: 'container' },
    )

    escribir('web')

    expect(identidades(onResultados.mock.lastCall![0] as QuadletUnit[])).toEqual(['web.container'])
  })

  it('muestra un estado vacío explícito cuando nada coincide', () => {
    const { onResultados } = montar([
      unidad({ name: 'web.service', file_name: 'web.container', stack: 'web' }),
    ])

    escribir('no-existe')

    expect(onResultados).toHaveBeenCalledWith([])
    expect(screen.getByText('No hay unidades que coincidan con los filtros')).toBeInTheDocument()
  })

  it('usa el token de texto secundario con fallback accesible en el estado vacío', () => {
    montar([unidad({ name: 'web.service', file_name: 'web.container', stack: 'web' })])
    escribir('no-existe')

    const mensaje = screen.getByText('No hay unidades que coincidan con los filtros')
    expect(mensaje.getAttribute('style')).toContain('var(--ant-color-text-secondary, #595959)')
    expect(mensaje.getAttribute('style')).not.toContain('#8c8c8c')
    expect(contraste('#595959', '#ffffff')).toBeGreaterThanOrEqual(4.5)
  })

  it('las etiquetas de las tarjetas usan el token de texto secundario y cumplen AA', () => {
    const css = readFileSync(ESTILOS, 'utf8')
    const regla = css.match(/\.podpot-tarjeta__datos dt\s*\{[^}]*\}/)?.[0] ?? ''
    const fallback = /#([0-9a-f]{6})\b/i.exec(regla)?.[1]

    expect(regla).toContain('var(--ant-color-text-secondary')
    expect(regla).not.toContain('#8c8c8c')
    expect(fallback).toBeDefined()
    expect(contraste(`#${fallback!}`, '#ffffff')).toBeGreaterThanOrEqual(4.5)
  })
})
