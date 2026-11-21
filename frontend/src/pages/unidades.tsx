import { useState } from 'react'
import { Alert, App, Button, Card, Space, Spin, Typography } from 'antd'
import { TablaDeUnidades } from '../components/TablaDeUnidades/TablaDeUnidades'
import { FormularioDeCreacion } from '../components/FormularioDeCreacion/FormularioDeCreacion'
import type { ValoresDeFormulario } from '../components/FormularioDeCreacion/FormularioDeCreacion'
import {
  useAccionDeUnidad,
  useBorrarQuadlet,
  useCrearQuadlet,
  useDetalleDeQuadlet,
  useEditarQuadlet,
  useUnits,
  useVerificarQuadlet,
} from '../hooks/useUnits'
import { mensajeDeError } from '../api/errores'
import type {
  AccionDeUnidad,
  PeticionDeCreacion,
  QuadletDetalle,
  QuadletUnit,
} from '../types/api'

const VERBOS: Record<AccionDeUnidad, string> = {
  start: 'arrancada',
  stop: 'parada',
  restart: 'reiniciada',
}

/** Valores de arranque del formulario: los que expone, tal cual se leyeron. */
function valoresDeEdicion(definicion: QuadletDetalle): Partial<ValoresDeFormulario> {
  return { kind: definicion.kind, name: definicion.name, image: definicion.image }
}

/**
 * Completa el cuerpo del `PATCH` con los campos que el formulario no expone.
 *
 * El `PATCH` reescribe el struct tipado **completo** —no hay fusión campo a
 * campo—, así que editar el nombre o la imagen desde el panel borraría todo lo
 * demás que el fichero tuviera. Se devuelven sin tocar los campos leídos que la
 * UI no muestra (fuera de su alcance por ahora): lo que el formulario sí expone
 * manda, porque va después en el objeto.
 */
function completarConDefinicion(
  peticion: PeticionDeCreacion,
  definicion: QuadletDetalle,
): PeticionDeCreacion {
  return {
    container_name: definicion.container_name,
    publish_ports: definicion.publish_ports,
    volumes: definicion.volumes,
    environments: definicion.environments,
    volume_name: definicion.volume_name,
    network_name: definicion.network_name,
    subnet: definicion.subnet,
    pod_name: definicion.pod_name,
    ...peticion,
  }
}

/**
 * Pantalla principal: el listado de quadlets, sus acciones de control y el
 * formulario de creación —que se reutiliza, precargado, para editar—.
 *
 * Una acción avisa por `message` con su desenlace y deja que la mutación
 * refresque el listado (el estado lo manda systemd). Mientras corre, los
 * botones de esa unidad se deshabilitan para no encadenar acciones.
 *
 * La creación sigue el mismo patrón: al crear con éxito la mutación invalida el
 * listado —la quadlet solo aparece cuando systemd la conoce—; al fallar se
 * muestra el `error.message` de la API (colisión incluida) y **no** se refresca.
 *
 * La edición lee primero la definición (`GET`), precarga el formulario con ese
 * tipo y esos campos y guarda con `PATCH` sobre el **nombre de fichero** que
 * publica la API (`unit.file_name`, la identidad real de la quadlet); si la
 * lectura falla se explica y **no** se abre el formulario con valores
 * inventados. El borrado exige confirmación en la propia fila y, antes de enviar
 * el `DELETE` destructivo, verifica con un `GET` que el fichero existe (el
 * listado es una foto y el fichero pudo desaparecer fuera del panel). Los
 * desenlaces avisan igual que la creación: `error.message` al fallar, refresco
 * al acertar.
 *
 * Mientras haya **cualquier** operación sobre una unidad en vuelo —acción,
 * lectura de la definición, guardado de la edición o borrado— sus botones de
 * fila se deshabilitan, para no encadenar una operación destructiva sobre una
 * edición a medias.
 */
export default function Unidades() {
  const { message } = App.useApp()
  const unidades = useUnits()
  const accion = useAccionDeUnidad()
  const creacion = useCrearQuadlet()
  const edicion = useEditarQuadlet()
  const verificacion = useVerificarQuadlet()
  const borrado = useBorrarQuadlet()
  const [enEdicion, setEnEdicion] = useState<QuadletUnit | null>(null)
  // Unidad con un borrado en vuelo (verificación o `DELETE`): la operación va por
  // nombre de fichero, pero la fila a deshabilitar se identifica por unidad.
  const [enBorrado, setEnBorrado] = useState<QuadletUnit | null>(null)
  // La definición se pide al entrar en edición; hasta que llegue, no hay
  // formulario: precargarlo a medias sería enseñar valores que no son.
  const lectura = useDetalleDeQuadlet(enEdicion?.file_name ?? null)

  /**
   * Unidad con alguna operación en vuelo, si la hay. Se deriva —nunca se
   * duplica en un `useState` que podría desincronizarse— por orden de gravedad:
   * una acción de control, un borrado (verificación + `DELETE`) y, por último,
   * la edición (lectura de la definición o guardado). Los botones de esa fila se
   * deshabilitan para no encadenar sobre ella una operación destructiva.
   *
   * El orden no es cosmético: una consulta deshabilitada también reporta
   * `isPending`, así que la edición solo cuenta cuando hay una edición abierta.
   */
  const enCurso: string | null = (() => {
    if (accion.isPending) return accion.variables?.name ?? null
    if (verificacion.isPending || borrado.isPending) return enBorrado?.name ?? null
    if (enEdicion !== null && (lectura.isPending || edicion.isPending)) {
      return enEdicion.name
    }
    return null
  })()

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

  /** Entra en edición: el fichero a leer es la identidad publicada por la API. */
  const alEditar = (unidad: QuadletUnit) => {
    setEnEdicion(unidad)
  }

  const alGuardarEdicion = async (peticion: PeticionDeCreacion): Promise<boolean> => {
    if (enEdicion === null || lectura.data === undefined) return false
    try {
      await edicion.mutateAsync({
        nombre: enEdicion.file_name,
        datos: completarConDefinicion(peticion, lectura.data),
      })
      message.success(`Quadlet «${enEdicion.name}» editada.`)
      setEnEdicion(null)
      return true
    } catch (error) {
      // Sin salir de la edición: lo escrito se conserva para corregirlo.
      message.error(mensajeDeError(error))
      return false
    }
  }

  const alBorrar = async (unidad: QuadletUnit) => {
    setEnBorrado(unidad)
    try {
      // Antes de un borrado destructivo se confirma que el fichero existe: el
      // listado es una foto y el fichero pudo desaparecer o cambiar fuera del
      // panel. Un `404` aquí aborta el `DELETE`.
      await verificacion.mutateAsync(unidad.file_name)
    } catch (error) {
      message.error(mensajeDeError(error))
      setEnBorrado(null)
      return
    }
    try {
      await borrado.mutateAsync(unidad.file_name)
      message.success(`Quadlet «${unidad.name}» borrada.`)
      // Si se estaba editando esa misma quadlet, el formulario ya no tiene sentido.
      setEnEdicion((actual) => (actual?.file_name === unidad.file_name ? null : actual))
    } catch (error) {
      message.error(mensajeDeError(error))
    } finally {
      setEnBorrado(null)
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

      {enEdicion === null ? (
        <Card title="Nueva quadlet" size="small" style={{ marginBottom: 16 }}>
          <FormularioDeCreacion onCrear={alCrear} creando={creacion.isPending} />
        </Card>
      ) : (
        <Card
          title={`Editar quadlet «${enEdicion.name}»`}
          size="small"
          style={{ marginBottom: 16 }}
        >
          {lectura.data !== undefined ? (
            <FormularioDeCreacion
              // El remontaje por fichero es lo que recarga los valores iniciales al
              // cambiar de quadlet editada: `initialValues` solo se lee al montar.
              key={enEdicion.file_name}
              valoresIniciales={valoresDeEdicion(lectura.data)}
              etiquetaDeEnvio="Guardar"
              identidadFija
              alCancelar={() => setEnEdicion(null)}
              onCrear={alGuardarEdicion}
              creando={edicion.isPending}
            />
          ) : lectura.isError ? (
            // Sin definición no hay formulario: un error nunca se disfraza de
            // valores por defecto.
            <Alert
              type="error"
              showIcon
              message="No se pudo leer la quadlet"
              description={mensajeDeError(lectura.error)}
              action={
                <Space size="small">
                  <Button size="small" onClick={() => void lectura.refetch()}>
                    Reintentar
                  </Button>
                  <Button size="small" onClick={() => setEnEdicion(null)}>
                    Cerrar
                  </Button>
                </Space>
              }
            />
          ) : (
            <Space size="small">
              <Spin size="small" />
              <span role="status">Cargando la quadlet…</span>
            </Space>
          )}
        </Card>
      )}

      <TablaDeUnidades
        unidades={unidades.data ?? []}
        cargando={unidades.isPending}
        enCurso={enCurso}
        onAccion={(unidad, tipo) => void alAccionar(unidad, tipo)}
        onEditar={alEditar}
        onBorrar={(unidad) => void alBorrar(unidad)}
      />
    </section>
  )
}
