# Spec Delta — autenticacion

## MODIFIED Requirements

### Requirement: Sesión en cookie

La sesión SHALL ser un JWT firmado con un secreto de la configuración, transportado en una cookie `HttpOnly` y `SameSite=Strict`, con `Secure` únicamente cuando el redirect sea https. Cada token de sesión SHALL caducar; la sesión puede renovarse de forma deslizante sin tope absoluto de vida. `GET /api/v1/auth/session` SHALL devolver la identidad autenticada de la sesión actual. `POST /api/v1/auth/logout` SHALL borrar la cookie de sesión siempre, con independencia de que la cookie presentada verifique o no. En modo `oidc`, el *gate* SHALL renovar la sesión de forma deslizante cuando a un token válido le queden la ventana de renovación o menos. SHALL emitir una cookie nueva con la misma identidad (`sub` y `email`), con `iat` y `exp` nuevos y con la vida completa. Un token fuera de la ventana NO SHALL renovarse. Salvo en el caso de refresh rechazado por el proveedor descrito en el requisito «Refresco del ID token del proveedor», la renovación NO SHALL cambiar el cuerpo ni el código de la respuesta.

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

#### Scenario: Sesión revocada por el proveedor

- **WHEN** el proveedor rechaza el `refresh_token` durante la renovación
- **THEN** SHALL recibir `401` con la envoltura y SHALL borrarse la cookie de sesión

#### Scenario: Modo deshabilitado

- **WHEN** la aplicación está en modo `disabled`
- **THEN** NO SHALL haber renovación de sesión

## ADDED Requirements

### Requirement: Refresco del ID token del proveedor

En modo `oidc`, cuando el canje del código devuelve un `refresh_token`, la aplicación SHALL asociarlo a la sesión emitida mediante un identificador `jti` y conservarlo **solo en memoria**. El `refresh_token` NO SHALL exponerse al cliente ni persistirse en disco; se pierde al reiniciar el proceso. Durante la renovación deslizante, si la sesión tiene `jti` y hay `refresh_token` asociado, la aplicación SHALL solicitar un ID token nuevo al `token_endpoint` con `grant_type=refresh_token` antes de emitir la cookie. El ID token nuevo SHALL verificarse con la misma exigencia que el del callback, salvo el `nonce`, que puede no venir en un refresh. Si el proveedor devuelve un `refresh_token` nuevo, SHALL sustituir al anterior. Si el proveedor responde `invalid_grant`, la aplicación SHALL descartar el `refresh_token` y considerar la sesión sin sesión. Si el refresh falla por red o por un error del proveedor distinto de `invalid_grant`, la aplicación SHALL mantener la sesión local y renovarla como hasta ahora. Sin `refresh_token`, la renovación local NO SHALL cambiar.

#### Scenario: Captura del refresh_token en el callback

- **WHEN** el canje del código devuelve un `refresh_token`
- **THEN** la sesión emitida SHALL llevar un `jti` y el `refresh_token` SHALL quedar asociado a ese `jti`

#### Scenario: Renovación con refresh_token válido

- **WHEN** una sesión con `jti` y `refresh_token` entra en la ventana de renovación
- **THEN** SHALL realizarse una petición al `token_endpoint` con `grant_type=refresh_token`
- **AND** el ID token nuevo SHALL verificarse y SHALL emitirse una cookie nueva con la misma identidad

#### Scenario: Rotación del refresh_token

- **WHEN** el proveedor devuelve un `refresh_token` nuevo en el refresh
- **THEN** SHALL sustituirse el anterior asociado al mismo `jti`

#### Scenario: Refresh rechazado por el proveedor

- **WHEN** el proveedor responde `invalid_grant` al refresh
- **THEN** SHALL descartarse el `refresh_token` y la petición SHALL recibir `401` con la cookie de sesión borrada

#### Scenario: Proveedor no disponible durante el refresh

- **WHEN** el refresh falla por red o por un error del proveedor distinto de `invalid_grant`
- **THEN** SHALL mantenerse la sesión local y renovarse como hasta ahora

#### Scenario: Sesión sin refresh_token

- **WHEN** la sesión no tiene `refresh_token` asociado
- **THEN** la renovación local SHALL comportarse como hasta ahora
