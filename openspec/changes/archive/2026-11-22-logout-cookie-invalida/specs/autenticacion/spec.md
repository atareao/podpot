# Spec Delta — autenticacion

## MODIFIED Requirements

### Requirement: Sesión en cookie

La sesión SHALL ser un JWT firmado con un secreto de la configuración, transportado en una cookie `HttpOnly` y `SameSite=Strict`, con `Secure` únicamente cuando el redirect sea https. La sesión SHALL caducar. `GET /api/v1/auth/session` SHALL devolver la identidad autenticada de la sesión actual. `POST /api/v1/auth/logout` SHALL borrar la cookie de sesión siempre, con independencia de que la cookie presentada verifique o no.

#### Scenario: Identidad de la sesión

- **WHEN** un cliente con sesión válida pide `GET /api/v1/auth/session`
- **THEN** SHALL recibir la identidad (identificador y correo) de esa sesión

#### Scenario: Cierre de sesión

- **WHEN** un cliente pide `POST /api/v1/auth/logout`
- **THEN** SHALL eliminar la cookie de sesión y las peticiones posteriores SHALL quedar sin sesión

#### Scenario: Cierre de sesión con cookie inválida

- **WHEN** un cliente pide `POST /api/v1/auth/logout` con una cookie de sesión que no verifica
- **THEN** SHALL responder con éxito y SHALL eliminar la cookie de sesión

#### Scenario: Cierre de sesión sin cookie

- **WHEN** un cliente pide `POST /api/v1/auth/logout` sin cookie de sesión
- **THEN** SHALL responder con éxito y SHALL emitir la cookie de borrado

#### Scenario: Sesión caducada

- **WHEN** se presenta una sesión caducada
- **THEN** SHALL considerarse sin sesión

### Requirement: Protección de la superficie de negocio

En modo `oidc`, la aplicación SHALL exigir sesión válida para toda la superficie de negocio (unidades, acciones, quadlets, stream y la ruta de sesión), y SHALL dejar públicas únicamente la salud, el login, el callback y el logout. Sin sesión, una petición a una ruta protegida SHALL responder `401` con la envoltura `{error}` habitual (nunca una redirección ni HTML). El stream SSE SHALL quedar protegido por el mismo mecanismo, de modo que sin sesión no se abra el flujo.

#### Scenario: Petición sin sesión

- **WHEN** en modo `oidc` se pide una ruta de negocio sin cookie de sesión válida
- **THEN** SHALL responder `401` con la envoltura `{error}`, sin redirigir

#### Scenario: Rutas públicas

- **WHEN** en modo `oidc` se piden la salud, el login, el callback o el logout
- **THEN** SHALL atenderse sin exigir sesión

#### Scenario: Stream sin sesión

- **WHEN** en modo `oidc` se pide `GET /api/v1/units/stream` sin sesión válida
- **THEN** SHALL responder `401` con la envoltura `{error}` y NO SHALL abrir el flujo de eventos

#### Scenario: Stream con sesión

- **WHEN** en modo `oidc` se pide `GET /api/v1/units/stream` con sesión válida
- **THEN** SHALL abrir el flujo con normalidad

#### Scenario: Modo deshabilitado no protege

- **WHEN** la aplicación está en modo `disabled`
- **THEN** la superficie de negocio SHALL seguir accesible sin sesión
