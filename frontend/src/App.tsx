import { Route, Routes } from 'react-router-dom'
import Unidades from './pages/unidades'

/**
 * Las rutas del panel. De momento hay una sola pantalla —el listado de
 * quadlets—; cualquier ruta cae en ella. El `BrowserRouter` vive en `main.tsx`.
 */
export default function App() {
  return (
    <Routes>
      <Route path="/" element={<Unidades />} />
      <Route path="*" element={<Unidades />} />
    </Routes>
  )
}
