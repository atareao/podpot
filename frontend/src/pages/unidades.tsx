import { Alert, App, Button, Card, Typography } from 'antd'
import { TablaDeUnidades } from '../components/TablaDeUnidades/TablaDeUnidades'
import { FormularioDeCreacion } from '../components/FormularioDeCreacion/FormularioDeCreacion'
import { useAccionDeUnidad, useCrearQuadlet, useUnits } from '../hooks/useUnits'
import { mensajeDeError } from '../api/errores'
import type { AccionDeUnidad, PeticionDeCreacion, QuadletUnit } from '../types/api'

const VERBOS: Record<AccionDeUnidad, string> = {
  start: 'arrancada',
  stop: 'parada',
  restart: 'reiniciada',
}

/**
 * Pantalla principal: el listado de quadlets, sus acciones de control y el
 * formulario de creación.
 *
 * Una acción avisa por `message` con su desenlace y deja que la mutación
 * refresque el listado (el estado lo manda systemd). Mientras corre, los
 * botones de esa unidad se deshabilitan para no encadenar acciones.
 *
 * La creación sigue el mismo patrón: al crear con éxito la mutación invalida el
 * listado —la quadlet solo aparece cuando systemd la conoce—; al fallar se
 * muestra el `error.message` de la API (colisión incluida) y **no** se refresca.
 */
export default function Unidades() {
  const { message } = App.useApp()
  const unidades = useUnits()
  const accion = useAccionDeUnidad()
  const creacion = useCrearQuadlet()

  // Estado derivado: la unidad «en curso» es la que lleva la mutación pendiente.
  // No se duplica en un `useState` propio que podría desincronizarse.
  const enCurso = accion.isPending ? (accion.variables?.name ?? null) : null

  const alAccionar = async (unidad: QuadletUnit, tipo: AccionDeUnidad) => {
    try {
      const resultado = await accion.mutateAsync({ name: unidad.name, accion: tipo })
      message.success(`Unidad «${resultado.name}» ${VERBOS[tipo]} (${resultado.active_state}).`)
    } catch (error) {
      message.error(mensajeDeError(error))
    }
  }

  const alCrear = async (peticion: PeticionDeCreacion): Promise<boolean> => {
    try {
      const creada = await creacion.mutateAsync(peticion)
      message.success(`Quadlet «${creada.unit_name}» creada.`)
      return true
    } catch (error) {
      message.error(mensajeDeError(error))
      return false
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

      <Card title="Nueva quadlet" size="small" style={{ marginBottom: 16 }}>
        <FormularioDeCreacion onCrear={alCrear} creando={creacion.isPending} />
      </Card>

      <TablaDeUnidades
        unidades={unidades.data ?? []}
        cargando={unidades.isPending}
        enCurso={enCurso}
        onAccion={(unidad, tipo) => void alAccionar(unidad, tipo)}
      />
    </section>
  )
}
