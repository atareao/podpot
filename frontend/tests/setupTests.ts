// Los matchers de jest-dom (`toBeInTheDocument`, …) se registran en `expect` de
// Vitest: la entrada `/vitest` es la que declara la augmentación de tipos (la
// raíz del paquete augmenta el `expect` de Jest y no compila aquí).
import '@testing-library/jest-dom/vitest'
import { EventSourceFalso } from './eventSourceFalso'

// jsdom no implementa `EventSource` y el panel abre una suscripción SSE al
// montar: sin este doble, cualquier prueba que pinte la pantalla fallaría al
// construir la suscripción. Las pruebas del canal lo inspeccionan a través de
// `EventSourceFalso.instancias`.
if (typeof globalThis.EventSource === 'undefined') {
  globalThis.EventSource = EventSourceFalso as unknown as typeof EventSource
}

// jsdom no implementa `window.matchMedia`, y Ant Design lo consulta para sus
// puntos de ruptura responsive. Este doble evalúa las consultas `min-width`/
// `max-width` contra `window.innerWidth`.
if (typeof window !== 'undefined' && typeof window.matchMedia !== 'function') {
  window.matchMedia = ((consulta: string) => {
    const maximo = /max-width:\s*([\d.]+)px/.exec(consulta)
    const minimo = /min-width:\s*([\d.]+)px/.exec(consulta)

    const evaluar = (): boolean => {
      let casa = true
      if (maximo !== null) casa = casa && window.innerWidth <= Number(maximo[1])
      if (minimo !== null) casa = casa && window.innerWidth >= Number(minimo[1])
      return casa
    }

    const oyentes = new Set<(evento: MediaQueryListEvent) => void>()
    const lista = {
      get matches() {
        return evaluar()
      },
      media: consulta,
      onchange: null,
      addEventListener: (_tipo: string, oyente: (evento: MediaQueryListEvent) => void) => {
        oyentes.add(oyente)
      },
      removeEventListener: (_tipo: string, oyente: (evento: MediaQueryListEvent) => void) => {
        oyentes.delete(oyente)
      },
      addListener: (oyente: (evento: MediaQueryListEvent) => void) => {
        oyentes.add(oyente)
      },
      removeListener: (oyente: (evento: MediaQueryListEvent) => void) => {
        oyentes.delete(oyente)
      },
      dispatchEvent: () => false,
    }
    return lista as unknown as MediaQueryList
  }) as typeof window.matchMedia
}
