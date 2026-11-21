import { memo, useEffect, useRef } from 'react'
import { Alert, Drawer, Empty } from 'antd'
import { useRegistrosDeUnidad } from '../../hooks/useRegistrosDeUnidad'
import type { LineaEnVisor } from '../../hooks/useRegistrosDeUnidad'

export interface VisorDeRegistrosProps {
  /**
   * Nombre de la unidad cuyos registros se muestran, o `null` si el visor está
   * cerrado. Al pasar a `null` se desmonta el cuerpo y con él la suscripción.
   */
  unidad: string | null
  /** Cierre solicitado desde el visor (botón de cerrar o máscara). */
  onCerrar: () => void
}

/** Distancia (px) al fondo por debajo de la cual se considera «pegado al fondo». */
const UMBRAL_DE_FONDO_PX = 24

/**
 * Una fila del visor, memorizada: al llegar un lote solo se montan las filas
 * nuevas y las ya pintadas no se vuelven a renderizar. La clave estable la pone
 * el padre (`linea.clave`), no el índice.
 */
const FilaDeRegistro = memo(function FilaDeRegistro({ linea }: { linea: LineaEnVisor }) {
  return (
    <div
      className={
        linea.stream === 'stderr' ? 'podpot-registro podpot-registro--stderr' : 'podpot-registro'
      }
    >
      {linea.line}
    </div>
  )
})

/**
 * Cuerpo del visor: es quien abre el canal y pinta las líneas.
 *
 * Se separa para que su ciclo de vida coincida exactamente con «visor abierto»:
 * el padre solo lo monta con una unidad no nula, así que desmontarlo —al cerrar—
 * cierra la suscripción sin lógica extra.
 */
function CuerpoDeRegistros({ nombre }: { nombre: string }) {
  const { lineas, error } = useRegistrosDeUnidad(nombre)
  const contenedor = useRef<HTMLDivElement>(null)
  const fin = useRef<HTMLDivElement>(null)
  // Solo se sigue el final si el usuario ya estaba ahí: si subió a leer, no se
  // le arrastra el scroll al llegar líneas nuevas.
  const pegadoAlFondo = useRef(true)

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
  }, [lineas])

  return (
    <div className="podpot-registros">
      {/* El fallo del canal se avisa aquí, dentro del visor: el resto del panel
          sigue funcionando. */}
      {error !== null && (
        <Alert type="warning" showIcon message={error} style={{ marginBottom: 12 }} />
      )}

      {lineas.length === 0 ? (
        <Empty description="Sin registros todavía" />
      ) : (
        <div
          ref={contenedor}
          onScroll={alDesplazar}
          className="podpot-registros__lineas"
          role="log"
          // `role="log"` implica `aria-live="polite"` y un log denso inundaría al
          // lector de pantalla: se apaga explícitamente.
          aria-live="off"
          aria-label={`Registros de ${nombre}`}
        >
          {lineas.map((linea) => (
            <FilaDeRegistro key={linea.clave} linea={linea} />
          ))}
          <div ref={fin} />
        </div>
      )}
    </div>
  )
}

/**
 * Visor de logs de una quadlet en un `Drawer`.
 *
 * Se abre desde una acción de fila de la tabla de unidades. Mientras está
 * abierto, su cuerpo mantiene la suscripción al canal SSE de esa unidad (ver
 * `useRegistrosDeUnidad`); al cerrarlo, el cuerpo se desmonta y el canal se da
 * de baja. No es un canal permanente: los logs solo interesan con el visor
 * abierto.
 */
export function VisorDeRegistros({ unidad, onCerrar }: VisorDeRegistrosProps) {
  return (
    <Drawer
      open={unidad !== null}
      title={unidad !== null ? `Registros de ${unidad}` : 'Registros'}
      placement="right"
      width={720}
      onClose={onCerrar}
    >
      {unidad !== null && <CuerpoDeRegistros nombre={unidad} />}
    </Drawer>
  )
}
