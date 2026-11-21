import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { accionDeUnidad, listarUnidades } from '../api/units'
import { borrarQuadlet, crearQuadlet, editarQuadlet, obtenerQuadlet } from '../api/quadlets'
import type {
  AccionDeUnidad,
  PeticionDeCreacion,
  QuadletAfectada,
  QuadletCreada,
  QuadletDetalle,
} from '../types/api'

/**
 * Claves de caché de React Query. Viven en un solo sitio para que invalidar sea
 * una operación deliberada y no una cadena escrita a mano en cada mutación.
 */
export const claves = {
  units: ['units'] as const,
  /** Definición de una quadlet concreta, por nombre de fichero. */
  quadlet: (fileName: string) => ['quadlet', fileName] as const,
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

/**
 * Edición de una quadlet existente. Se identifica por el nombre de **fichero**
 * (`web.container`), no por el de unidad, y el cuerpo es el struct tipado
 * completo del tipo. Al terminar con éxito el listado se vuelve a pedir: la
 * recarga de systemd es lo que hace visible el cambio, no la respuesta. La
 * definición cacheada también se tira, que ya no describe el fichero.
 */
export function useEditarQuadlet() {
  const cliente = useQueryClient()
  return useMutation({
    mutationFn: ({
      nombre,
      datos,
    }: {
      nombre: string
      datos: PeticionDeCreacion
    }): Promise<QuadletAfectada> => editarQuadlet(nombre, datos),
    onSuccess: (_afectada, variables) => {
      void cliente.invalidateQueries({ queryKey: claves.quadlet(variables.nombre) })
      return cliente.invalidateQueries({ queryKey: claves.units })
    },
  })
}

/**
 * Borrado de una quadlet existente, por nombre de fichero. Como en la edición,
 * el listado se vuelve a pedir al terminar con éxito: quien lo manda es el
 * fichero, pero lo que se pinta es el estado que reporta systemd.
 */
export function useBorrarQuadlet() {
  const cliente = useQueryClient()
  return useMutation({
    mutationFn: (nombre: string): Promise<QuadletAfectada> => borrarQuadlet(nombre),
    onSuccess: (_afectada, nombre) => {
      void cliente.invalidateQueries({ queryKey: claves.quadlet(nombre) })
      return cliente.invalidateQueries({ queryKey: claves.units })
    },
  })
}

/**
 * Verificación de la existencia del fichero antes de una operación destructiva.
 *
 * La identidad de `DELETE`/`PATCH` es el nombre de fichero que publica la API
 * (`unit.file_name`), así que ya no hay deducción que desambiguar. Aun así, el
 * listado es una foto: entre pintarlo y borrar, el fichero pudo desaparecer o
 * cambiar fuera del panel. Antes de borrar se pide la definición del fichero: si
 * no existe, la lectura da `404` y el borrado no llega a enviarse. Se hace con
 * `fetchQuery` —no con un `useQuery` montado— porque es una comprobación
 * imperativa, puntual, y así comparte caché con la lectura de la edición
 * (`claves.quadlet`).
 */
export function useVerificarQuadlet() {
  const cliente = useQueryClient()
  return useMutation({
    mutationFn: (nombre: string): Promise<QuadletDetalle> =>
      cliente.fetchQuery({
        queryKey: claves.quadlet(nombre),
        queryFn: ({ signal }): Promise<QuadletDetalle> => obtenerQuadlet(nombre, signal),
        staleTime: 0,
      }),
  })
}

/**
 * Definición de una quadlet, para precargar el formulario de edición.
 *
 * Se pide al abrir la edición (`fileName` no nulo) y nunca antes: la lista no
 * trae los campos. `staleTime` a cero porque el fichero puede haber cambiado
 * fuera del panel desde la última lectura y la precarga tiene que ser la real.
 */
export function useDetalleDeQuadlet(fileName: string | null) {
  const nombre = fileName ?? ''
  return useQuery({
    queryKey: claves.quadlet(nombre),
    queryFn: ({ signal }): Promise<QuadletDetalle> => obtenerQuadlet(nombre, signal),
    enabled: nombre !== '',
    staleTime: 0,
  })
}
