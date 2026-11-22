# Spec Delta — autenticacion

## MODIFIED Requirements

### Requirement: Sesión en cookie

La sesión SHALL ser un JWT firmado con un secreto de la configuración, transportado en una cookie `HttpOnly` y `SameSite=Strict`, con `Secure` únicamente cuando el redirect sea https. Cada token de sesión SHALL caducar; la sesión puede renovarse de forma deslizante sin tope absoluto de vida. `GET /api/v1/auth/session` SHALL devolver la identidad autenticada de la sesión actual. `POST /api/v1/auth/logout` SHALL borrar la cookie de sesión siempre, con independencia de que la cookie presentada verifique o no. En modo `oidc`, el *gate* SHALL renovar la sesión de forma deslizante cuando a un token válido le queden la ventana de renovación o menos. SHALL emitir una cookie nueva con la misma identidad (`sub` y `email`), con `iat` y `exp` nuevos y con la vida completa. Un token fuera de la ventana NO SHALL renovarse. La renovación NO SHALL cambiar el cuerpo ni el código de la respuesta.

#### Scenario: Identidad de la sesión

- **WHEN** un cliente con sesión válida pide `GET /api/v1/auth/session`
- **THEN** SHALL recibir la identidad (identificador y correo) de esa sesión

#### Scenario: Cierre de sesión

- **WHEN** un cliente pide `POST /api/v1/auth/logout`
- **THEN** SHALL eliminar la cookie de sesión y las peticiones posteriores SHALL quedar sin sesión

#### Scenario: Cierre de sesión con cookie inválida

- **WHEN** un cliente pide `POST /api/v1/auth/logout` con una cookie de sesión que no verifica
- **THEN** SHALL responder con éxito y SHALL borrar la cookie de sesión

#### Scenario: Cierre de sesión sin cookie

- **WHEN** un cliente pide `POST /api/v1/auth/logout` sin cookie de sesión
- **THEN** SHALL responder con éxito y SHALL emitir la cookie de borrado

#### Scenario: Sesión caducada

- **WHEN** se presenta una sesión caducada
- **THEN** SHALL considerarse sin sesión

#### Scenario: Renovación dentro de la ventana

- **WHEN** un cliente con sesión válida pide una ruta de negocio y al token le quedan la ventana de renovación o menos
- **THEN** SHALL recibir `200` con la misma respuesta y una `Set-Cookie` con una sesión nueva
- **AND** la sesión nueva SHALL conservar el mismo `sub` y `email` y un `exp` posterior

#### Scenario: Sesión reciente sin renovación

- **WHEN** un cliente con sesión válida pide una ruta de negocio y al token le quedan más de la ventana de renovación
- **THEN** SHALL recibir `200` sin `Set-Cookie` de renovación

#### Scenario: Sesión inválida sin renovación

- **WHEN** un cliente pide una ruta de negocio con una cookie caducada o con firma inválida
- **THEN** SHALL recibir `401` con la envoltura y NO SHALL recibir una `Set-Cookie` de renovación

#### Scenario: Modo deshabilitado

- **WHEN** la aplicación está en modo `disabled`
- **THEN** NO SHALL haber renovación de sesión
