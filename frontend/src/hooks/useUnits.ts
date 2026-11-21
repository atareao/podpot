import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { accionDeUnidad, listarUnidades } from '../api/units'
import { crearQuadlet } from '../api/quadlets'
import type { AccionDeUnidad, PeticionDeCreacion, QuadletCreada } from '../types/api'

/**
 * Claves de caché de React Query. Viven en un solo sitio para que invalidar sea
 * una operación deliberada y no una cadena escrita a mano en cada mutación.
 */
export const claves = {
  units: ['units'] as const,
}

/** Listado de quadlets con su estado. */
export function useUnits() {
  return useQuery({
    queryKey: claves.units,
    queryFn: ({ signal }) => listarUnidades(signal),
  })
}

/**
 * Acción de control (`start`/`stop`/`restart`). Al terminar con éxito, el
 * listado se vuelve a pedir: el estado autoritativo lo tiene systemd, no la
 * respuesta de la acción.
 */
export function useAccionDeUnidad() {
  const cliente = useQueryClient()
  return useMutation({
    mutationFn: ({ name, accion }: { name: string; accion: AccionDeUnidad }) =>
      accionDeUnidad(name, accion),
    onSuccess: () => cliente.invalidateQueries({ queryKey: claves.units }),
  })
}

/**
 * Creación de una quadlet. Al terminar con éxito, el listado se vuelve a pedir:
 * la quadlet recién creada solo aparece cuando systemd la conoce tras la
 * recarga, así que la respuesta de la creación no basta para pintar la tabla.
 */
export function useCrearQuadlet() {
  const cliente = useQueryClient()
  return useMutation({
    mutationFn: (datos: PeticionDeCreacion): Promise<QuadletCreada> => crearQuadlet(datos),
    onSuccess: () => cliente.invalidateQueries({ queryKey: claves.units }),
  })
}
