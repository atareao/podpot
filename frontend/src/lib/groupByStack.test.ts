import type { ActiveState, TipoDeQuadlet } from '../types/api'
import { unidad } from '../../tests/fixtures'
import { groupByStack } from './groupByStack'

function u(
  name: string,
  stack: string,
  kind: TipoDeQuadlet | null = 'container',
  active_state: ActiveState = 'active',
) {
  return unidad({ name, file_name: name.replace(/\.service$/, '.container'), stack, kind, active_state })
}

describe('groupByStack', () => {
  it('devuelve una lista vacía para un listado vacío', () => {
    expect(groupByStack([])).toEqual([])
  })

  it('junta las unidades del mismo stack y ordena los stacks sin distinguir mayúsculas', () => {
    const unidades = [
      u('zeta.service', 'Zeta'),
      u('foo.service', 'foo'),
      u('foo-volume.service', 'foo', 'volume'),
      u('bar.service', 'bar'),
      u('foo-network.service', 'foo', 'network'),
    ]

    const grupos = groupByStack(unidades)

    expect(grupos.map((grupo) => grupo.stack)).toEqual(['bar', 'foo', 'Zeta'])
    expect(grupos[1]?.unidades.map((unidad) => unidad.name)).toEqual([
      'foo.service',
      'foo-network.service',
      'foo-volume.service',
    ])
    // La agrupación no altera la grafía que publica el backend.
    expect(grupos[2]?.stack).toBe('Zeta')
  })

  it('da su propio grupo a un auxiliar huérfano', () => {
    const grupos = groupByStack([
      u('backup.volume.service', 'backup', 'volume'),
      u('web.service', 'web'),
    ])

    expect(grupos.map((grupo) => grupo.stack)).toEqual(['backup', 'web'])
    expect(grupos[0]?.unidades).toHaveLength(1)
  })

  it('ordena principales antes que auxiliares y luego por nombre, de forma determinista', () => {
    const unidades = [
      u('z-volume.service', 'foo', 'volume'),
      u('a-network.service', 'foo', 'network'),
      u('z.service', 'foo'),
      u('a.service', 'foo', 'pod'),
    ]

    expect(groupByStack(unidades)[0]?.unidades.map((unidad) => unidad.name)).toEqual([
      'a.service',
      'z.service',
      'a-network.service',
      'z-volume.service',
    ])
  })

  it('no cambia el orden cuando cambia el estado de una unidad', () => {
    const unidades = [
      u('web.service', 'foo', 'container', 'inactive'),
      u('web-volume.service', 'foo', 'volume', 'active'),
    ]
    const antes = groupByStack(unidades).map((grupo) => grupo.unidades.map((unidad) => unidad.name))

    const cambiadas = unidades.map((unidad) =>
      unidad.name === 'web.service'
        ? { ...unidad, active_state: 'active' as const }
        : unidad,
    )

    expect(groupByStack(cambiadas).map((grupo) => grupo.unidades.map((unidad) => unidad.name))).toEqual(
      antes,
    )
  })

  it('no muta la entrada y conserva las referencias de las unidades', () => {
    const unidades = [u('foo.service', 'foo'), u('bar.service', 'bar'), u('foo-volume.service', 'foo', 'volume')]
    const original = unidades.map((unidad) => ({ ...unidad }))

    const grupos = groupByStack(unidades)

    expect(unidades).toEqual(original)
    expect(grupos.flatMap((grupo) => grupo.unidades)).toEqual(
      expect.arrayContaining([unidades[0], unidades[1], unidades[2]]),
    )
  })
})
