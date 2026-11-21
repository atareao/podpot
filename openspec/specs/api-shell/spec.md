# api-shell Specification

## Purpose
El servidor HTTP de podpot: arranque, envoltura JSON única de las respuestas, ruta de salud, manejo de errores y apagado ordenado.

## Requirements

### Requirement: Envoltura JSON única

Todas las rutas de negocio bajo `/api/v1/` SHALL responder con una envoltura JSON uniforme (un campo con los datos y, cuando proceda, un campo de error), también en los errores. La **única excepción** SHALL ser la ruta de stream (`GET /api/v1/units/stream`), que es una respuesta continua y no un documento único; aun así, el **contenido** de cada evento del stream SHALL reutilizar la envoltura `{data}`.

#### Scenario: Respuesta de negocio correcta

- **WHEN** una ruta de negocio devuelve datos
- **THEN** la respuesta SHALL ser `200` con la envoltura JSON y los datos dentro

#### Scenario: Recurso desconocido

- **WHEN** se pide una ruta inexistente
- **THEN** la respuesta SHALL ser `404` con la misma envoltura y un error descriptivo

#### Scenario: Método no permitido

- **WHEN** se usa un método no permitido sobre una ruta existente
- **THEN** la respuesta SHALL ser `405` con la misma envoltura

#### Scenario: Fallbacks preservados en la ruta de creación

- **WHEN** se usa un método distinto de `POST` sobre `/api/v1/quadlets`
- **THEN** la respuesta SHALL ser `405` con la misma envoltura, preservando el fallback existente

#### Scenario: Fallbacks preservados en la ruta de una quadlet

- **WHEN** se usa un método distinto de `GET`/`PATCH`/`DELETE` sobre `/api/v1/quadlets/{name}`
- **THEN** la respuesta SHALL ser `405` con la misma envoltura

#### Scenario: El stream no usa la envoltura de documento único

- **WHEN** se pide `GET /api/v1/units/stream`
- **THEN** la respuesta SHALL ser un flujo `text/event-stream` (no un documento JSON único), y el contenido de cada evento SHALL usar la envoltura `{data}`

### Requirement: Ruta de salud

El servidor SHALL exponer `GET /api/v1/health` sin autenticación, con el estado del proceso y el modo de autenticación activo (`oidc` o `disabled`).

#### Scenario: Salud disponible

- **WHEN** el proceso está vivo y se llama a `GET /api/v1/health`
- **THEN** SHALL responder `200` con un estado que incluya, al menos, el modo de escucha

#### Scenario: Salud anuncia el modo de auth

- **WHEN** se pide `GET /api/v1/health`
- **THEN** SHALL incluir en la respuesta el modo de autenticación activo (`oidc` o `disabled`)

#### Scenario: Salud accesible sin sesión

- **WHEN** en modo `oidc` se pide `GET /api/v1/health` sin sesión
- **THEN** SHALL responder con normalidad, sin exigir sesión

### Requirement: Apagado ordenado

El servidor SHALL dejar de aceptar conexiones nuevas y cerrar los recursos ante una señal de terminación, incluidos los flujos SSE abiertos.

#### Scenario: Terminación limpia

- **WHEN** el proceso recibe `SIGTERM`
- **THEN** SHALL terminar el servidor y liberar el pool y el cliente D-Bus antes de salir

#### Scenario: Terminación con un stream abierto

- **WHEN** el proceso recibe `SIGTERM` con un cliente suscrito a `GET /api/v1/units/stream`
- **THEN** SHALL cerrar el flujo SSE y completar el apagado sin bloquearse esperando a esa conexión

### Requirement: Ruta de creación de quadlets

El servidor SHALL exponer `POST /api/v1/quadlets` para crear una quadlet de un tipo soportado. La respuesta SHALL usar la envoltura existente `{data}`/`{error}`.

#### Scenario: Creación correcta

- **WHEN** se envía una petición válida de creación de quadlet
- **THEN** SHALL responder `201` (o `200`) con `{data}` conteniendo, al menos, el nombre de unidad generado

#### Scenario: Error de creación envuelto

- **WHEN** la creación falla con un error tipado
- **THEN** la respuesta SHALL usar `{error:{code,message}}` con el estado HTTP correspondiente al tipo de error

### Requirement: Ruta de consulta de quadlets

El servidor SHALL exponer `GET /api/v1/quadlets/{name}` para leer la definición de una quadlet existente, devolviendo su tipo y sus campos. La respuesta SHALL usar la envoltura existente `{data}`/`{error}`. Si el fichero no existe, SHALL producirse `NotFound` (`not_found`, `404`).

#### Scenario: Consulta correcta

- **WHEN** se pide una quadlet existente por su nombre de fichero
- **THEN** SHALL responder `200` con `{data}` conteniendo, al menos, el tipo (`kind`) y los campos de la quadlet

#### Scenario: Consulta de inexistente

- **WHEN** se pide un nombre de fichero que no existe en el directorio configurado
- **THEN** la respuesta SHALL ser `NotFound` (`not_found`, `404`) con la misma envoltura

### Requirement: Ruta de edición de quadlets

El servidor SHALL exponer `PATCH /api/v1/quadlets/{name}` para actualizar una quadlet existente. La respuesta SHALL usar la envoltura existente `{data}`/`{error}`.

#### Scenario: Edición correcta

- **WHEN** se envía una petición válida de edición de una quadlet existente
- **THEN** SHALL responder `200` con `{data}` conteniendo, al menos, el nombre de unidad afectado

#### Scenario: Error de edición envuelto

- **WHEN** la edición falla con un error tipado
- **THEN** la respuesta SHALL usar `{error:{code,message}}` con el estado HTTP correspondiente al tipo de error

### Requirement: Ruta de borrado de quadlets

El servidor SHALL exponer `DELETE /api/v1/quadlets/{name}` para borrar una quadlet existente. La respuesta SHALL usar la envoltura existente `{data}`/`{error}`.

#### Scenario: Borrado correcto

- **WHEN** se envía una petición válida de borrado de una quadlet existente
- **THEN** SHALL responder `200` con `{data}` confirmando el nombre de fichero borrado

#### Scenario: Error de borrado envuelto

- **WHEN** el borrado falla con un error tipado
- **THEN** la respuesta SHALL usar `{error:{code,message}}` con el estado HTTP correspondiente al tipo de error

### Requirement: Ruta de listado de unidades

El servidor SHALL exponer `GET /api/v1/units` devolviendo las quadlets del usuario con su estado. La respuesta SHALL usar la envoltura existente `{data}`/`{error}`. Por cada quadlet SHALL publicar, además de su nombre de unidad y sus estados, su **nombre de fichero** (`file_name`), que es la identidad con la que operan `PATCH`/`DELETE /api/v1/quadlets/{name}`. NUNCA SHALL publicar la ruta del sistema de ficheros de la que procede la unidad.

#### Scenario: Listado con la identidad real de cada quadlet

- **WHEN** se llama a `GET /api/v1/units` y hay quadlets instaladas
- **THEN** SHALL responder `200` con `{data}` conteniendo, por cada quadlet, al menos su `name`, `active_state`, `sub_state`, `unit_file_state` y su `file_name` (el basename del fichero de la quadlet, p. ej. `web.container`)

#### Scenario: La ruta no se publica

- **WHEN** se inspecciona cualquier unidad del listado
- **THEN** SHALL contener el nombre de fichero pero NO la ruta del sistema de ficheros de la que systemd la generó

#### Scenario: Sin quadlets

- **WHEN** el usuario no tiene quadlets instaladas
- **THEN** SHALL responder `200` con `{data}` como array vacío

### Requirement: Ruta de stream de unidades

El servidor SHALL exponer `GET /api/v1/units/stream` como ruta de negocio bajo `/api/v1/`, devolviendo un flujo `text/event-stream` con la instantánea de las quadlets (ver la capability `unidades-en-vivo`). El método `GET` SHALL ser el único admitido.

#### Scenario: Stream disponible

- **WHEN** se pide `GET /api/v1/units/stream`
- **THEN** SHALL responder `200` con `Content-Type: text/event-stream`

#### Scenario: Método no permitido en el stream

- **WHEN** se usa un método distinto de `GET` sobre `/api/v1/units/stream`
- **THEN** la respuesta SHALL ser `405` con la envoltura `{error}` habitual

### Requirement: Rutas de autenticación

El servidor SHALL exponer `GET /api/v1/auth/login` y `GET /api/v1/auth/callback` como rutas públicas, y `POST /api/v1/auth/logout` y `GET /api/v1/auth/session` como rutas protegidas (ver la capability `autenticacion`).

#### Scenario: Login y callback públicos

- **WHEN** en modo `oidc` se piden `GET /api/v1/auth/login` o `GET /api/v1/auth/callback`
- **THEN** SHALL atenderse sin exigir sesión

#### Scenario: Sesión y logout protegidos

- **WHEN** en modo `oidc` se piden `POST /api/v1/auth/logout` o `GET /api/v1/auth/session` sin sesión válida
- **THEN** SHALL responder `401` con la envoltura `{error}`

### Requirement: Partición público/negocio

En modo `oidc`, el servidor SHALL servir la superficie de negocio tras un gate que exige sesión, y SHALL mantener públicas únicamente la salud y las rutas de login/callback. Un método no permitido sobre una ruta de negocio SHALL seguir respondiendo `405` con la envoltura, con independencia de la sesión.

#### Scenario: Superficie de negocio protegida

- **WHEN** en modo `oidc` se pide cualquier ruta de negocio sin sesión válida
- **THEN** SHALL responder `401` con la envoltura `{error}`

#### Scenario: Método no permitido conserva 405

- **WHEN** se usa un método no permitido sobre una ruta de negocio existente
- **THEN** SHALL responder `405` con la envoltura, preservando el fallback existente
