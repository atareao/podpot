import { useMemo, useState } from 'react'
import { Button, Empty, Popconfirm, Select, Space, Spin, Table, Tag, Tooltip, Typography } from 'antd'
import type { TableProps } from 'antd'
import { EstadoDeUnidad } from '../EstadoDeUnidad/EstadoDeUnidad'
import { etiquetaDeAccionEnCurso } from '../../acciones'
import { etiquetaDeAccion, iconoYAccion } from '../../acciones-icono'
import { identidadDe } from '../../identidad'
import { groupByStack } from '../../lib/groupByStack'
import type { TrabajosEnCurso } from '../../hooks/useTrabajosEnVivo'
import type {
  AccionDeUnidad,
  QuadletUnit,
  TipoDeQuadlet,
} from '../../types/api'

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
  const [tipo, setTipo] = useState<TipoDeQuadlet | null>(null)
  const [estado, setEstado] = useState<string | null>(null)

  // Derivación pura del estado actual: al llegar un evento SSE se recalcula con
  // el mismo orden determinista y los filtros siguen aplicados. Los grupos sin
  // coincidencias sencillamente no producen filas.
  const { filas, celdasDeStack } = useMemo(() => {
    const grupos = groupByStack(
      unidades.filter(
        (unidad) =>
          (tipo === null || unidad.kind === tipo) &&
          (estado === null || unidad.active_state === estado),
      ),
    )
    const celdas = new Map<number, number>()
    const datos: QuadletUnit[] = []
    for (const grupo of grupos) {
      // El primer renglón del grupo muestra la cabecera con `rowSpan` de todas
      // sus filas: la cabecera informa del stack, cada fila conserva sus datos.
      celdas.set(datos.length, grupo.unidades.length)
      datos.push(...grupo.unidades)
    }
    return { filas: datos, celdasDeStack: celdas }
  }, [unidades, tipo, estado])

  const { tiposDisponibles, estadosDisponibles } = useMemo(
    () => ({
      tiposDisponibles: [...new Set(unidades.flatMap((unidad) => (unidad.kind === null ? [] : [unidad.kind])))].sort(),
      estadosDisponibles: [...new Set(unidades.map((unidad) => unidad.active_state))].sort(),
    }),
    [unidades],
  )

  const columnas: TableProps<QuadletUnit>['columns'] = [
    {
      title: 'Stack',
      key: 'stack',
      onCell: (_unidad, indice) => {
        const tamano = celdasDeStack.get(indice ?? -1)
        return tamano === undefined ? { rowSpan: 0 } : { rowSpan: tamano }
      },
      render: (_valor, unidad) => (
        <Typography.Text strong aria-label={`Stack ${unidad.stack}`}>
          {unidad.stack}
        </Typography.Text>
      ),
    },
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
        const { nombre: arrancar, icono: iconoArrancar } = iconoYAccion('start')
        const { nombre: parar, icono: iconoParar } = iconoYAccion('stop')
        const { nombre: reiniciar, icono: iconoReiniciar } = iconoYAccion('restart')
        const { nombre: editar, icono: iconoEditar } = iconoYAccion('edit')
        const { nombre: logs, icono: iconoLogs } = iconoYAccion('logs')
        const { nombre: borrar, icono: iconoBorrar } = iconoYAccion('borrar')
        return (
          <Space size="small" wrap>
            <Tooltip title={etiquetaDeAccion('start')}>
              <Button
                size="small"
                aria-label={arrancar(identidad)}
                disabled={bloqueada || EN_MARCHA.has(unidad.active_state)}
                onClick={() => onAccion(unidad, 'start')}
              >
                {iconoArrancar}
              </Button>
            </Tooltip>
            <Tooltip title={etiquetaDeAccion('stop')}>
              <Button
                size="small"
                aria-label={parar(identidad)}
                disabled={bloqueada || PARADA.has(unidad.active_state)}
                onClick={() => onAccion(unidad, 'stop')}
              >
                {iconoParar}
              </Button>
            </Tooltip>
            <Tooltip title={etiquetaDeAccion('restart')}>
              <Button
                size="small"
                aria-label={reiniciar(identidad)}
                disabled={bloqueada}
                onClick={() => onAccion(unidad, 'restart')}
              >
                {iconoReiniciar}
              </Button>
            </Tooltip>
            <Tooltip title={etiquetaDeAccion('edit')}>
              <Button
                size="small"
                aria-label={editar(identidad)}
                disabled={bloqueada}
                onClick={() => onEditar(unidad)}
              >
                {iconoEditar}
              </Button>
            </Tooltip>
            {/* Los logs solo se leen: no se bloquean con una operación en curso
                sobre la fila (no encadenan nada destructivo). */}
            <Tooltip title={etiquetaDeAccion('logs')}>
              <Button size="small" aria-label={logs(identidad)} onClick={() => onVerRegistros(unidad)}>
                {iconoLogs}
              </Button>
            </Tooltip>
            {/* El borrado es destructivo: la confirmación vive aquí y solo al
                confirmarla se emite `onBorrar`; cancelar no sale de la fila.
                La confirmación nombra el fichero real (identidad publicada por
                la API): el nombre de unidad no basta para identificarlo. */}
            <Popconfirm
              title={`¿Borrar la quadlet «${identidadDe(unidad)}»?`}
              description={`Se borrará el fichero «${identidadDe(unidad)}». No se puede deshacer.`}
              okText="Sí, borrar"
              cancelText="Cancelar"
              okButtonProps={{ danger: true }}
              onConfirm={() => onBorrar(unidad)}
            >
              <Tooltip title={etiquetaDeAccion('borrar')}>
                <Button
                  size="small"
                  danger
                  aria-label={borrar(identidad)}
                  disabled={bloqueada}
                >
                  {iconoBorrar}
                </Button>
              </Tooltip>
            </Popconfirm>
          </Space>
        )
      },
    },
  ]

  const hayFiltros = tipo !== null || estado !== null

  // `aria-label` llega al `<table>` real: antd lo reenvía a rc-table, que lo
  // esparce sobre el elemento (`pickAttrs(props, { aria: true })`).
  return (
    <>
      <Space size="small" wrap style={{ marginBottom: 12 }}>
        <Select<TipoDeQuadlet>
          aria-label="Filtrar por tipo"
          allowClear
          placeholder="Filtrar por tipo"
          style={{ minWidth: 180 }}
          value={tipo ?? undefined}
          options={tiposDisponibles.map((valor) => ({ label: valor, value: valor }))}
          onChange={(valor) => setTipo(valor ?? null)}
        />
        <Select<string>
          aria-label="Filtrar por estado"
          allowClear
          placeholder="Filtrar por estado"
          style={{ minWidth: 180 }}
          value={estado ?? undefined}
          options={estadosDisponibles.map((valor) => ({ label: valor, value: valor }))}
          onChange={(valor) => setEstado(valor ?? null)}
        />
      </Space>
      <Table<QuadletUnit>
        aria-label="Quadlets"
        // El `file_name` es la identidad única de la quadlet: dos filas pueden
        // compartir nombre de unidad (colisión no inyectiva), pero nunca fichero.
        rowKey="file_name"
        columns={columnas}
        dataSource={filas}
        loading={cargando}
        pagination={false}
        scroll={{ x: 'max-content' }}
        locale={{
          emptyText: (
            <Empty
              description={
                hayFiltros
                  ? 'No hay quadlets que coincidan con los filtros'
                  : 'No hay quadlets'
              }
            />
          ),
        }}
      />
    </>
  )
}
