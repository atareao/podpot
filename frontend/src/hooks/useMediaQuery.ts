import { useCallback, useMemo, useSyncExternalStore } from 'react'

/** Devuelve si la consulta CSS coincide y sigue sus cambios mientras esté montado. */
export function useMediaQuery(consulta: string): boolean {
  const lista = useMemo(() => window.matchMedia(consulta), [consulta])

  const suscribirse = useCallback(
    (notificar: () => void) => {
      lista.addEventListener('change', notificar)
      return () => lista.removeEventListener('change', notificar)
    },
    [lista],
  )
  const obtenerInstantanea = useCallback(() => lista.matches, [lista])

  return useSyncExternalStore(suscribirse, obtenerInstantanea, () => false)
}
