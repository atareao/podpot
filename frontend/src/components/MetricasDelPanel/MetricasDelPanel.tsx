import type { CSSProperties } from 'react'
import { theme } from 'antd'
import { useMediaQuery } from '../../hooks/useMediaQuery'
import type { QuadletUnit } from '../../types/api'

export interface MetricasDelPanelProps {
  /** Instantánea actual de quadlets: los recuentos se derivan de ella en cada render. */
  unidades: QuadletUnit[]
}

interface Tarjeta {
  titulo: string
  valor: number
  color: string
}

const CONTENEDOR: CSSProperties = {
  display: 'grid',
  width: '100%',
}

const CONSULTA_MOVIL = '(max-width: 767px)'

/**
 * Cuatro recuentos derivados de `active_state`; no hay estado propio, así que
 * una nueva instantánea (SSE o recarga) actualiza las tarjetas sin más.
 * Los colores y la densidad salen de los tokens del tema vigente (cssVar).
 */
export function MetricasDelPanel({ unidades }: MetricasDelPanelProps) {
  const { token } = theme.useToken()
  const movil = useMediaQuery(CONSULTA_MOVIL)

  const tarjetas: Tarjeta[] = [
    { titulo: 'Total', valor: unidades.length, color: token.colorPrimary },
    {
      titulo: 'En ejecución',
      valor: unidades.filter((unidad) => unidad.active_state === 'active').length,
      color: token.colorSuccessText,
    },
    {
      titulo: 'Paradas',
      valor: unidades.filter((unidad) => unidad.active_state === 'inactive').length,
      color: token.colorTextTertiary,
    },
    {
      titulo: 'Fallidas',
      valor: unidades.filter((unidad) => unidad.active_state === 'failed').length,
      color: token.colorErrorText,
    },
  ]

  const estiloContenedor: CSSProperties = {
    ...CONTENEDOR,
    gridTemplateColumns: `repeat(auto-fit, minmax(${movil ? 140 : 180}px, 1fr))`,
    gap: movil ? token.marginXS : token.margin,
  }

  const estiloEtiqueta: CSSProperties = {
    color: token.colorTextSecondary,
    fontSize: movil ? token.fontSizeSM : token.fontSize,
  }

  return (
    <div style={estiloContenedor}>
      {tarjetas.map((tarjeta) => (
        <section
          key={tarjeta.titulo}
          role="group"
          aria-label={tarjeta.titulo}
          style={{
            border: `1px solid ${token.colorBorderSecondary}`,
            borderRadius: token.borderRadiusLG,
            padding: movil
              ? `${token.paddingXS}px ${token.padding}px`
              : `${token.padding}px ${token.paddingLG}px`,
            background: token.colorBgContainer,
            color: token.colorText,
            borderTop: `3px solid ${tarjeta.color}`,
          }}
        >
          <div style={estiloEtiqueta}>{tarjeta.titulo}</div>
          <div style={{ color: tarjeta.color, fontSize: movil ? 24 : 28, fontWeight: 600, lineHeight: 1.3 }}>
            {tarjeta.valor}
          </div>
        </section>
      ))}
    </div>
  )
}
