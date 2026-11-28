import { Button, Modal } from 'antd'
import type { QuadletDetalle, QuadletUnit } from '../../types/api'

export interface ModalDeDetallesProps {
  abierto: boolean
  /** Fila de la que se pidieron los detalles; solo se usa para el título. */
  unidad: QuadletUnit
  /** Respuesta de `GET /api/v1/quadlets/{name}`, o `null` si aún no llegó. */
  detalle: QuadletDetalle | null
  /** Error de carga, mostrado dentro del modal sin afectar al panel de detrás. */
  error: string | null
  onCerrar: () => void
}

interface CampoProps {
  etiqueta: string
  valores: string[]
}

function Campo({ etiqueta, valores }: CampoProps) {
  return (
    <div style={{ marginBottom: 12 }}>
      <div style={{ color: 'var(--ant-color-text-secondary, #595959)', fontSize: 12 }}>{etiqueta}</div>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px 12px' }}>
        {valores.map((valor) => (
          <span key={valor}>{valor}</span>
        ))}
      </div>
    </div>
  )
}

function camposDelDetalle(detalle: QuadletDetalle): CampoProps[] {
  const campos: CampoProps[] = []
  const anadir = (etiqueta: string, valores: string[] | string | undefined): void => {
    if (valores === undefined) return
    if (typeof valores === 'string') {
      if (valores !== '') campos.push({ etiqueta, valores: [valores] })
    } else if (valores.length > 0) {
      campos.push({ etiqueta, valores })
    }
  }

  anadir('Imagen', detalle.image)
  anadir('Nombre del contenedor', detalle.container_name)
  anadir('Puertos publicados', detalle.publish_ports)
  anadir('Volúmenes', detalle.volumes)
  anadir('Entornos', detalle.environments)
  anadir('Nombre del volumen', detalle.volume_name)
  anadir('Conexión', [detalle.network_name, detalle.subnet].filter((valor) => valor !== undefined))
  anadir('Agrupación', detalle.pod_name)
  return campos
}

/**
 * Detalle de una quadlet con los campos que realmente publica la API. No inventa
 * información (por ejemplo un estado de salud) que el endpoint no devuelve.
 */
export function ModalDeDetalles({
  abierto,
  unidad,
  detalle,
  error,
  onCerrar,
}: ModalDeDetallesProps) {
  const campos = detalle === null ? [] : camposDelDetalle(detalle)

  return (
    <Modal
      open={abierto}
      title={`Detalles de ${unidad.file_name}`}
      onCancel={onCerrar}
      footer={<Button onClick={onCerrar}>Cerrar</Button>}
    >
      {error !== null ? (
        <p role="alert">{error}</p>
      ) : detalle === null ? (
        <p>No hay definición disponible para esta quadlet.</p>
      ) : (
        campos.map((campo) => <Campo key={campo.etiqueta} {...campo} />)
      )}
    </Modal>
  )
}
