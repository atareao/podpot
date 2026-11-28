import { createElement } from 'react'
import type { ReactNode } from 'react'
import { PlayCircleOutlined, StopOutlined, ReloadOutlined, EditOutlined, FileTextOutlined, DeleteOutlined } from '@ant-design/icons'

export type AccionNombre = 'start' | 'stop' | 'restart' | 'edit' | 'logs' | 'borrar'

export interface AccionIcono {
  icono: ReactNode
  nombre: (identidad: string) => string
}

/**
 * Etiqueta visible de cada acción: única fuente de las palabras que aparecen
 * en los tooltips y en los nombres accesibles de los botones.
 */
const ETIQUETAS: Record<AccionNombre, string> = {
  start: 'Arrancar',
  stop: 'Parar',
  restart: 'Reiniciar',
  edit: 'Editar',
  logs: 'Logs',
  borrar: 'Borrar',
}

const ACCIONES: Record<AccionNombre, AccionIcono> = {
  start: { icono: createElement(PlayCircleOutlined), nombre: (identidad) => `${ETIQUETAS.start} ${identidad}` },
  stop: { icono: createElement(StopOutlined), nombre: (identidad) => `${ETIQUETAS.stop} ${identidad}` },
  restart: { icono: createElement(ReloadOutlined), nombre: (identidad) => `${ETIQUETAS.restart} ${identidad}` },
  edit: { icono: createElement(EditOutlined), nombre: (identidad) => `${ETIQUETAS.edit} ${identidad}` },
  logs: { icono: createElement(FileTextOutlined), nombre: (identidad) => `${ETIQUETAS.logs} ${identidad}` },
  borrar: { icono: createElement(DeleteOutlined), nombre: (identidad) => `${ETIQUETAS.borrar} ${identidad}` },
}

/** Etiqueta del tooltip de una acción, sin la identidad de la fila. */
export function etiquetaDeAccion(accion: AccionNombre): string {
  return ETIQUETAS[accion]
}

export function iconoYAccion(accion: AccionNombre): AccionIcono {
  return ACCIONES[accion]
}