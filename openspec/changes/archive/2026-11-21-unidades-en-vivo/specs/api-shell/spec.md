# Spec Delta — api-shell

## Purpose

El servidor HTTP de podpot: arranque, envoltura JSON única de las respuestas, ruta de salud, manejo de errores y apagado ordenado. Este delta añade la ruta de stream (SSE) y exceptúa el stream de la envoltura JSON única, además de fijar el cierre del stream en el apagado ordenado.

## MODIFIED Requirements

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

### Requirement: Apagado ordenado

El servidor SHALL dejar de aceptar conexiones nuevas y cerrar los recursos ante una señal de terminación, incluidos los flujos SSE abiertos.

#### Scenario: Terminación limpia

- **WHEN** el proceso recibe `SIGTERM`
- **THEN** SHALL terminar el servidor y liberar el pool y el cliente D-Bus antes de salir

#### Scenario: Terminación con un stream abierto

- **WHEN** el proceso recibe `SIGTERM` con un cliente suscrito a `GET /api/v1/units/stream`
- **THEN** SHALL cerrar el flujo SSE y completar el apagado sin bloquearse esperando a esa conexión

## ADDED Requirements

### Requirement: Ruta de stream de unidades

El servidor SHALL exponer `GET /api/v1/units/stream` como ruta de negocio bajo `/api/v1/`, devolviendo un flujo `text/event-stream` con la instantánea de las quadlets (ver la capability `unidades-en-vivo`). El método `GET` SHALL ser el único admitido.

#### Scenario: Stream disponible

- **WHEN** se pide `GET /api/v1/units/stream`
- **THEN** SHALL responder `200` con `Content-Type: text/event-stream`

#### Scenario: Método no permitido en el stream

- **WHEN** se usa un método distinto de `GET` sobre `/api/v1/units/stream`
- **THEN** la respuesta SHALL ser `405` con la envoltura `{error}` habitual
