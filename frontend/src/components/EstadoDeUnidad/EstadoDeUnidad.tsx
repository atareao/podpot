import { Badge } from 'antd'
import type { ActiveState } from '../../types/api'

type EstadoDeBadge = 'success' | 'processing' | 'default' | 'error' | 'warning'

interface DescripcionDeEstado {
  status: EstadoDeBadge
  etiqueta: string
}

const ESTADOS: Record<ActiveState, DescripcionDeEstado> = {
  active: { status: 'success', etiqueta: 'En ejecución' },
  activating: { status: 'processing', etiqueta: 'Arrancando' },
  reloading: { status: 'processing', etiqueta: 'Recargando' },
  deactivating: { status: 'processing', etiqueta: 'Parando' },
  inactive: { status: 'default', etiqueta: 'Parada' },
  failed: { status: 'error', etiqueta: 'Fallida' },
  maintenance: { status: 'warning', etiqueta: 'Mantenimiento' },
}

export interface EstadoDeUnidadProps {
  /** `active_state` de la unidad. */
  estado: string
}

/**
 * El estado de una quadlet como etiqueta de color. Un valor que no conozca el
 * panel se muestra tal cual: mejor un texto sin traducir que un hueco en blanco.
 */
export function EstadoDeUnidad({ estado }: EstadoDeUnidadProps) {
  const descripcion =
    ESTADOS[estado as ActiveState] ?? { status: 'default' as const, etiqueta: estado }
  return <Badge status={descripcion.status} text={descripcion.etiqueta} />
}
