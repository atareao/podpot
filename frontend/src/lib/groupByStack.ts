import type { QuadletUnit, TipoDeQuadlet } from '../types/api'

/** Un grupo de unidades que comparten el `stack` que publica el backend. */
export interface GrupoDeStack {
  stack: string
  unidades: QuadletUnit[]
}

/**
 * Los tipos auxiliares van después de los principales. `null` (extensión no
 * reconocida por el panel) también se considera auxiliar: la tabla sigue siendo
 * operable con unidades que el backend no sabe clasificar.
 */
const AUXILIARES: ReadonlySet<TipoDeQuadlet | null> = new Set(['volume', 'network', null])

/** Comparación determinista, sin depender del locale del navegador. */
function compararTextos(a: string, b: string): number {
  const izquierda = a.toLowerCase()
  const derecha = b.toLowerCase()
  if (izquierda < derecha) return -1
  if (izquierda > derecha) return 1
  return a < b ? -1 : a > b ? 1 : 0
}

function compararUnidades(a: QuadletUnit, b: QuadletUnit): number {
  const prioridad = Number(AUXILIARES.has(a.kind)) - Number(AUXILIARES.has(b.kind))
  return (
    prioridad ||
    compararTextos(a.name, b.name) ||
    compararTextos(a.file_name, b.file_name)
  )
}

/**
 * Agrupa por el campo `stack` recibido: no deduce nada del nombre de la unidad.
 * Ordena los grupos por nombre (case-insensitive) y, dentro, principales antes
 * que auxiliares y luego por nombre. No muta la entrada ni reordena las filas
 * cuando cambia el estado: solo `stack`, `kind` y `name` intervienen en el orden.
 */
export function groupByStack(unidades: QuadletUnit[]): GrupoDeStack[] {
  const porStack = new Map<string, QuadletUnit[]>()

  for (const unidad of unidades) {
    const stack = unidad.stack ?? ''
    const grupo = porStack.get(stack)
    if (grupo === undefined) porStack.set(stack, [unidad])
    else grupo.push(unidad)
  }

  return [...porStack.entries()]
    .sort(([a], [b]) => compararTextos(a, b))
    .map(([stack, grupo]) => ({ stack, unidades: [...grupo].sort(compararUnidades) }))
}
