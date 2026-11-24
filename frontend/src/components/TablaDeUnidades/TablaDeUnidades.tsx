import { Button, Empty, Popconfirm, Space, Spin, Tag } from 'antd'
import { EstadoDeUnidad } from '../EstadoDeUnidad/EstadoDeUnidad'
import { etiquetaDeAccionEnCurso } from '../../acciones'
import { identidadDe } from '../../identidad'
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
  return (
    <table aria-label="Quadlets" className="podpot-tabla">
      <thead>
        <tr>
          <th scope="col">Unidad</th>
          <th scope="col">Estado</th>
          <th scope="col">Progreso</th>
          <th scope="col">Sub-estado</th>
          <th scope="col">Fichero</th>
          <th scope="col">Acciones</th>
        </tr>
      </thead>
      <tbody>
        {cargando ? (
          <tr>
            <td colSpan={6} className="podpot-tabla__cargando">
              <Spin size="small" />
              <span> Cargando…</span>
            </td>
          </tr>
        ) : unidades.length === 0 ? (
          <tr>
            <td colSpan={6}>
              <Empty description="No hay quadlets" />
            </td>
          </tr>
        ) : (
          unidades.map((unidad) => {
            const bloqueada = enCurso === unidad.name
            const identidad = identidadDe(unidad)
            const kinds = progresoPorUnidad[unidad.name]
            const etiquetas = kinds?.map(etiquetaDeAccionEnCurso) ?? []
            return (
              <tr key={unidad.file_name}>
                <td className="podpot-tabla__celda--unidad">
                  <Space direction="vertical" size={0}>
                    <span>{identidadDe(unidad)}</span>
                    <span className="podpot-texto-secundario">{unidad.name}</span>
                  </Space>
                </td>
                <td>
                  <EstadoDeUnidad estado={unidad.active_state} />
                </td>
                <td>
                  {kinds && kinds.length > 0 && (
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
                  )}
                </td>
                <td>{unidad.sub_state}</td>
                <td>{unidad.unit_file_state}</td>
                <td>
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
                    <Button
                      size="small"
                      aria-label={`Logs ${identidad}`}
                      onClick={() => onVerRegistros(unidad)}
                    >
                      Logs
                    </Button>
                    <Popconfirm
                      title={`¿Borrar la quadlet «${identidadDe(unidad)}»?`}
                      description={`Se borrará el fichero «${identidadDe(unidad)}». No se puede deshacer.`}
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
                </td>
              </tr>
            )
          })
        )}
      </tbody>
    </table>
  )
}