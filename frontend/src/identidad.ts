import type { QuadletUnit } from './types/api'

/**
 * Identidad con la que se muestra y se nombra una quadlet: su `file_name`
 * publicado por la API (la clave real de `PATCH`/`DELETE`), no el nombre de
 * unidad systemd. El contrato de `file_name` es `string` no nulo; si llega
 * vacío, cae al nombre de unidad para no dejar la fila sin identificar ni los
 * botones sin nombre accesible.
 */
export function identidadDe(unidad: QuadletUnit): string {
  return unidad.file_name || unidad.name
}
