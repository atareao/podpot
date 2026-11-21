import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

// Panel de podpot. En desarrollo no hay CORS: Vite sirve el SPA y reenvía
// `/api/*` al backend en 127.0.0.1:8080 (el mismo puerto que usa el servidor
// Axum por defecto).
export default defineConfig({
  plugins: [react()],
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
  },
})
