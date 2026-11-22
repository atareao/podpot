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

### Requirement: Caché del JWKS con frescura acotada

En modo `oidc`, el conjunto de claves públicas del proveedor (`JWKS`) SHALL almacenarse en caché tras la primera lectura y reutilizarse en los callbacks posteriores, de manera que NO SHALL realizarse una petición al `jwks_uri` en cada callback. La caché SHALL considerarse fresca solo durante un TTL acotado; transcurrido el TTL, el siguiente callback SHALL volver a solicitar el `jwks_uri`. Si el ID token presenta un `kid` que el conjunto cacheado no publica, la aplicación SHALL refrescar el `jwks_uri` (una sola vez) antes de rechazar el token. El contrato HTTP NO SHALL cambiar: un ID token que no verifica SHALL seguir rechazándose con `401` y con el mismo motivo genérico, y un fallo del proveedor al refrescar el `jwks_uri` SHALL desenlazar en `BadGateway` (`502`) con la envoltura existente `{error:{code,message}}`, sin exponer la causa subyacente.

#### Scenario: Reutilización dentro del TTL

- **WHEN** dos callbacks válidos llegan dentro del TTL de la caché
- **THEN** SHALL realizarse una sola petición al `jwks_uri`
- **AND** el segundo callback SHALL verificar el ID token con el conjunto cacheado

#### Scenario: Expiración del TTL

- **WHEN** el TTL de la caché ha transcurrido y llega un callback válido
- **THEN** SHALL solicitarse de nuevo el `jwks_uri`
- **AND** la caché SHALL sustituirse por el conjunto recién leído

#### Scenario: Rotación de claves con un kid que la caché no publica

- **WHEN** el ID token viene firmado con un `kid` que el conjunto cacheado no publica
- **AND** el refresco del `jwks_uri` publica esa clave
- **THEN** SHALL aceptarse el ID token y emitirse la sesión

#### Scenario: Proveedor caído al refrescar el JWKS

- **WHEN** el refresco del `jwks_uri` falla (red, estado no exitoso o cuerpo no válido)
- **THEN** la respuesta SHALL ser `502` con el código `BadGateway` y la envoltura existente
- **AND** la causa subyacente NO SHALL exponerse en el cuerpo
- **AND** NO SHALL emitirse sesión
