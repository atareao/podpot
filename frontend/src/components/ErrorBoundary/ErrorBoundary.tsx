import { Component } from 'react'
import type { ErrorInfo, ReactNode } from 'react'

export interface ErrorBoundaryProps {
  children: ReactNode
}

interface ErrorBoundaryState {
  error: Error | null
}

/** Envoltorio que evita que un fallo de render deje el panel en blanco. */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // React ya registra el error; el detalle queda aquí por si se instrumenta.
    void error
    void info
  }

  private reintentar = (): void => {
    this.setState({ error: null })
  }

  render(): ReactNode {
    if (this.state.error !== null) {
      return (
        <div role="alert" style={{ padding: 24 }}>
          <p>No se pudo mostrar esta vista. Puede volver a intentarlo.</p>
          <button type="button" onClick={this.reintentar}>
            Reintentar
          </button>
        </div>
      )
    }
    return this.props.children
  }
}
