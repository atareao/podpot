# Spec Delta — api-shell

## Purpose

El servidor HTTP de podpot. Este delta añade la ruta de stream de progreso de jobs, generaliza la excepción a la envoltura JSON única para cubrir los flujos SSE y fija su cierre en el apagado ordenado.

## MODIFIED Requirements

### Requirement: Envoltura JSON única

Todas las rutas de negocio bajo `/api/v1/` SHALL responder con una envoltura JSON uniforme (un campo con los datos y, cuando proceda, un campo de error), también en los errores. Las **únicas excepciones** SHALL ser los flujos SSE (`GET /api/v1/units/stream` y `GET /api/v1/jobs/stream`), que son respuestas continuas y no documentos únicos; aun así, el **contenido** de cada evento SHALL reutilizar la envoltura `{data}`.

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

#### Scenario: El stream de trabajos no usa la envoltura de documento único

- **WHEN** se pide `GET /api/v1/jobs/stream`
- **THEN** la respuesta SHALL ser un flujo `text/event-stream` (no un documento JSON único), y el contenido de cada evento SHALL usar la envoltura `{data}`

### Requirement: Apagado ordenado

El servidor SHALL dejar de aceptar conexiones nuevas y cerrar los recursos ante una señal de terminación, incluidos los flujos SSE abiertos.

#### Scenario: Terminación limpia

- **WHEN** el proceso recibe `SIGTERM`
- **THEN** SHALL terminar el servidor y liberar el pool y el cliente D-Bus antes de salir

#### Scenario: Terminación con un stream abierto

- **WHEN** el proceso recibe `SIGTERM` con un cliente suscrito a un flujo SSE (`GET /api/v1/units/stream` o `GET /api/v1/jobs/stream`)
- **THEN** SHALL cerrar el flujo SSE y completar el apagado sin bloquearse esperando a esa conexión

## ADDED Requirements

### Requirement: Ruta de stream de trabajos

El servidor SHALL exponer `GET /api/v1/jobs/stream` como ruta de negocio bajo `/api/v1/`, devolviendo un flujo `text/event-stream` con el progreso de los jobs de systemd (ver la capability `progreso-de-trabajos`). SHALL admitirse únicamente `GET`, y la ruta SHALL quedar bajo la protección del gate como el resto de la superficie de negocio.

#### Scenario: Stream de trabajos disponible

- **WHEN** se pide `GET /api/v1/jobs/stream`
- **THEN** SHALL responder `200` con `Content-Type: text/event-stream`

#### Scenario: Método no permitido en el stream de trabajos

- **WHEN** se usa un método distinto de `GET` sobre `/api/v1/jobs/stream`
- **THEN** la respuesta SHALL ser `405` con la envoltura `{error}` habitual

#### Scenario: Sin sesión en modo OIDC

- **WHEN** en modo `oidc` se pide la ruta de trabajos sin sesión válida
- **THEN** SHALL responder `401` con la envoltura `{error}` y NO SHALL abrir el flujo
