import { useId, useMemo, useState } from 'react'
import { Button, Popconfirm, Space, Spin, Tag, Tooltip, Typography } from 'antd'
import { DownOutlined } from '@ant-design/icons'
import { EstadoDeUnidad } from '../EstadoDeUnidad/EstadoDeUnidad'
import { etiquetaDeAccionEnCurso } from '../../acciones'
import { etiquetaDeAccion, iconoYAccion } from '../../acciones-icono'
import { identidadDe } from '../../identidad'
import { groupByStack } from '../../lib/groupByStack'
import type { TrabajosEnCurso } from '../../hooks/useTrabajosEnVivo'
import type { AccionDeUnidad, QuadletUnit } from '../../types/api'

export interface GrupoDeUnidadesProps {
  /** Nombre del stack que compartes las unidades del grupo. */
  stack: string
  /** Unidades del grupo, en cualquier orden: se ordenan de forma determinista. */
  unidades: QuadletUnit[]
  /** Acción de control solicitada desde una tarjeta. */
  onAccion?: (unidad: QuadletUnit, accion: AccionDeUnidad) => void
  /** Edición solicitada desde una tarjeta. */
  onEditar?: (unidad: QuadletUnit) => void
  /** Apertura del visor de registros solicitada desde una tarjeta. */
  onVerRegistros?: (unidad: QuadletUnit) => void
  /** Borrado **confirmado** desde una tarjeta. */
  onBorrar?: (unidad: QuadletUnit) => void
  /** Inspección de los campos de la quadlet (sin entrar en edición). */
  onInspeccionar?: (unidad: QuadletUnit) => void
  /** Nombre de unidad con una mutación en vuelo: se bloquean sus controles. */
  enCurso?: string | null
  /** Tipos de job en curso por unidad, publicados por el canal SSE. */
  progresoPorUnidad?: TrabajosEnCurso
  /** Unidades a resaltar tras un cambio de estado en vivo. */
  resaltadas?: ReadonlySet<string>
}

const EN_MARCHA: ReadonlySet<string> = new Set(['active', 'activating', 'reloading'])
const PARADA: ReadonlySet<string> = new Set(['inactive', 'deactivating'])

/**
 * Grupo de tarjetas de un stack. La cabecera es un botón —así funciona con
 * ratón y con teclado— y el cuerpo se identifica con `aria-controls`. El orden
 * de las tarjetas sale de `groupByStack`: principales antes que auxiliares y,
 * dentro de cada bloque, por nombre. No depende del `active_state`, de modo que
 * un evento SSE no mueve una tarjeta de sitio. El grupo y su cuerpo son
 * `role="presentation"`: así las tarjetas son hijos accesibles directos del
 * `list` del listado y no se interpone un contenedor con rol propio.
 */
export function GrupoDeUnidades({
  stack,
  unidades,
  onAccion,
  onEditar,
  onVerRegistros,
  onBorrar,
  onInspeccionar,
  enCurso = null,
  progresoPorUnidad = {},
  resaltadas,
}: GrupoDeUnidadesProps) {
  const [abierto, setAbierto] = useState(true)
  const idDelCuerpo = useId()

  const ordenadas = useMemo(
    () => groupByStack(unidades).flatMap((grupo) => grupo.unidades),
    [unidades],
  )

  const { nombre: arrancar, icono: iconoArrancar } = iconoYAccion('start')
  const { nombre: parar, icono: iconoParar } = iconoYAccion('stop')
  const { nombre: reiniciar, icono: iconoReiniciar } = iconoYAccion('restart')
  const { nombre: editar, icono: iconoEditar } = iconoYAccion('edit')
  const { nombre: logs, icono: iconoLogs } = iconoYAccion('logs')
  const { nombre: borrar, icono: iconoBorrar } = iconoYAccion('borrar')

  return (
    <div role="presentation" className="podpot-grupo">
      <button
        id={`${idDelCuerpo}-cabecera`}
        type="button"
        className="podpot-grupo__cabecera"
        aria-expanded={abierto}
        aria-controls={idDelCuerpo}
        onClick={() => setAbierto((valor) => !valor)}
      >
        <DownOutlined
          aria-hidden="true"
          className={abierto ? 'podpot-grupo__chevron' : 'podpot-grupo__chevron podpot-grupo__chevron--cerrado'}
        />
        <span>{stack}</span>
        <Typography.Text type="secondary">
          {ordenadas.length} {ordenadas.length === 1 ? 'unidad' : 'unidades'}
        </Typography.Text>
      </button>

      {abierto && (
        <div
          id={idDelCuerpo}
          role="presentation"
          className="podpot-grupo__tarjetas"
        >
          {ordenadas.map((unidad) => {
            const identidad = identidadDe(unidad)
            const kinds = progresoPorUnidad[unidad.name] ?? []
            const bloqueada = enCurso === unidad.name || kinds.length > 0
            const etiquetas = kinds.map(etiquetaDeAccionEnCurso)
            const resaltada = resaltadas?.has(unidad.name) ?? false

            return (
              <article
                key={unidad.file_name || unidad.name}
                role="listitem"
                aria-label={`Unidad ${identidad}`}
                className={resaltada ? 'podpot-tarjeta podpot-tarjeta--resaltada' : 'podpot-tarjeta'}
              >
                {/* El `file_name` es la identidad primaria; el nombre de unidad
                    queda como dato secundario, en el mismo bloque y por detrás. */}
                <table role="presentation" className="podpot-tarjeta__identidad">
                  <tbody>
                    <tr>
                      <td role="presentation">
                        <button
                          type="button"
                          className="podpot-tarjeta__enlace"
                          aria-label={`Detalles de ${identidad}`}
                          onClick={() => onInspeccionar?.(unidad)}
                        >
                          <Typography.Text strong>{identidad}</Typography.Text>
                        </button>
                        <Typography.Text type="secondary">{unidad.name}</Typography.Text>
                      </td>
                    </tr>
                  </tbody>
                </table>

                <div className="podpot-tarjeta__estado">
                  <EstadoDeUnidad estado={unidad.active_state} />
                </div>

                <dl className="podpot-tarjeta__datos">
                  <dt>Sub-estado</dt>
                  <dd>{unidad.sub_state}</dd>
                  <dt>Fichero</dt>
                  <dd>{unidad.unit_file_state}</dd>
                  <dt>Tipo</dt>
                  <dd>{unidad.kind ?? 'desconocido'}</dd>
                </dl>

                {etiquetas.length > 0 && (
                  <div
                    role="status"
                    aria-label={`Acción en curso en ${identidad}: ${etiquetas.join(', ')}`}
                    className="podpot-tarjeta__progreso"
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

                <Space size="small" wrap className="podpot-tarjeta__acciones">
                  <Tooltip title={etiquetaDeAccion('start')}>
                    <Button
                      size="small"
                      aria-label={arrancar(identidad)}
                      disabled={bloqueada || EN_MARCHA.has(unidad.active_state)}
                      onClick={() => onAccion?.(unidad, 'start')}
                    >
                      {iconoArrancar}
                    </Button>
                  </Tooltip>
                  <Tooltip title={etiquetaDeAccion('stop')}>
                    <Button
                      size="small"
                      aria-label={parar(identidad)}
                      disabled={bloqueada || PARADA.has(unidad.active_state)}
                      onClick={() => onAccion?.(unidad, 'stop')}
                    >
                      {iconoParar}
                    </Button>
                  </Tooltip>
                  <Tooltip title={etiquetaDeAccion('restart')}>
                    <Button
                      size="small"
                      aria-label={reiniciar(identidad)}
                      disabled={bloqueada}
                      onClick={() => onAccion?.(unidad, 'restart')}
                    >
                      {iconoReiniciar}
                    </Button>
                  </Tooltip>
                  <Tooltip title={etiquetaDeAccion('edit')}>
                    <Button
                      size="small"
                      aria-label={editar(identidad)}
                      disabled={bloqueada}
                      onClick={() => onEditar?.(unidad)}
                    >
                      {iconoEditar}
                    </Button>
                  </Tooltip>
                  {/* Los registros se leen: no se bloquean por un job en curso. */}
                  <Tooltip title={etiquetaDeAccion('logs')}>
                    <Button
                      size="small"
                      aria-label={logs(identidad)}
                      onClick={() => onVerRegistros?.(unidad)}
                    >
                      {iconoLogs}
                    </Button>
                  </Tooltip>
                  <Popconfirm
                    title={`¿Borrar la quadlet «${identidad}»?`}
                    description={`Se borrará el fichero «${identidad}». No se puede deshacer.`}
                    okText="Sí, borrar"
                    cancelText="Cancelar"
                    okButtonProps={{ danger: true }}
                    onConfirm={() => onBorrar?.(unidad)}
                  >
                    <Tooltip title={etiquetaDeAccion('borrar')}>
                      <Button size="small" danger aria-label={borrar(identidad)} disabled={bloqueada}>
                        {iconoBorrar}
                      </Button>
                    </Tooltip>
                  </Popconfirm>
                </Space>
              </article>
            )
          })}
        </div>
      )}
    </div>
  )
}
