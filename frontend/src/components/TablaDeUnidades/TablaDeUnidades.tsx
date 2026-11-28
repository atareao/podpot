import { useEffect, useMemo, useRef, useState } from 'react'
import { Empty, Select, Space, Spin } from 'antd'
import { GrupoDeUnidades } from '../GrupoDeUnidades/GrupoDeUnidades'
import { groupByStack } from '../../lib/groupByStack'
import type { TrabajosEnCurso } from '../../hooks/useTrabajosEnVivo'
import type {
  AccionDeUnidad,
  ActiveState,
  QuadletUnit,
  TipoDeQuadlet,
} from '../../types/api'

export interface TablaDeUnidadesProps {
  /** Quadlets a listar (pueden ser un subconjunto filtrado). */
  unidades: QuadletUnit[]
  /**
   * Lista completa, sin filtrar, que alimenta la comparación entre
   * instantáneas. Si no se aporta, se usa `unidades` (compatibilidad con usos
   * aislados del componente). La búsqueda cambia `unidades`, no esta prop: así
   * una unidad que reaparece no se confunde con un evento SSE.
   */
  instantanea?: QuadletUnit[]
  /** Mientras se pide la lista. */
  cargando: boolean
  /** Acción solicitada desde una tarjeta. */
  onAccion: (unidad: QuadletUnit, accion: AccionDeUnidad) => void
  /** Edición solicitada desde una tarjeta. */
  onEditar: (unidad: QuadletUnit) => void
  /** Apertura del visor de registros solicitada desde una tarjeta. */
  onVerRegistros: (unidad: QuadletUnit) => void
  /**
   * Borrado **confirmado** desde una tarjeta: la confirmación vive en el grupo,
   * así que cancelarla no llega a emitir nada.
   */
  onBorrar: (unidad: QuadletUnit) => void
  /** Inspección de campos sin entrar en edición. */
  onInspeccionar?: (unidad: QuadletUnit) => void
  /** Nombre de la unidad con una acción en curso: sus botones se deshabilitan. */
  enCurso?: string | null
  /**
   * Tipos de acción de los jobs en curso por unidad, del canal de trabajos.
   * Una unidad con al menos un `kind` muestra el indicador «en curso» y bloquea
   * sus acciones hasta que el job termina.
   */
  progresoPorUnidad?: TrabajosEnCurso
}

/** Duración del resaltado tras un cambio de estado informado por SSE. */
const DURACION_DE_RESALTADO_MS = 2000

/**
 * Listado de quadlets como grupos de tarjetas por stack. Sustituye a la tabla:
 * el contenedor conserva el nombre accesible «Quadlets» y cada tarjeta expone
 * el `file_name`, su estado y sus acciones. Presentacional: no habla con la API
 * ni con React Query —quien la usa (`pages/unidades.tsx`) es el que conecta—.
 */
export function TablaDeUnidades({
  unidades,
  instantanea,
  cargando,
  onAccion,
  onEditar,
  onVerRegistros,
  onBorrar,
  onInspeccionar,
  enCurso = null,
  progresoPorUnidad = {},
}: TablaDeUnidadesProps) {
  const [tipo, setTipo] = useState<TipoDeQuadlet | null>(null)
  const [estado, setEstado] = useState<ActiveState | null>(null)
  const [resaltadas, setResaltadas] = useState<ReadonlySet<string>>(() => new Set())
  const estadosPrevios = useRef<Map<string, ActiveState> | null>(null)
  const visibles = useRef(unidades)
  const fuenteDeEstados = instantanea ?? unidades

  // Se sincroniza en un efecto previo al del resaltado: cuando llega un evento
  // SSE en el mismo render en que cambian los filtros, la comparación usa las
  // tarjetas que ahora se muestran, no las del render anterior.
  useEffect(() => {
    visibles.current = unidades
  }, [unidades])

  // Un cambio de `active_state` entre instantáneas resalta la tarjeta durante
  // 2 s. El primer render no resalta nada: no hay instantánea previa que
  // comparar. Solo se resalta una clave que ya estaba en el mapa anterior; una
  // unidad que entra por un filtro no tiene evento SSE que informar. El
  // temporizador se cancela al llegar otro cambio o al desmontar.
  useEffect(() => {
    const estados = new Map(fuenteDeEstados.map((unidad) => [unidad.name, unidad.active_state]))
    const anteriores = estadosPrevios.current
    estadosPrevios.current = estados
    if (anteriores === null) return

    const cambiadas = visibles.current
      .filter(
        (unidad) =>
          anteriores.has(unidad.name) && anteriores.get(unidad.name) !== unidad.active_state,
      )
      .map((unidad) => unidad.name)
    // El cleanup de la instantánea anterior canceló el temporizador: si no
    // hubo cambios, hay que apagar el resaltado aquí o se queda para siempre.
    // La actualización funcional evita un render extra cuando ya está apagado y
    // no cambia las dependencias del efecto, así que no hay bucle.
    if (cambiadas.length === 0) {
      setResaltadas((actuales) => (actuales.size === 0 ? actuales : new Set<string>()))
      return
    }

    setResaltadas(new Set(cambiadas))
    const temporizador = window.setTimeout(() => setResaltadas(new Set()), DURACION_DE_RESALTADO_MS)
    return () => window.clearTimeout(temporizador)
  }, [fuenteDeEstados])

  // Derivación pura del estado actual: al llegar un evento SSE se recalcula con
  // el mismo orden determinista y los filtros siguen aplicados. Los grupos sin
  // coincidencias sencillamente no se pintan.
  const grupos = useMemo(
    () =>
      groupByStack(
        unidades.filter(
          (unidad) =>
            (tipo === null || unidad.kind === tipo) &&
            (estado === null || unidad.active_state === estado),
        ),
      ),
    [unidades, tipo, estado],
  )

  const { tiposDisponibles, estadosDisponibles } = useMemo(
    () => ({
      tiposDisponibles: [
        ...new Set(unidades.flatMap((unidad) => (unidad.kind === null ? [] : [unidad.kind]))),
      ].sort(),
      estadosDisponibles: [...new Set(unidades.map((unidad) => unidad.active_state))].sort(),
    }),
    [unidades],
  )

  const hayFiltros = tipo !== null || estado !== null
  const sinCoincidencias = grupos.length === 0 && !cargando

  return (
    <>
      <Space size="small" wrap style={{ marginBottom: 12 }}>
        <Select<TipoDeQuadlet>
          aria-label="Filtrar por tipo"
          allowClear
          placeholder="Filtrar por tipo"
          style={{ minWidth: 180 }}
          value={tipo ?? undefined}
          options={tiposDisponibles.map((valor) => ({ label: valor, value: valor }))}
          onChange={(valor) => setTipo(valor ?? null)}
        />
        <Select<ActiveState>
          aria-label="Filtrar por estado"
          allowClear
          placeholder="Filtrar por estado"
          style={{ minWidth: 180 }}
          value={estado ?? undefined}
          options={estadosDisponibles.map((valor) => ({ label: valor, value: valor }))}
          onChange={(valor) => setEstado(valor ?? null)}
        />
      </Space>

      <div role="list" aria-label="Quadlets" className="podpot-listado">
        {cargando ? (
          <div role="status" aria-label="Cargando quadlets">
            <Space size="small">
              <Spin size="small" aria-hidden="true" />
              <span>Cargando quadlets…</span>
            </Space>
          </div>
        ) : sinCoincidencias ? (
          <Empty
            description={
              hayFiltros ? 'No hay quadlets que coincidan con los filtros' : 'No hay quadlets'
            }
          />
        ) : (
          grupos.map((grupo) => (
            <GrupoDeUnidades
              key={grupo.stack}
              stack={grupo.stack}
              unidades={grupo.unidades}
              onAccion={onAccion}
              onEditar={onEditar}
              onVerRegistros={onVerRegistros}
              onBorrar={onBorrar}
              onInspeccionar={onInspeccionar}
              enCurso={enCurso}
              progresoPorUnidad={progresoPorUnidad}
              resaltadas={resaltadas}
            />
          ))
        )}
      </div>
    </>
  )
}
