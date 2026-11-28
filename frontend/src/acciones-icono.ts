import { createElement } from 'react'
import type { ReactNode } from 'react'
import { PlayCircleOutlined, StopOutlined, ReloadOutlined, EditOutlined, FileTextOutlined, DeleteOutlined } from '@ant-design/icons'

export type AccionNombre = 'start' | 'stop' | 'restart' | 'edit' | 'logs' | 'borrar'

export interface AccionIcono {
  icono: ReactNode
  nombre: (identidad: string) => string
}

const ACCIONES: Record<AccionNombre, AccionIcono> = {
  start: { icono: createElement(PlayCircleOutlined), nombre: (identidad) => `Arrancar ${identidad}` },
  stop: { icono: createElement(StopOutlined), nombre: (identidad) => `Parar ${identidad}` },
  restart: { icono: createElement(ReloadOutlined), nombre: (identidad) => `Reiniciar ${identidad}` },
  edit: { icono: createElement(EditOutlined), nombre: (identidad) => `Editar ${identidad}` },
  logs: { icono: createElement(FileTextOutlined), nombre: (identidad) => `Logs ${identidad}` },
  borrar: { icono: createElement(DeleteOutlined), nombre: (identidad) => `Borrar ${identidad}` },
}

export function iconoYAccion(accion: AccionNombre): AccionIcono {
  return ACCIONES[accion]
}