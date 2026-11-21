/**
 * El estado de la sesión del panel, listo para pintar.
 *
 * La consulta de salud decide el modo; la de sesión solo se lanza en `oidc`. Un
 * `401` **no se reintenta** ni se relee al montar otro observador: es una
 * respuesta definitiva («no hay sesión»), no un fallo pasajero, y reintentarla
 * sería un bucle de peticiones. Un `401` de **cualquier** petición invalida esta
 * consulta (`clienteDeConsultas`), y la guarda (`SoloConSesion`) vuelve a la
 * portada.
 */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import type { UseMutationResult, UseQueryResult } from '@tanstack/react-query'
import { cerrarLaSesion, obtenerSalud, obtenerSesion } from '../api/sesion'
import { esErrorDeSesion } from '../api/envelope'
import { claves } from './claves'
import type { Health, ModoDeAutenticacion, Sesion } from '../types/api'

export interface EstadoDeSesion {
  /** `null` mientras `/health` no ha contestado. */
  modo: ModoDeAutenticacion | null
  sesion: Sesion | null
  cargando: boolean
  /** Hay proveedor de identidad y no hay sesión: toca enseñar el botón de entrar. */
  sinSesion: boolean
  error: Error | null
  /** Vuelve a preguntar la sesión (por ejemplo, tras entrar en otra pestaña). */
  refrescar: () => void
}

/** `/health` no cambia durante la vida de la pestaña: no hace falta repetirlo. */
export function useSalud(): UseQueryResult<Health, Error> {
  return useQuery({
    queryKey: claves.salud,
    queryFn: obtenerSalud,
    staleTime: Number.POSITIVE_INFINITY,
    retry: false,
  })
}

/**
 * El estado de la sesión del panel. En modo `disabled` devuelve
 * `modo: 'disabled'` y **no** llama a `/auth/*`.
 */
export function useSesion(): EstadoDeSesion {
  const cliente = useQueryClient()
  const salud = useSalud()
  const modo = salud.data?.auth_mode ?? null

  const consultaDeSesion = useQuery({
    queryKey: claves.sesion,
    queryFn: obtenerSesion,
    enabled: modo === 'oidc',
    // Un `401` es una respuesta, no un fallo pasajero: ni se reintenta…
    retry: false,
    // …ni se relee al montar otro observador. Sin esto, al volver a la portada
    // se remonta la guarda y la relectura volvería a pedir la sesión: dos
    // peticiones donde la spec quiere una.
    retryOnMount: false,
    // Con sesión válida tampoco se relee por el mero hecho de que otro
    // componente la observe: los cambios de sesión son por evento, no por montaje.
    refetchOnMount: false,
  })

  // Un `401` no es un error que enseñar: es «no hay sesión».
  const esSinSesion = esErrorDeSesion(consultaDeSesion.error)
  const error = salud.error ?? consultaDeSesion.error ?? null

  return {
    modo,
    // Tras un `401`, TanStack Query conserva el `data` anterior: `sinSesion`
    // manda y la sesión se publica como ausente, no obsoleta.
    sesion: esSinSesion ? null : (consultaDeSesion.data ?? null),
    cargando: salud.isPending || (modo === 'oidc' && consultaDeSesion.isPending),
    sinSesion: esSinSesion,
    error: esSinSesion ? null : error,
    refrescar: () => {
      void cliente.invalidateQueries({ queryKey: claves.sesion })
    },
  }
}

/** Cierra la sesión y vuelve a leerla, que es lo que deja `sinSesion: true`. */
export function useCerrarSesion(): UseMutationResult<null, Error, void> {
  const cliente = useQueryClient()

  return useMutation<null, Error, void>({
    mutationFn: cerrarLaSesion,
    onSuccess: () => {
      void cliente.invalidateQueries({ queryKey: claves.sesion })
    },
  })
}
