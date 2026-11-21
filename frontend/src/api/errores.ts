import { ErrorDeApi, ErrorDeProtocolo } from './envelope'
import { ErrorDeRed } from './client'

/** Mensaje presentable de un error de la API o de red, sin volcar JSON crudo. */
export function mensajeDeError(error: unknown): string {
  if (error instanceof ErrorDeApi) return error.message
  if (error instanceof ErrorDeProtocolo) return 'La respuesta del servidor no era la esperada.'
  if (error instanceof ErrorDeRed) return 'No se pudo contactar con el servidor.'
  if (error instanceof Error) return error.message
  return 'Ha ocurrido un error inesperado.'
}
