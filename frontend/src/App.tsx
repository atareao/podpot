import { Route, Routes } from 'react-router-dom'
import { SoloConSesion } from './components/SoloConSesion/SoloConSesion'
import Unidades from './pages/unidades'

/**
 * Las rutas del panel. De momento hay una sola pantalla —el listado de
 * quadlets—; cualquier ruta cae en ella, pero **tras la guarda de sesión**: en
 * modo `oidc` sin sesión, `SoloConSesion` muestra la portada pública; en modo
 * `disabled` deja pasar (dev). El `BrowserRouter` vive en `main.tsx`.
 */
export default function App() {
  return (
    <Routes>
      <Route
        path="/"
        element={
          <SoloConSesion>
            <Unidades />
          </SoloConSesion>
        }
      />
      <Route
        path="*"
        element={
          <SoloConSesion>
            <Unidades />
          </SoloConSesion>
        }
      />
    </Routes>
  )
}
