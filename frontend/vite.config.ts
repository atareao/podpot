import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

// Panel de podpot. En desarrollo no hay CORS: Vite sirve el SPA y reenvía
// `/api/*` al backend en 127.0.0.1:8080 (el mismo puerto que usa el servidor
// Axum por defecto).

// Reparto del bundle de producción por proveedor: antd, React y React Query
// viajan en chunks propios y estables, de modo que un cambio en el código de
// la aplicación no invalida su caché. El resto de dependencias caen en `vendor`
// para que en el chunk de entrada solo quede el código de la aplicación.
const RE_ANTD = /node_modules\/antd\//
const RE_ANTD_RC =
  /node_modules\/(@ant-design|@rc-component|rc-[^/]+)\//
const RE_QUERY = /node_modules\/@tanstack\//
const REACT_PACKAGES = [
  'react',
  'react-dom',
  'react-router',
  'react-router-dom',
  'scheduler',
]

const vendorChunk = (id: string): string | undefined => {
  if (!id.includes('node_modules')) return undefined
  if (RE_ANTD.test(id)) return 'vendor-antd'
  if (RE_ANTD_RC.test(id)) return 'vendor-antd-rc'
  if (RE_QUERY.test(id)) return 'vendor-query'
  if (REACT_PACKAGES.some((pkg) => id.includes(`node_modules/${pkg}/`))) {
    return 'vendor-react'
  }
  return 'vendor'
}

export default defineConfig({
  plugins: [react()],
  build: {
    minify: 'terser',
    terserOptions: {
      compress: {
        passes: 2,
        drop_console: true,
        drop_debugger: true,
        pure_funcs: ['console.log', 'console.info', 'console.debug'],
      },
      mangle: {
        safari10: true,
      },
    },
    rollupOptions: {
      output: {
        manualChunks: vendorChunk,
      },
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
      },
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: './tests/setupTests.ts',
    globals: true,
    css: true,
    testTimeout: 60000,
  },
})
