// @vitest-environment node
import { build } from 'vite'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

// Presupuesto por chunk individual: Vite separa `antd` de sus dependencias
// `rc-*` en chunks `vendor-antd*`, y cada uno debe quedar por debajo del
// umbral raw de 500 kB. No se suman tamaños entre chunks.
const UMBRAL_RAW = 500 * 1024

const esChunkDeAntd = (nombre: string): boolean => /^vendor-antd/.test(nombre)

// `build()` puede devolver un watcher: solo los resultados con `output`
// contienen ficheros. Tipado mínimo para no depender de `rollup` como
// dependencia directa (Vite 7 no reexporta `OutputChunk`/`RollupOutput`).
interface ArchivoSalida {
  type: string
  fileName: string
  code?: string
}

interface ChunkSalida extends ArchivoSalida {
  type: 'chunk'
  code: string
}

type SalidaBuild = Awaited<ReturnType<typeof build>>

// Rollup entrega la ruta completa (`assets/vendor-antd-*.js`); el nombre
// lógico del chunk es el último segmento.
const nombreLogico = (ruta: string): string => ruta.split('/').pop() ?? ruta

const chunksDeAntd = (salida: SalidaBuild): ChunkSalida[] => {
  const resultados = Array.isArray(salida) ? salida : [salida]
  const archivos: ArchivoSalida[] = resultados.flatMap((resultado) =>
    'output' in resultado ? resultado.output : [],
  )

  return archivos.filter(
    (f): f is ChunkSalida => f.type === 'chunk' && esChunkDeAntd(nombreLogico(f.fileName)),
  )
}

const verificarPresupuesto = (chunks: ChunkSalida[]): void => {
  chunks.forEach((f) => {
    expect(f.code.length, `${f.fileName}: ${(f.code.length / 1024).toFixed(2)} kB`).toBeLessThan(
      UMBRAL_RAW,
    )
  })
}

it('mantiene antd y @ant-design/icons como dependencias declaradas', () => {
  const contenido: unknown = JSON.parse(
    readFileSync(fileURLToPath(new URL('../package.json', import.meta.url)), 'utf8'),
  )
  const paquete = contenido as { dependencies: Record<string, string> }

  expect(paquete.dependencies.antd).toBeDefined()
  expect(paquete.dependencies['@ant-design/icons']).toBeDefined()
})

it('cada chunk de antd en el bundle de producción está por debajo de 500 kB', async () => {
  const salida: SalidaBuild = await build({
    mode: 'production',
    configFile: fileURLToPath(new URL('../vite.config.ts', import.meta.url)),
    logLevel: 'silent',
    build: { write: false },
  })

  const chunks = chunksDeAntd(salida)

  expect(chunks.length).toBeGreaterThan(0)
  verificarPresupuesto(chunks)
})

it('informa del nombre y tamaño en kB si un chunk de antd alcanza el umbral', () => {
  const chunkEnElLimite: ChunkSalida = {
    type: 'chunk',
    fileName: 'assets/vendor-antd-Grande.js',
    code: 'x'.repeat(UMBRAL_RAW),
  }

  expect(() => verificarPresupuesto([chunkEnElLimite])).toThrow(/vendor-antd.*kB/)
})
