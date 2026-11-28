/**
 * Contrato que fijan estos tests para GREEN:
 * - `ErrorBoundary` envuelve `children` y muestra un mensaje de recuperación con
 *   un botón «Reintentar» cuando un hijo lanza durante el render.
 */

import type { ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ErrorBoundary } from './ErrorBoundary'

function RompeTodo(): ReactNode {
  throw new Error('fallo de renderizado')
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('ErrorBoundary', () => {
  it('muestra un mensaje de recuperación en lugar de una pantalla vacía', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})

    render(
      <ErrorBoundary>
        <RompeTodo />
      </ErrorBoundary>,
    )

    expect(screen.getByText(/no se pudo mostrar|algo salió mal|se produjo un error/i)).toBeInTheDocument()
    expect(document.body.textContent?.trim()).not.toBe('')
  })

  it('ofrece un botón de reintento que vuelve a montar a los hijos', async () => {
    const usuario = userEvent.setup()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const vista = render(
      <ErrorBoundary>
        <RompeTodo />
      </ErrorBoundary>,
    )

    expect(screen.getByRole('button', { name: /reintentar/i })).toBeInTheDocument()
    vista.rerender(
      <ErrorBoundary>
        <span>Panel recuperado</span>
      </ErrorBoundary>,
    )
    //tras el reintento, el contenido vuelve a pintarse
    await usuario.click(screen.getByRole('button', { name: /reintentar/i }))
    expect(screen.getByText('Panel recuperado')).toBeInTheDocument()
  })
})
