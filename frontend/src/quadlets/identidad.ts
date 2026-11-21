/**
 * Identidad de una quadlet a partir del nombre de unidad que publica la API.
 *
 * La identidad de una quadlet para `PATCH`/`DELETE` es el **nombre de fichero**
 * (`<stem>.<kind>`), pero `GET /api/v1/units` solo publica el nombre de unidad
 * (`<stem>.service` o `<stem>-<tipo>.service`). Este módulo reconstruye el
 * fichero con la regla inversa a la de `QuadletKind::unit_name` del backend.
 *
 * La regla inversa no es unívoca —un contenedor llamado `web-volume` genera el
 * mismo `web-volume.service` que el volumen `web`—, así que se aplica la misma
 * convención que el backend al generar: solo `volume`, `network` y `pod` añaden
 * sufijo, y cualquier otra cosa es un contenedor.
 */

import type { TipoDeQuadlet } from '../types/api'

/** Sufijos de tipo que systemd añade al nombre de unidad, con su extensión. */
const SUFIJOS: ReadonlyArray<readonly [sufijo: string, kind: TipoDeQuadlet]> = [
  ['-volume', 'volume'],
  ['-network', 'network'],
  ['-pod', 'pod'],
]

const EXTENSION_DE_UNIDAD = '.service'

/** La quadlet a la que apunta una unidad: su stem, su tipo y su nombre de fichero. */
export interface IdentidadDeQuadlet {
  /** Nombre sin extensión (`web`). */
  stem: string
  /** Tipo de quadlet deducido del sufijo de la unidad. */
  kind: TipoDeQuadlet
  /** Nombre de fichero completo (`web.container`): la clave de `PATCH`/`DELETE`. */
  file_name: string
}

/**
 * Traduce un nombre de unidad a la identidad de su quadlet.
 *
 * Devuelve `null` cuando el nombre no es una unidad de systemd (no acaba en
 * `.service` o no le queda stem): no se inventa una identidad con la que
 * accionar un fichero que no existe.
 */
export function identidadDeUnidad(nombreUnidad: string): IdentidadDeQuadlet | null {
  if (!nombreUnidad.endsWith(EXTENSION_DE_UNIDAD)) return null

  const base = nombreUnidad.slice(0, -EXTENSION_DE_UNIDAD.length)
  if (base === '') return null

  for (const [sufijo, kind] of SUFIJOS) {
    // El sufijo debe dejar stem: `-volume.service` no es el volumen de nada.
    if (base.length > sufijo.length && base.endsWith(sufijo)) {
      const stem = base.slice(0, -sufijo.length)
      return { stem, kind, file_name: `${stem}.${kind}` }
    }
  }

  return { stem: base, kind: 'container', file_name: `${base}.container` }
}
