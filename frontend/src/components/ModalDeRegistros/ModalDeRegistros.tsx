import { memo, useEffect, useMemo, useRef, useState } from 'react'
import { Alert, Button, Modal, Empty } from 'antd'
import { useRegistrosDeUnidad } from '../../hooks/useRegistrosDeUnidad'
import type { LineaEnVisor } from '../../hooks/useRegistrosDeUnidad'
import type { QuadletUnit } from '../../types/api'

export interface ModalDeRegistrosProps {
  abierto: boolean
  /** Unidad cuyo canal SSE se abre mientras el modal está visible. */
  unidad: QuadletUnit
  onCerrar: () => void
}

/** Distancia al fondo (px) por debajo de la cual se considera «pegado al fondo». */
const UMBRAL_DE_FONDO_PX = 24

const FilaDeRegistro = memo(function FilaDeRegistro({ linea }: { linea: LineaEnVisor }) {
  return (
    <div style={{ display: 'flex', gap: 12, fontFamily: 'monospace' }}>
      <span style={{ color: '#8c8c8c', flexShrink: 0 }}>{linea.stream}</span>
      <span>{linea.line}</span>
    </div>
  )
})

/**
 * Visor de registros en un modal. El canal solo existe mientras está abierto:
 * se pasa `null` al hook en caso contrario y este da de baja la suscripción.
 */
export function ModalDeRegistros({ abierto, unidad, onCerrar }: ModalDeRegistrosProps) {
  const { lineas, error } = useRegistrosDeUnidad(abierto ? unidad.name : null)
  const [busqueda, setBusqueda] = useState('')
  const contenedor = useRef<HTMLDivElement>(null)
  const fin = useRef<HTMLDivElement>(null)
  const pegadoAlFondo = useRef(true)

  const visibles = useMemo(() => {
    const texto = busqueda.trim().toLocaleLowerCase()
    return texto === ''
      ? lineas
      : lineas.filter((linea) => linea.line.toLocaleLowerCase().includes(texto))
  }, [lineas, busqueda])

  const alDesplazar = (): void => {
    const elemento = contenedor.current
    if (elemento === null) return
    const distancia = elemento.scrollHeight - elemento.scrollTop - elemento.clientHeight
    pegadoAlFondo.current = distancia <= UMBRAL_DE_FONDO_PX
  }

  useEffect(() => {
    if (!pegadoAlFondo.current) return
    // `scrollIntoView` no existe en jsdom: la llamada opcional lo deja no-op.
    fin.current?.scrollIntoView?.({ block: 'end' })
  }, [visibles])

  return (
    <Modal
      open={abierto}
      title={`Registros de ${unidad.name}`}
      onCancel={onCerrar}
      footer={<Button onClick={onCerrar}>Cerrar</Button>}
    >
      <input
        type="text"
        aria-label="Buscar en registros"
        placeholder="Buscar en registros"
        value={busqueda}
        onChange={(evento) => setBusqueda(evento.target.value)}
        style={{ width: '100%', padding: '8px 12px', boxSizing: 'border-box', marginBottom: 12 }}
      />
      {error !== null && (
        <Alert type="warning" showIcon message={error} style={{ marginBottom: 12 }} />
      )}
      <div
        ref={contenedor}
        role="log"
        aria-live="off"
        aria-label={`Registros de ${unidad.name}`}
        onScroll={alDesplazar}
        style={{
          maxHeight: 420,
          overflowY: 'auto',
          border: '1px solid #f0f0f0',
          borderRadius: 8,
          padding: 12,
        }}
      >
        {visibles.length === 0 ? (
          <Empty description="Sin registros todavía" />
        ) : (
          visibles.map((linea) => <FilaDeRegistro key={linea.clave} linea={linea} />)
        )}
        <div ref={fin} />
      </div>
    </Modal>
  )
}
