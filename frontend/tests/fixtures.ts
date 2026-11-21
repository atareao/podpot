/**
 * Constructores de DTO para las pruebas. Rellenan los campos que no importan a
 * la prueba con valores válidos del contrato, para que cada test solo escriba lo
 * que está afirmando.
 */

import type { ProgresoDeJob, QuadletUnit } from '../src/types/api'

export function unidad(campos: Partial<QuadletUnit> & { name: string }): QuadletUnit {
  return {
    // Valor por defecto del contrato: la API publica el nombre de fichero real.
    // Aquí solo se rellena con la forma más común (`<stem>.container`) para que
    // cada prueba escriba únicamente lo que afirma; no es una deducción de
    // producción. Los casos no triviales (volumen, red, pod, colisión) pasan su
    // propio `file_name`.
    file_name: `${campos.name.replace(/\.service$/, '')}.container`,
    kind: 'container',
    active_state: 'active',
    sub_state: 'running',
    unit_file_state: 'generated',
    ...campos,
  }
}

/**
 * Progreso de un job de una unidad, con la forma que publica el canal
 * `GET /api/v1/jobs/stream` ya desenvuelta. Por defecto, un alta (`new`) sin
 * resultado; cada prueba sobrescribe lo que afirma.
 */
export function progresoDeJob(campos: Partial<ProgresoDeJob> & { unit: string }): ProgresoDeJob {
  return {
    job: 1,
    kind: 'start',
    phase: 'new',
    result: null,
    ...campos,
  }
}
