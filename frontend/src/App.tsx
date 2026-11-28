import { Route, Routes } from 'react-router-dom'
import { Button, ConfigProvider, Space, theme, Typography } from 'antd'
import { MoonOutlined, SunOutlined } from '@ant-design/icons'
import { ErrorBoundary } from './components/ErrorBoundary/ErrorBoundary'
import { SoloConSesion } from './components/SoloConSesion/SoloConSesion'
import { useCerrarSesion, useSesion } from './hooks/useSesion'
import { useTema } from './hooks/useTema'
import Unidades from './pages/unidades'

/**
 * Las rutas del panel, tras una cabecera fija con la marca, el usuario y el
 * control de salida. No hay barra lateral: el panel es una sola vista. El
 * `BrowserRouter` vive en `main.tsx`; aquí solo se dibuja la estructura y se
 * envuelve todo en `ErrorBoundary` y en el tema elegido.
 */
export default function App() {
  const { sesion } = useSesion()
  const cerrarSesion = useCerrarSesion()
  const { tema, alternarTema } = useTema()

  return (
    <ConfigProvider
      theme={{ algorithm: tema === 'dark' ? theme.darkAlgorithm : theme.defaultAlgorithm, cssVar: true }}
    >
      <header className="podpot-cabecera">
        <div className="podpot-cabecera__contenido">
          <Typography.Text strong className="podpot-marca">
            podpot
          </Typography.Text>
          <Space size="small" wrap className="podpot-cabecera__acciones">
            {sesion !== null && (
              <Typography.Text type="secondary" className="podpot-usuario">
                {sesion.email !== '' ? sesion.email : sesion.sub}
              </Typography.Text>
            )}
            <Button
              type="text"
              size="small"
              aria-label={tema === 'dark' ? 'Cambiar a tema claro' : 'Cambiar a tema oscuro'}
              icon={tema === 'dark' ? <SunOutlined /> : <MoonOutlined />}
              onClick={alternarTema}
            />
            <Button size="small" onClick={() => cerrarSesion.mutate()}>
              Cerrar sesión
            </Button>
          </Space>
        </div>
      </header>

      <main className="podpot-contenido">
        <ErrorBoundary>
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
        </ErrorBoundary>
      </main>
    </ConfigProvider>
  )
}
