# Spec Delta — autenticacion

## MODIFIED Requirements

### Requirement: Sesión en cookie

La sesión SHALL ser un JWT firmado con un secreto de la configuración, transportado en una cookie `HttpOnly` y `SameSite=Strict`, con `Secure` únicamente cuando el redirect sea https. Cada token de sesión SHALL caducar; la sesión puede renovarse de forma deslizante sin tope absoluto de vida. `GET /api/v1/auth/session` SHALL devolver la identidad autenticada de la sesión actual. `POST /api/v1/auth/logout` SHALL borrar la cookie de sesión siempre, con independencia de que la cookie presentada verifique o no. En modo `oidc`, el *gate* SHALL renovar la sesión de forma deslizante cuando a un token válido le queden la ventana de renovación o menos. SHALL emitir una cookie nueva con la misma identidad (`sub` y `email`), con `iat` y `exp` nuevos y con la vida completa. Un token fuera de la ventana NO SHALL renovarse. La renovación NO SHALL cambiar el cuerpo ni el código de la respuesta.

#### Scenario: Cierre de sesión con epoch

- **WHEN** un cliente pide `POST /api/v1/auth/logout`
- **THEN** SHALL eliminar la cookie de sesión y SHALL establecer un `logout_epoch` futuro en la cookie de borrado

#### Scenario: Renovación rechazada por logout_epoch

- **WHEN** un cliente con sesión válida pide una ruta de negocio y el token tiene un `logout_epoch` menor o igual al actual
- **THEN** SHALL recibir `401` con la envoltura y NO SHALL recibir una `Set-Cookie` de renovación

#### Scenario: Renovación conserva logout_epoch

- **WHEN** un cliente con sesión válida pide una ruta de negocio y el token tiene un `logout_epoch` establecido
- **THEN** SHALL recibir `200` con la misma respuesta y una `Set-Cookie` con una sesión nueva
- **AND** la sesión nueva SHALL conservar el mismo `logout_epoch` que el token original

#### Scenario: Renovación sin logout_epoch

- **WHEN** un cliente con sesión válida pide una ruta de negocio y el token NO tiene `logout_epoch`
- **THEN** SHALL recibir `200` con la misma respuesta y una `Set-Cookie` con una sesión nueva
- **AND** la sesión nueva NO SHALL tener `logout_epoch`
