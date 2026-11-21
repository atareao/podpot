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
 * Listado de quadlets: una fila por unidad, con su estado y las acciones de
 * control. Presentacional: no habla con la API ni con React Query —quien la usa
 * (`pages/unidades.tsx`) es el que conecta—.
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
              disabled={bloqueada || unidad.active_state === 'active'}
              onClick={() => onAccion(unidad, 'start')}
            >
              Arrancar
            </Button>
            <Button
              size="small"
              disabled={bloqueada || unidad.active_state === 'inactive'}
              onClick={() => onAccion(unidad, 'stop')}
            >
              Parar
            </Button>
            <Button size="small" disabled={bloqueada} onClick={() => onAccion(unidad, 'restart')}>
              Reiniciar
            </Button>
          </Space>
        )
      },
    },
  ]

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
