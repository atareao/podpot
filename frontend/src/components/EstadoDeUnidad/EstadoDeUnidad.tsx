import { Badge } from 'antd'
import type { ActiveState } from '../../types/api'

interface DescripcionDeEstado {
  status: 'success' | 'default' | 'error'
  etiqueta: string
}

const ESTADOS: Record<ActiveState, DescripcionDeEstado> = {
  active: { status: 'success', etiqueta: 'En ejecución' },
  inactive: { status: 'default', etiqueta: 'Parada' },
  failed: { status: 'error', etiqueta: 'Fallida' },
}

export interface EstadoDeUnidadProps {
  /** `active_state` de la unidad. */
  estado: string
}

/**
 * El estado de una quadlet como etiqueta de color. Un valor que no esté en el
 * contrato se muestra tal cual: mejor un texto sin traducir que un hueco en
 * blanco.
 */
export function EstadoDeUnidad({ estado }: EstadoDeUnidadProps) {
  const descripcion = ESTADOS[estado as ActiveState] ?? { status: 'default' as const, etiqueta: estado }
  return <Badge status={descripcion.status} text={descripcion.etiqueta} />
}
