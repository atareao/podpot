import { useState } from 'react'
import { Alert, App, Button, Typography } from 'antd'
import { TablaDeUnidades } from '../components/TablaDeUnidades/TablaDeUnidades'
import { useAccionDeUnidad, useUnits } from '../hooks/useUnits'
import { mensajeDeError } from '../api/errores'
import type { AccionDeUnidad, QuadletUnit } from '../types/api'

const VERBOS: Record<AccionDeUnidad, string> = {
  start: 'arrancada',
  stop: 'parada',
  restart: 'reiniciada',
}

/**
 * Pantalla principal: el listado de quadlets y sus acciones de control.
 *
 * Una acción avisa por `message` con su desenlace y deja que la mutación
 * refresque el listado (el estado lo manda systemd). Mientras corre, los
 * botones de esa unidad se deshabilitan para no encadenar acciones.
 */
export default function Unidades() {
  const { message } = App.useApp()
  const unidades = useUnits()
  const accion = useAccionDeUnidad()
  const [enCurso, setEnCurso] = useState<string | null>(null)

  const alAccionar = async (unidad: QuadletUnit, tipo: AccionDeUnidad) => {
    setEnCurso(unidad.name)
    try {
      const resultado = await accion.mutateAsync({ name: unidad.name, accion: tipo })
      message.success(`Unidad «${resultado.name}» ${VERBOS[tipo]} (${resultado.active_state}).`)
    } catch (error) {
      message.error(mensajeDeError(error))
    } finally {
      setEnCurso(null)
    }
  }

  return (
    <section className="podpot-pagina" aria-labelledby="titulo-de-la-pantalla">
      <Typography.Title id="titulo-de-la-pantalla" level={1} style={{ marginTop: 0 }}>
        Quadlets
      </Typography.Title>

      {unidades.error !== null && (
        <Alert
          type="error"
          showIcon
          message="No se pudieron cargar las quadlets"
          description={mensajeDeError(unidades.error)}
          action={
            <Button size="small" onClick={() => void unidades.refetch()}>
              Reintentar
            </Button>
          }
          style={{ marginBottom: 16 }}
        />
      )}

      <TablaDeUnidades
        unidades={unidades.data ?? []}
        cargando={unidades.isPending}
        enCurso={enCurso}
        onAccion={(unidad, tipo) => void alAccionar(unidad, tipo)}
      />
    </section>
  )
}
