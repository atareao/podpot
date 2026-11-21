import { useEffect, useMemo, useState } from 'react'
import { RUTA_BASE } from '../api/client'
import { desenvolver } from '../api/envelope'
import type { ProgresoDeJob } from '../types/api'

/** Canal SSE de progreso de trabajos, en el mismo origen que el resto de la API. */
export const RUTA_DEL_CANAL_DE_TRABAJOS = `${RUTA_BASE}/jobs/stream`

/**
 * Trabajos en curso, por unidad y por id de job: el estado interno del hook.
 *
 * La clave exterior es el **nombre de unidad** (el campo `unit` del evento); la
 * interior, el id del job de systemd, con su tipo de acción (`kind`). Guardar por
 * job —y no solo por unidad— es lo que permite que un `done` dé de baja **su**
 * job sin apagar el indicador mientras queden otros sobre la misma unidad.
 */
type TrabajosPorUnidad = Record<string, Record<number, string>>

/**
 * Tipos de acción en curso por unidad: el mapa derivado que expone el hook.
 *
 * Una unidad sin jobs en curso no aparece en el mapa; el panel la trata como «sin
 * indicador». Cada valor es la lista de `kind` de los jobs vivos de esa unidad.
 */
export type TrabajosEnCurso = Record<string, string[]>

/**
 * Aplica un progreso al estado: `new` da de alta su `jobId → kind`, `done` lo da
 * de baja por `jobId` (y limpia la unidad cuando se queda sin jobs). Devuelve el
 * mismo objeto si nada cambia, para no forzar re-render.
 */
function trasProgreso(actual: TrabajosPorUnidad, progreso: ProgresoDeJob): TrabajosPorUnidad {
  const { unit, job, kind, phase } = progreso
  const deLaUnidad = actual[unit]

  if (phase === 'new') {
    if (deLaUnidad?.[job] === kind) return actual
    return { ...actual, [unit]: { ...(deLaUnidad ?? {}), [job]: kind } }
  }

  // `done`: se da de baja **ese** job, no toda la unidad.
  if (deLaUnidad === undefined || !(job in deLaUnidad)) return actual

  const restantes = { ...deLaUnidad }
  delete restantes[job]

  if (Object.keys(restantes).length === 0) {
    const sinLaUnidad = { ...actual }
    delete sinLaUnidad[unit]
    return sinLaUnidad
  }
  return { ...actual, [unit]: restantes }
}

/** Aplana el estado por job al mapa de `kind`s en curso por unidad. */
function aKindsPorUnidad(estado: TrabajosPorUnidad): TrabajosEnCurso {
  const porUnidad: TrabajosEnCurso = {}
  for (const [unit, jobs] of Object.entries(estado)) {
    porUnidad[unit] = Object.values(jobs)
  }
  return porUnidad
}

/**
 * Mantiene, en vivo, el progreso de las acciones por unidad.
 *
 * Abre una suscripción `EventSource` al canal SSE de trabajos y, por cada evento
 * `job` —un evento **con nombre**, de ahí `addEventListener` y no `onmessage`—,
 * desenvuelve su `{data}` y actualiza el estado por job: alta al `new`, baja por
 * `jobId` al `done`. Expone, por unidad, los `kind` de sus jobs vivos.
 *
 * La suscripción se cierra al desmontar. La reconexión es comportamiento **nativo
 * de `EventSource`** (reintenta por su cuenta, guiado por el `retry:` que manda el
 * servidor); el doble de pruebas no lo modela, pero el hook no añade lógica para
 * ello. Si el canal no está disponible, el panel sigue con su comportamiento
 * actual: este hook solo aporta el indicador, que simplemente no aparecerá. Un
 * evento ilegible se ignora —no tumba la vista—.
 */
export function useTrabajosEnVivo(): TrabajosEnCurso {
  const [trabajos, setTrabajos] = useState<TrabajosPorUnidad>({})

  useEffect(() => {
    const fuente = new EventSource(RUTA_DEL_CANAL_DE_TRABAJOS)

    const alRecibirProgreso = (evento: MessageEvent<string>): void => {
      let progreso: ProgresoDeJob
      try {
        progreso = desenvolver<ProgresoDeJob>({
          http: 200,
          cuerpo: JSON.parse(evento.data) as unknown,
        })
      } catch {
        // Un evento que no cumple el contrato se descarta: el estado que ya hay se
        // queda como está hasta el siguiente evento válido.
        return
      }
      setTrabajos((actual) => trasProgreso(actual, progreso))
    }

    // El canal emite un evento con nombre (`job`), no el `message` por defecto.
    fuente.addEventListener('job', alRecibirProgreso as EventListener)

    return () => {
      fuente.close()
    }
  }, [])

  // El derivado solo se recalcula cuando cambia el estado (nunca en un re-render
  // por otra causa): mantiene la referencia estable cuando nada cambió.
  return useMemo(() => aKindsPorUnidad(trabajos), [trabajos])
}
