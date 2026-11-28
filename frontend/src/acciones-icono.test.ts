import { isValidElement } from 'react'
import type { ReactElement } from 'react'
import {
  DeleteOutlined,
  EditOutlined,
  FileTextOutlined,
  PlayCircleOutlined,
  ReloadOutlined,
  StopOutlined,
} from '@ant-design/icons'
import { etiquetaDeAccion, iconoYAccion } from './acciones-icono'
import type { AccionNombre } from './acciones-icono'

interface Caso {
  accion: AccionNombre
  Icono: ReactElement['type']
  etiqueta: string
}

const CASOS: readonly Caso[] = [
  { accion: 'start', Icono: PlayCircleOutlined, etiqueta: 'Arrancar' },
  { accion: 'stop', Icono: StopOutlined, etiqueta: 'Parar' },
  { accion: 'restart', Icono: ReloadOutlined, etiqueta: 'Reiniciar' },
  { accion: 'edit', Icono: EditOutlined, etiqueta: 'Editar' },
  { accion: 'logs', Icono: FileTextOutlined, etiqueta: 'Logs' },
  { accion: 'borrar', Icono: DeleteOutlined, etiqueta: 'Borrar' },
]

describe('acciones-icono', () => {
  it.each(CASOS)('asocia el icono y el nombre a $accion', ({ accion, Icono, etiqueta }) => {
    const { icono, nombre } = iconoYAccion(accion)

    expect(isValidElement(icono) && icono.type).toBe(Icono)
    expect(nombre('web.container')).toBe(`${etiqueta} web.container`)
  })

  it.each(CASOS)('expone la etiqueta de $accion para los tooltips', ({ accion, etiqueta }) => {
    expect(etiquetaDeAccion(accion)).toBe(etiqueta)
  })
})
