import { Button, Flex, Typography } from 'antd'

/**
 * Portada pública: la primera pantalla que ve quien no tiene sesión.
 *
 * No hace peticiones propias. Solo se pinta cuando la guarda (`SoloConSesion`)
 * decide que hay proveedor de identidad y no hay sesión. El botón es un **enlace
 * real** a `/api/v1/auth/login`: el `302` al proveedor lo hace el servidor, no el
 * panel —no es un `fetch`, es una navegación del navegador—.
 */
export default function Portada() {
  return (
    <Flex
      vertical
      align="center"
      justify="center"
      gap={16}
      style={{ minHeight: '60vh', textAlign: 'center' }}
    >
      <Typography.Title level={1} style={{ margin: 0 }}>
        podpot
      </Typography.Title>
      <Typography.Paragraph style={{ maxWidth: 480, margin: 0 }}>
        Gestiona tus quadlets de Podman: consulta el estado de tus contenedores y créalos desde el
        panel. Entra con tu proveedor de identidad para continuar.
      </Typography.Paragraph>
      <Button type="primary" size="large" href="/api/v1/auth/login">
        Entrar
      </Button>
    </Flex>
  )
}
