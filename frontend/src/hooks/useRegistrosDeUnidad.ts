import { startTransition, useEffect, useRef, useState } from 'react'
import { RUTA_BASE } from '../api/client'
import { desenvolver } from '../api/envelope'
import type { LineaDeRegistro } from '../types/api'

/**
 * Canal SSE de logs de una unidad concreta.
 *
 * `{name}` es el nombre **completo** de la unidad (p. ej. `centinela-db.service`),
 * tal y como lo publica el listado (`unit.name`). Se codifica porque el nombre
 * viaja en un segmento de la ruta.
 */
export function rutaDeRegistros(nombre: string): string {
  return `${RUTA_BASE}/units/${encodeURIComponent(nombre)}/logs/stream`
}

/**
 * Tope de líneas conservadas en el visor. Un contenedor parlanchón no debe
 * crecer sin límite: al superarlo se descartan las más antiguas.
 */
export const MAXIMO_DE_LINEAS = 2000

/**
 * Cada cuánto se vuelca el buffer acumulado en el estado. Las líneas que llegan
 * entre volcados se consolidan en **un solo** `setState` (un re-render por lote,
 * no por línea).
 */
export const INTERVALO_DE_VOLCADO_MS = 150

/**
 * Una línea tal y como la pinta el visor: el contenido del canal más una clave
 * estable para las listas (`clave`), que sobrevive al recorte del tope.
 */
export interface LineaEnVisor extends LineaDeRegistro {
  /** Secuencia incremental: clave única y estable de la fila. */
  clave: number
}

/** Estado que expone el hook: las líneas acumuladas y el aviso de error, si lo hay. */
export interface EstadoDeRegistros {
  /** Líneas recibidas desde que se abrió el visor, en orden de llegada. */
  lineas: LineaEnVisor[]
  /**
   * Mensaje cuando el canal falla, o `null` si todo va bien. La vista lo enseña
   * como aviso sin romper el resto del panel.
   */
  error: string | null
}

/** Un payload que cumple el contrato del evento `log`. */
function esLineaDeRegistro(valor: unknown): valor is LineaDeRegistro {
  if (typeof valor !== 'object' || valor === null) return false
  const linea = valor as Record<string, unknown>
  return (
    typeof linea.line === 'string' && (linea.stream === 'stdout' || linea.stream === 'stderr')
  )
}

/**
 * Mantiene los logs de una unidad mientras su visor está abierto.
 *
 * Con `nombre` no nulo abre una suscripción `EventSource` al canal SSE y, por
 * cada evento `log` —un evento **con nombre**, de ahí `addEventListener` y no
 * `onmessage`—, desenvuelve su `{data:{line,stream}}` y **encola** la línea. El
 * buffer vive en un `useRef`, no en el estado: así un contenedor parlanchón no
 * dispara un re-render (ni una copia O(n) del array) por línea. El lote se
 * vuelca en un solo `setState` cada `INTERVALO_DE_VOLCADO_MS`, dentro de una
 * `startTransition` para no bloquear la interacción.
 *
 * Con `nombre` nulo no hay suscripción: los logs solo interesan con el visor
 * abierto, así que el canal no se multiplica por unidad.
 *
 * Un evento ilegible se descarta —no tumba el visor— y un fallo del canal se
 * expone en `error`; ese aviso se limpia al reconectar (`open`), no solo al
 * llegar una línea, porque `EventSource` reintenta solo las caídas transitorias.
 * La suscripción se cierra en el cleanup: al cerrar el visor o al desmontar. Al
 * cambiar de unidad (o al cerrarla) se vacía lo acumulado, que era de otra
 * suscripción.
 */
export function useRegistrosDeUnidad(nombre: string | null): EstadoDeRegistros {
  const [lineas, setLineas] = useState<LineaEnVisor[]>([])
  const [error, setError] = useState<string | null>(null)

  // Líneas recibidas y aún sin volcar: fuera del estado, sin re-render por línea.
  const pendientes = useRef<LineaEnVisor[]>([])
  // Secuencia incremental: clave estable de cada fila, para que el recorte del
  // tope no reutilice nodos del DOM con otro texto.
  const secuencia = useRef(0)
  // Temporizador del volcado por lote, si hay uno programado.
  const volcado = useRef<number | null>(null)

  // Unidad a la que pertenece lo acumulado. Al cambiar de unidad (o al cerrar el
  // visor), lo que había era de otra suscripción y no vale: se ajusta el estado
  // en el propio render —el patrón que React recomienda para «resetear estado
  // cuando cambia una prop»— en vez de en un efecto, que provocaría renders en
  // cascada.
  const [nombreActual, setNombreActual] = useState<string | null>(nombre)
  if (nombreActual !== nombre) {
    setNombreActual(nombre)
    setLineas([])
    setError(null)
  }

  useEffect(() => {
    const cancelarVolcado = (): void => {
      if (volcado.current !== null) {
        clearTimeout(volcado.current)
        volcado.current = null
      }
    }

    // Cada suscripción arranca de cero: lo pendiente era de la unidad anterior.
    pendientes.current = []
    secuencia.current = 0
    cancelarVolcado()

    // Sin visor abierto no hay canal: los logs solo interesan mientras se ven.
    if (nombre === null) return

    const volcar = (): void => {
      volcado.current = null
      const lote = pendientes.current
      if (lote.length === 0) return
      pendientes.current = []
      // Un solo `set` por lote, y en transición: pintar muchas líneas de golpe no
      // bloquea cerrar el visor ni cambiar de unidad.
      startTransition(() => {
        setLineas((actuales) => {
          const unidas = actuales.length === 0 ? lote : [...actuales, ...lote]
          return unidas.length > MAXIMO_DE_LINEAS ? unidas.slice(-MAXIMO_DE_LINEAS) : unidas
        })
        // Llegaron líneas: el canal está vivo y el aviso anterior ya no aplica.
        setError(null)
      })
    }

    const programarVolcado = (): void => {
      if (volcado.current !== null) return
      volcado.current = window.setTimeout(volcar, INTERVALO_DE_VOLCADO_MS)
    }

    const fuente = new EventSource(rutaDeRegistros(nombre))

    const alRecibirLinea = (evento: MessageEvent<string>): void => {
      let linea: unknown
      try {
        linea = desenvolver<unknown>({ http: 200, cuerpo: JSON.parse(evento.data) as unknown })
      } catch {
        // Un evento que no cumple el contrato se descarta: el visor sigue.
        return
      }
      if (!esLineaDeRegistro(linea)) return
      pendientes.current.push({ ...linea, clave: secuencia.current++ })
      programarVolcado()
    }

    const alFallar = (): void => {
      setError('No se pudieron leer los registros de la unidad.')
    }

    const alAbrir = (): void => {
      // `open` llega también tras reconectar: el aviso de la caída transitoria sobra.
      setError(null)
    }

    // El canal emite un evento con nombre (`log`), no el `message` por defecto.
    fuente.addEventListener('log', alRecibirLinea as EventListener)
    // `error` es el evento estándar de `EventSource`: el canal no está disponible.
    fuente.addEventListener('error', alFallar as EventListener)
    // `open` marca la conexión (re)establecida.
    fuente.addEventListener('open', alAbrir as EventListener)

    return () => {
      fuente.close()
      cancelarVolcado()
    }
  }, [nombre])

  return { lineas, error }
}
