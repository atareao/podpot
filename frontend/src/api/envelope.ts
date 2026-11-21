/**
 * Desarrollo de la envoltura única de la API de podpot.
 *
 * - **Éxito:** `{ "data": <T|null> }`.
 * - **Error:** `{ "error": { "code": "<snake_case>", "message": "<texto>" } }`.
 *
 * Un cuerpo sin envoltura (un proxy, un HTML de un `502`) no es de fiar: es un
 * error de **protocolo**, no un dato a medio leer.
 */

/** Forma de la respuesta antes de desenvolverla: el código HTTP y el cuerpo crudo. */
export interface RespuestaEnBruto {
  /** Código HTTP de la cabecera. */
  http: number
  /** Cuerpo ya parseado, tal cual (puede no ser un objeto). */
  cuerpo: unknown
}

/** La respuesta es un error de la API: `{error: {code, message}}`. */
export class ErrorDeApi extends Error {
  readonly status: number
  readonly code: string

  constructor(status: number, code: string, message: string) {
    super(message)
    this.name = 'ErrorDeApi'
    this.status = status
    this.code = code
  }
}

/** La respuesta incumple el protocolo: le falta la envoltura. */
export class ErrorDeProtocolo extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'ErrorDeProtocolo'
    this.status = status
  }
}

function esObjeto(valor: unknown): valor is Record<string, unknown> {
  return typeof valor === 'object' && valor !== null && !Array.isArray(valor)
}

/**
 * Quita la envoltura y devuelve el `data`.
 *
 * @throws {ErrorDeApi} si el cuerpo trae la envoltura de error, con su código y mensaje.
 * @throws {ErrorDeProtocolo} si el cuerpo no trae ninguna de las dos envolturas.
 */
export function desenvolver<T>(respuesta: RespuestaEnBruto): T {
  const { http, cuerpo } = respuesta

  if (!esObjeto(cuerpo)) {
    throw new ErrorDeProtocolo(
      http,
      `la respuesta (HTTP ${http}) no trae la envoltura de la API: se esperaba un objeto JSON`,
    )
  }

  const error = cuerpo.error
  if (esObjeto(error)) {
    const code = typeof error.code === 'string' && error.code !== '' ? error.code : 'error'
    const message =
      typeof error.message === 'string' && error.message !== ''
        ? error.message
        : `la API contestó ${http}`
    throw new ErrorDeApi(http, code, message)
  }

  if (!('data' in cuerpo)) {
    throw new ErrorDeProtocolo(
      http,
      `la respuesta (HTTP ${http}) no trae ni \`data\` ni \`error\``,
    )
  }

  return cuerpo.data as T
}
