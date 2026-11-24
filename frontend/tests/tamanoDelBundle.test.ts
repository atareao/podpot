// @vitest-environment node
import { build } from 'vite'
import { fileURLToPath } from 'node:url'

// El panel mide su huella de antd en el build de producción: los chunks
// atribuibles a antd (`vendor-antd*`) deben quedar por debajo del umbral de
// aviso de Vite (500 kB raw, como `chunkSizeWarningLimit`). Hoy superan el
// umbral (~834 kB raw entre `vendor-antd` y `vendor-antd-rc`), por lo que este
// test arranca en rojo y pasa una vez se reduzca la huella.
const UMBRAL_RAW = 500 * 1024 // 500 kB raw

const esChunkDeAntd = (nombre: string): boolean => /vendor-antd/.test(nombre)

it('la huella de antd en el bundle de producción está por debajo de 500 kB', async () => {
  const salida = await build({
    mode: 'production',
    configFile: fileURLToPath(new URL('../vite.config.ts', import.meta.url)),
    logLevel: 'silent',
    build: { write: false },
  })

  const chunks = Array.isArray(salida)
    ? salida.flatMap((resultado) => resultado.output)
    : salida.output

  const chunksDeAntd = chunks.filter((f) => f.type === 'chunk' && esChunkDeAntd(f.fileName))

  expect(chunksDeAntd.length).toBeGreaterThan(0)

  // Cada chunk antd por debajo del umbral.
  chunksDeAntd.forEach((f) => {
    const kB = (f.code.length / 1024).toFixed(2)
    expect(
      `${f.fileName}: ${kB} kB — un chunk de antd supera o toca el umbral de 500 kB`,
    )
    expect(f.code.length).toBeLessThan(UMBRAL_RAW)
  })

  // Huella combinada de antd por debajo del umbral.
  const bytes = chunksDeAntd.reduce((total, f) => total + f.code.length, 0)
  const desglose = chunksDeAntd
    .map((f) => `${f.fileName}: ${(f.code.length / 1024).toFixed(2)} kB`)
    .join(' · ')
  if (bytes > UMBRAL_RAW) {
    throw new Error(
      `La huella de antd pesa ${(bytes / 1024).toFixed(2)} kB (raw) y supera el umbral de 500 kB: ${desglose}`,
    )
  }
})
