/**
 * Claves de caché de React Query. Viven en un solo sitio para que invalidar sea
 * una operación deliberada y no una cadena escrita a mano en cada mutación.
 */
export const claves = {
  units: ['units'] as const,
  /** Definición de una quadlet concreta, por nombre de fichero. */
  quadlet: (fileName: string) => ['quadlet', fileName] as const,
  /** `GET /health` — versión y modo de autenticación (`auth_mode`). */
  salud: ['salud'] as const,
  /** `GET /auth/session` — la identidad de la sesión actual. */
  sesion: ['sesion'] as const,
}
