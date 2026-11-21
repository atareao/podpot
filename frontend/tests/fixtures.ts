/**
 * Constructores de DTO para las pruebas. Rellenan los campos que no importan a
 * la prueba con valores válidos del contrato, para que cada test solo escriba lo
 * que está afirmando.
 */

import type { QuadletUnit } from '../src/types/api'

export function unidad(campos: Partial<QuadletUnit> & { name: string }): QuadletUnit {
  return {
    active_state: 'active',
    sub_state: 'running',
    unit_file_state: 'generated',
    ...campos,
  }
}
