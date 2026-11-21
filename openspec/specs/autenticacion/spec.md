# autenticacion Specification

## Purpose
El modo de autenticación de podpot: por defecto deshabilitado (dev en loopback), o mediante un proveedor OIDC con una sesión JWT en cookie y un gate que protege la superficie de negocio.

## Requirements

### Requirement: Modo de autenticación

La aplicación SHALL operar en modo `disabled` o `oidc`. En modo `disabled` SHALL servir toda la superficie sin autenticar (comportamiento actual). En modo `oidc` SHALL exigir una sesión válida para la superficie de negocio. El modo SHALL ser `disabled` salvo que estén definidas **todas** las variables del proveedor; una definición parcial SHALL ser un error de arranque que nombra la variable que falta. El estado de salud SHALL publicar el modo activo.

#### Scenario: Modo deshabilitado por defecto

- **WHEN** no hay ninguna variable OIDC definida
- **THEN** la aplicación SHALL arrancar en modo `disabled` y servir la superficie sin exigir sesión

#### Scenario: Configuración OIDC parcial

- **WHEN** se define alguna variable OIDC pero no todas
- **THEN** el arranque SHALL fallar nombrando la variable que falta, sin arrancar en un modo a medias

#### Scenario: Modo OIDC anunciado

- **WHEN** se consulta el estado de salud en modo `oidc`
- **THEN** SHALL indicar que el modo activo es `oidc`

### Requirement: Inicio de sesión OIDC

En modo `oidc`, la aplicación SHALL exponer `GET /api/v1/auth/login`, que SHALL redirigir (302) al proveedor con un `state` de un solo uso y un `nonce`. `GET /api/v1/auth/callback` SHALL validar el `state`, canjear el código, verificar el ID token (firma y `nonce`) y, si es válido, emitir la sesión en una cookie y redirigir (302) al panel. Un `state` desconocido, caducado o reutilizado, o un ID token inválido, SHALL rechazarse sin emitir sesión.

#### Scenario: Redirección al proveedor

- **WHEN** un cliente pide `GET /api/v1/auth/login`
- **THEN** SHALL recibir un `302` al `authorization_endpoint` del proveedor con `state` y `nonce`

#### Scenario: Callback correcto

- **WHEN** el proveedor redirige al callback con un código válido y el `state` esperado
- **THEN** SHALL emitir la cookie de sesión y redirigir (302) al panel

#### Scenario: State reutilizado o caducado

- **WHEN** llega un callback con un `state` desconocido, caducado o ya usado
- **THEN** SHALL rechazar la petición sin emitir sesión

#### Scenario: ID token inválido

- **WHEN** el ID token no verifica su firma o su `nonce`
- **THEN** SHALL rechazar la petición sin emitir sesión

### Requirement: Sesión en cookie

La sesión SHALL ser un JWT firmado con un secreto de la configuración, transportado en una cookie `HttpOnly` y `SameSite=Strict`, con `Secure` únicamente cuando el redirect sea https. La sesión SHALL caducar. `GET /api/v1/auth/session` SHALL devolver la identidad autenticada de la sesión actual, y `POST /api/v1/auth/logout` SHALL invalidarla.

#### Scenario: Identidad de la sesión

- **WHEN** un cliente con sesión válida pide `GET /api/v1/auth/session`
- **THEN** SHALL recibir la identidad (identificador y correo) de esa sesión

#### Scenario: Cierre de sesión

- **WHEN** un cliente pide `POST /api/v1/auth/logout`
- **THEN** SHALL eliminar la cookie de sesión y las peticiones posteriores SHALL quedar sin sesión

#### Scenario: Sesión caducada

- **WHEN** se presenta una sesión caducada
- **THEN** SHALL considerarse sin sesión

### Requirement: Protección de la superficie de negocio

En modo `oidc`, la aplicación SHALL exigir sesión válida para toda la superficie de negocio (unidades, acciones, quadlets, stream y las rutas de sesión/logout), y SHALL dejar públicas únicamente la salud, el login y el callback. Sin sesión, una petición a una ruta protegida SHALL responder `401` con la envoltura `{error}` habitual (nunca una redirección ni HTML). El stream SSE SHALL quedar protegido por el mismo mecanismo, de modo que sin sesión no se abra el flujo.

#### Scenario: Petición sin sesión

- **WHEN** en modo `oidc` se pide una ruta de negocio sin cookie de sesión válida
- **THEN** SHALL responder `401` con la envoltura `{error}`, sin redirigir

#### Scenario: Rutas públicas

- **WHEN** en modo `oidc` se piden la salud, el login o el callback
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
