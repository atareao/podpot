import { useEffect, useMemo, useState } from 'react'
import type { ActiveState, QuadletUnit, TipoDeQuadlet } from '../../types/api'

export interface FiltrosDeBusqueda {
  kind?: TipoDeQuadlet
  active_state?: ActiveState
}

export interface BarraDeBusquedaProps {
  /** Instantánea de unidades sobre la que se aplican búsqueda y filtros. */
  unidades: QuadletUnit[]
  /** Filtros ya seleccionados en otra parte del panel. */
  filtros?: FiltrosDeBusqueda
  /** Notifica la lista resultante, también al montar. */
  onResultados: (resultados: QuadletUnit[]) => void
}

/** Retardo de la búsqueda: escribir no debe filtrar a cada pulsación. */
const RETARDO_MS = 250

/** Los campos de texto sobre los que busca el cuadro. */
function coinciden(unidad: QuadletUnit, texto: string): boolean {
  if (texto === '') return true
  const campos = [unidad.file_name, unidad.name, unidad.stack, unidad.kind ?? '']
  return campos.some((campo) => campo.toLocaleLowerCase().includes(texto))
}

/**
 * Cuadro de búsqueda con filtros inmediatos: el texto se aplica con debounce,
 * pero `filtros` se refleja en cuanto cambia. El listado resultante se publica
 * hacia el componente que pinta las filas.
 */
export function BarraDeBusqueda({ unidades, filtros, onResultados }: BarraDeBusquedaProps) {
  const [texto, setTexto] = useState('')
  const [textoAplicado, setTextoAplicado] = useState('')

  useEffect(() => {
    const temporizador = window.setTimeout(() => setTextoAplicado(texto), RETARDO_MS)
    return () => window.clearTimeout(temporizador)
  }, [texto])

  const kind = filtros?.kind
  const estado = filtros?.active_state
  const resultados = useMemo(() => {
    const busqueda = textoAplicado.trim().toLocaleLowerCase()
    return unidades.filter(
      (unidad) =>
        (kind === undefined || unidad.kind === kind) &&
        (estado === undefined || unidad.active_state === estado) &&
        coinciden(unidad, busqueda),
    )
  }, [unidades, kind, estado, textoAplicado])

  useEffect(() => {
    onResultados(resultados)
  }, [resultados, onResultados])

  return (
    <div>
      <input
        type="text"
        aria-label="Buscar unidades"
        placeholder="Buscar unidades"
        value={texto}
        onChange={(evento) => setTexto(evento.target.value)}
        style={{ width: '100%', maxWidth: 360, padding: '8px 12px', boxSizing: 'border-box' }}
      />
      {resultados.length === 0 && (
        <p style={{ color: 'var(--ant-color-text-secondary, #595959)', margin: '8px 0 0' }}>
          No hay unidades que coincidan con los filtros
        </p>
      )}
    </div>
  )
}
