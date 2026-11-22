import { Button, Empty, Popconfirm, Space, Spin, Table, Tag, Typography } from 'antd'
import type { TableProps } from 'antd'
import { EstadoDeUnidad } from '../EstadoDeUnidad/EstadoDeUnidad'
import { etiquetaDeAccionEnCurso } from '../../acciones'
import type { TrabajosEnCurso } from '../../hooks/useTrabajosEnVivo'
import type { AccionDeUnidad, QuadletUnit } from '../../types/api'

export interface TablaDeUnidadesProps {
  /** Quadlets a listar. */
  unidades: QuadletUnit[]
  /** Mientras se pide la lista. */
  cargando: boolean
  /** Acción solicitada desde una fila. */
  onAccion: (unidad: QuadletUnit, accion: AccionDeUnidad) => void
  /** Edición solicitada desde una fila. */
  onEditar: (unidad: QuadletUnit) => void
  /** Apertura del visor de logs solicitada desde una fila. */
  onVerRegistros: (unidad: QuadletUnit) => void
  /**
   * Borrado **confirmado** desde una fila: la confirmación vive aquí, así que
   * cancelarla no llega a emitir nada.
   */
  onBorrar: (unidad: QuadletUnit) => void
  /** Nombre de la unidad con una acción en curso: sus botones se deshabilitan. */
  enCurso?: string | null
  /**
   * Tipos de acción de los jobs en curso por unidad (nombre de unidad → `kind`s),
   * del canal de trabajos. Una unidad presente aquí, con al menos un `kind`,
   * muestra el indicador «en curso». Solo pinta: la acción sigue por su sitio
   * (API + notificación). El tipo lo define el hook (`TrabajosEnCurso`), fuente
   * única de la forma.
   */
  progresoPorUnidad?: TrabajosEnCurso
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
 * Identidad con la que se muestra y se nombra una quadlet: su `file_name`
 * publicado por la API (la clave real de `PATCH`/`DELETE`), no el nombre de
 * unidad systemd. Si la API no lo publica —nulo o vacío—, cae al nombre de
 * unidad para no dejar la fila sin identificar ni los botones sin nombre
 * accesible.
 */
function identidadDe(unidad: QuadletUnit): string {
  return unidad.file_name || unidad.name
}

/**
 * Listado de quadlets: una fila por unidad, con su estado y las acciones de
 * control. Presentacional: no habla con la API ni con React Query —quien la usa
 * (`pages/unidades.tsx`) es el que conecta—.
 *
 * La fila se identifica por el `file_name` publicado por la API (su identidad
 * real), con el nombre de unidad systemd como dato secundario. Cada botón lleva
 * un `aria-label` con esa identidad: hay uno por fila y, sin él, todos los
 * «Arrancar» comparten nombre accesible.
 */
export function TablaDeUnidades({
  unidades,
  cargando,
  onAccion,
  onEditar,
  onVerRegistros,
  onBorrar,
  enCurso = null,
  progresoPorUnidad = {},
}: TablaDeUnidadesProps) {
  const columnas: TableProps<QuadletUnit>['columns'] = [
    {
      title: 'Unidad',
      key: 'unidad',
      render: (_valor, unidad) => (
        <Space direction="vertical" size={0}>
          {/* El `file_name` es la identidad primaria de la fila… */}
          <span>{identidadDe(unidad)}</span>
          {/* …y el nombre de unidad systemd, el dato secundario. */}
          <Typography.Text type="secondary">{unidad.name}</Typography.Text>
        </Space>
      ),
    },
    {
      title: 'Estado',
      key: 'estado',
      render: (_valor, unidad) => <EstadoDeUnidad estado={unidad.active_state} />,
    },
    {
      title: 'Progreso',
      key: 'progreso',
      render: (_valor, unidad) => {
        const kinds = progresoPorUnidad[unidad.name]
        if (kinds === undefined || kinds.length === 0) return null
        const etiquetas = kinds.map(etiquetaDeAccionEnCurso)
        // Indicador aditivo: mientras haya jobs para la unidad se ve un spinner
        // con sus tipos de acción, en español. Es un `<div role="status">` (no un
        // `<span>`): contiene el `Space`, que rinde un elemento de flujo. El
        // `aria-label` da el nombre accesible completo, así que el `Spin` se marca
        // decorativo con `aria-hidden` para no anunciar dos veces la región.
        return (
          <div
            role="status"
            aria-label={`Acción en curso en ${identidadDe(unidad)}: ${etiquetas.join(', ')}`}
          >
            <Space size="small" wrap>
              <Spin size="small" aria-hidden="true" />
              {kinds.map((kind, indice) => (
                <Tag color="processing" key={`${kind}-${indice}`}>
                  {etiquetaDeAccionEnCurso(kind)}
                </Tag>
              ))}
            </Space>
          </div>
        )
      },
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
        // Las claves internas siguen por **nombre de unidad**: es lo que viaja en
        // la acción HTTP (`{ name }`) y en el campo `unit` del SSE de jobs. El
        // `aria-label`, en cambio, usa la identidad mostrada (`file_name`).
        const bloqueada = enCurso === unidad.name
        const identidad = identidadDe(unidad)
        return (
          <Space size="small" wrap>
            <Button
              size="small"
              aria-label={`Arrancar ${identidad}`}
              disabled={bloqueada || EN_MARCHA.has(unidad.active_state)}
              onClick={() => onAccion(unidad, 'start')}
            >
              Arrancar
            </Button>
            <Button
              size="small"
              aria-label={`Parar ${identidad}`}
              disabled={bloqueada || PARADA.has(unidad.active_state)}
              onClick={() => onAccion(unidad, 'stop')}
            >
              Parar
            </Button>
            <Button
              size="small"
              aria-label={`Reiniciar ${identidad}`}
              disabled={bloqueada}
              onClick={() => onAccion(unidad, 'restart')}
            >
              Reiniciar
            </Button>
            <Button
              size="small"
              aria-label={`Editar ${identidad}`}
              disabled={bloqueada}
              onClick={() => onEditar(unidad)}
            >
              Editar
            </Button>
            {/* Los logs solo se leen: no se bloquean con una operación en curso
                sobre la fila (no encadenan nada destructivo). */}
            <Button
              size="small"
              aria-label={`Logs ${identidad}`}
              onClick={() => onVerRegistros(unidad)}
            >
              Logs
            </Button>
            {/* El borrado es destructivo: la confirmación vive aquí y solo al
                confirmarla se emite `onBorrar`; cancelar no sale de la fila.
                La confirmación nombra el fichero real (identidad publicada por
                la API): el nombre de unidad no basta para identificarlo. */}
            <Popconfirm
              title={`¿Borrar la quadlet «${unidad.name}»?`}
              description={`Se borrará el fichero «${unidad.file_name}». No se puede deshacer.`}
              okText="Sí, borrar"
              cancelText="Cancelar"
              okButtonProps={{ danger: true }}
              onConfirm={() => onBorrar(unidad)}
            >
              <Button
                size="small"
                danger
                aria-label={`Borrar ${identidad}`}
                disabled={bloqueada}
              >
                Borrar
              </Button>
            </Popconfirm>
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
      // El `file_name` es la identidad única de la quadlet: dos filas pueden
      // compartir nombre de unidad (colisión no inyectiva), pero nunca fichero.
      rowKey="file_name"
      columns={columnas}
      dataSource={unidades}
      loading={cargando}
      pagination={false}
      scroll={{ x: 'max-content' }}
      locale={{ emptyText: <Empty description="No hay quadlets" /> }}
    />
  )
}
