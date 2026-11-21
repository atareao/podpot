import { Button, Empty, Space, Table } from 'antd'
import type { TableProps } from 'antd'
import { EstadoDeUnidad } from '../EstadoDeUnidad/EstadoDeUnidad'
import type { AccionDeUnidad, QuadletUnit } from '../../types/api'

export interface TablaDeUnidadesProps {
  /** Quadlets a listar. */
  unidades: QuadletUnit[]
  /** Mientras se pide la lista. */
  cargando: boolean
  /** Acción solicitada desde una fila. */
  onAccion: (unidad: QuadletUnit, accion: AccionDeUnidad) => void
  /** Nombre de la unidad con una acción en curso: sus botones se deshabilitan. */
  enCurso?: string | null
}

/**
 * Estados intermedios: aunque `ActiveState` no sea `active`/`inactive`, una
 * unidad que se está activando (`activating`, `reloading`) no se puede arrancar
 * otra vez, y una que se está desactivando (`deactivating`) no se puede parar.
 * Es una unión abierta a propósito: un estado nuevo de systemd cae del lado
 * permisivo en vez de romper la tabla.
 */
const EN_MARCHA: ReadonlySet<string> = new Set(['active', 'activating', 'reloading'])
const PARADA: ReadonlySet<string> = new Set(['inactive', 'deactivating'])

/**
 * Listado de quadlets: una fila por unidad, con su estado y las acciones de
 * control. Presentacional: no habla con la API ni con React Query —quien la usa
 * (`pages/unidades.tsx`) es el que conecta—.
 *
 * Cada botón lleva un `aria-label` con el nombre de la unidad: hay uno por fila
 * y, sin él, todos los «Arrancar» comparten nombre accesible.
 */
export function TablaDeUnidades({
  unidades,
  cargando,
  onAccion,
  enCurso = null,
}: TablaDeUnidadesProps) {
  const columnas: TableProps<QuadletUnit>['columns'] = [
    {
      title: 'Unidad',
      dataIndex: 'name',
      key: 'unidad',
    },
    {
      title: 'Estado',
      key: 'estado',
      render: (_valor, unidad) => <EstadoDeUnidad estado={unidad.active_state} />,
    },
    {
      title: 'Sub-estado',
      dataIndex: 'sub_state',
      key: 'sub-estado',
    },
    {
      title: 'Fichero',
      dataIndex: 'unit_file_state',
      key: 'fichero',
    },
    {
      title: 'Acciones',
      key: 'acciones',
      render: (_valor, unidad) => {
        const bloqueada = enCurso === unidad.name
        return (
          <Space size="small" wrap>
            <Button
              size="small"
              aria-label={`Arrancar ${unidad.name}`}
              disabled={bloqueada || EN_MARCHA.has(unidad.active_state)}
              onClick={() => onAccion(unidad, 'start')}
            >
              Arrancar
            </Button>
            <Button
              size="small"
              aria-label={`Parar ${unidad.name}`}
              disabled={bloqueada || PARADA.has(unidad.active_state)}
              onClick={() => onAccion(unidad, 'stop')}
            >
              Parar
            </Button>
            <Button
              size="small"
              aria-label={`Reiniciar ${unidad.name}`}
              disabled={bloqueada}
              onClick={() => onAccion(unidad, 'restart')}
            >
              Reiniciar
            </Button>
          </Space>
        )
      },
    },
  ]

  // `aria-label` llega al `<table>` real: antd lo reenvía a rc-table, que lo
  // esparce sobre el elemento (`pickAttrs(props, { aria: true })`).
  return (
    <Table<QuadletUnit>
      aria-label="Quadlets"
      rowKey="name"
      columns={columnas}
      dataSource={unidades}
      loading={cargando}
      pagination={false}
      scroll={{ x: 'max-content' }}
      locale={{ emptyText: <Empty description="No hay quadlets" /> }}
    />
  )
}
