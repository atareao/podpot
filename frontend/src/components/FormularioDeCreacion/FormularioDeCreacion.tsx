import { Button, Form, Input, Select, Space } from 'antd'
import type { PeticionDeCreacion, TipoDeQuadlet } from '../../types/api'

/**
 * Etiquetas del selector de tipo, con su extensión a la vista.
 *
 * Se declara como `Record<TipoDeQuadlet, string>`: si se añade una variante al
 * tipo, TypeScript exige aquí su etiqueta en vez de dejar el desplegable cojo.
 * El orden de las claves es el de inserción, que es el que verá el usuario.
 */
const ETIQUETAS: Record<TipoDeQuadlet, string> = {
  container: 'Contenedor (.container)',
  volume: 'Volumen (.volume)',
  network: 'Red (.network)',
  pod: 'Pod (.pod)',
}

/** Opciones del `Select`, derivadas una sola vez del mapa de etiquetas. */
const OPCIONES = (Object.entries(ETIQUETAS) as Array<[TipoDeQuadlet, string]>).map(
  ([value, label]) => ({ value, label }),
)

/** Valores tal y como los recoge el formulario (planos, sin anidar). */
interface ValoresDeFormulario {
  kind: TipoDeQuadlet
  name: string
  image?: string
}

export interface FormularioDeCreacionProps {
  /**
   * Crea la quadlet con la API. Devuelve `true` si se creó: solo entonces se
   * limpia el formulario, para no perder lo escrito si hubo un error.
   */
  onCrear: (peticion: PeticionDeCreacion) => Promise<boolean> | boolean
  /** Mientras hay una creación en curso: el botón se bloquea. */
  creando?: boolean
}

/**
 * Compone el cuerpo plano de la petición desde los valores del formulario.
 *
 * El cliente **nunca** manda contenido de fichero ni nada anidado: solo los
 * campos del contrato (`name`, `kind` y, en `.container`, `image`). Un campo de
 * más lo rechazaría el `deny_unknown_fields` del backend con un `400`.
 */
function construirPeticion(valores: ValoresDeFormulario): PeticionDeCreacion {
  const peticion: PeticionDeCreacion = { name: valores.name.trim(), kind: valores.kind }
  if (valores.kind === 'container') {
    peticion.image = valores.image?.trim()
  }
  return peticion
}

/**
 * Formulario Ant Design para crear una quadlet nueva.
 *
 * Presentacional respecto a React Query: quien lo usa (`pages/unidades.tsx`)
 * conecta la mutación, avisa del resultado y refresca el listado. El selector de
 * tipo decide qué campos mínimos se piden: un `.container` necesita su imagen;
 * `volume`, `network` y `pod` se conforman con el nombre.
 */
export function FormularioDeCreacion({ onCrear, creando = false }: FormularioDeCreacionProps) {
  const [formulario] = Form.useForm<ValoresDeFormulario>()
  // El tipo manda en los campos visibles: se observa y el resto del formulario
  // se adapta sin duplicar el estado en un `useState` que podría desincronizarse.
  const kind = Form.useWatch('kind', formulario) ?? 'container'

  const alEnviar = async (valores: ValoresDeFormulario) => {
    const creada = await onCrear(construirPeticion(valores))
    if (creada) formulario.resetFields()
  }

  return (
    <Form<ValoresDeFormulario>
      form={formulario}
      layout="vertical"
      initialValues={{ kind: 'container' }}
      onFinish={(valores) => void alEnviar(valores)}
    >
      <Space size="middle" wrap align="start">
        <Form.Item name="kind" label="Tipo de quadlet">
          <Select style={{ width: 200 }} options={OPCIONES} />
        </Form.Item>

        <Form.Item
          name="name"
          label="Nombre"
          rules={[{ required: true, whitespace: true, message: 'Introduce un nombre' }]}
        >
          <Input style={{ width: 220 }} placeholder="web" autoComplete="off" />
        </Form.Item>

        {kind === 'container' && (
          <Form.Item
            name="image"
            label="Imagen"
            rules={[{ required: true, whitespace: true, message: 'Introduce una imagen' }]}
          >
            <Input
              style={{ width: 320 }}
              placeholder="docker.io/library/nginx"
              autoComplete="off"
            />
          </Form.Item>
        )}
      </Space>

      <Form.Item style={{ marginBottom: 0 }}>
        <Button type="primary" htmlType="submit" loading={creando}>
          Crear
        </Button>
      </Form.Item>
    </Form>
  )
}
