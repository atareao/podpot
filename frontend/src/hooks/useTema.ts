import { useCallback, useEffect, useRef, useState } from 'react'

export type Tema = 'light' | 'dark'

export interface UsoDeTema {
  tema: Tema
  establecerTema: (tema: Tema) => void
  alternarTema: () => void
}

const CLAVE = 'podpot-tema'

function esEleccion(valor: string | null): valor is Tema {
  return valor === 'light' || valor === 'dark'
}

function temaInicial(): Tema {
  const guardado = window.localStorage.getItem(CLAVE)
  if (esEleccion(guardado)) return guardado
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

/** Tema claro/oscuro con elección persistida; sin elección sigue al sistema. */
export function useTema(): UsoDeTema {
  const [tema, setTema] = useState<Tema>(temaInicial)
  // Referencia al valor vigente: permite alternar sin efectos ni STATE extra.
  const actual = useRef(tema)

  useEffect(() => {
    const consulta = window.matchMedia('(prefers-color-scheme: dark)')
    const alCambiar = (evento: MediaQueryListEvent): void => {
      // Una elección guardada gana siempre, incluso si cambió durante este mount.
      if (esEleccion(window.localStorage.getItem(CLAVE))) return
      const nuevo: Tema = evento.matches ? 'dark' : 'light'
      actual.current = nuevo
      setTema(nuevo)
    }
    consulta.addEventListener('change', alCambiar)
    return () => consulta.removeEventListener('change', alCambiar)
  }, [])

  const establecerTema = useCallback((nuevo: Tema): void => {
    actual.current = nuevo
    window.localStorage.setItem(CLAVE, nuevo)
    setTema(nuevo)
  }, [])

  const alternarTema = useCallback((): void => {
    establecerTema(actual.current === 'dark' ? 'light' : 'dark')
  }, [establecerTema])

  return { tema, establecerTema, alternarTema }
}
