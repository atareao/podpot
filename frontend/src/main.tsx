import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { QueryClientProvider } from '@tanstack/react-query'
import { App as AppDeAntd } from 'antd'
import App from './App'
import { crearClienteDeConsultas } from './api/clienteDeConsultas'
import './estilos.css'

/**
 * Arranque del panel: busca `#root` y monta la raíz de providers —consultas,
 * contexto de Ant Design (para `message`/`notification`) y router—. Sin
 * componentes definidos aquí: el rule de Fast Refresh pide que un módulo sin
 * exports de componentes no declare componentes.
 */

const contenedor = document.getElementById('root')
if (!contenedor) {
  throw new Error('No se encontró el elemento #root en el documento')
}

const cliente = crearClienteDeConsultas()

createRoot(contenedor).render(
  <StrictMode>
    <QueryClientProvider client={cliente}>
      <AppDeAntd>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </AppDeAntd>
    </QueryClientProvider>
  </StrictMode>
)
