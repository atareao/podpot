# Spec Delta — api-shell

## Purpose

El servidor HTTP de podpot. Este delta añade la ruta de stream de logs de una quadlet, bajo la mitad protegida.

## ADDED Requirements

### Requirement: Ruta de stream de logs

El servidor SHALL exponer `GET /api/v1/units/{name}/logs/stream` como ruta de negocio bajo `/api/v1/`, devolviendo un flujo `text/event-stream` con los logs del contenedor de la unidad (ver la capability `registros`). SHALL admitirse únicamente `GET`, y la ruta SHALL quedar bajo la protección del gate como el resto de la superficie de negocio. Si la unidad no tiene contenedor, SHALL responder un error explícito con la envoltura `{error}` en lugar de abrir el flujo.

#### Scenario: Stream de logs disponible

- **WHEN** se pide `GET /api/v1/units/{name}/logs/stream` con una unidad que tiene contenedor
- **THEN** SHALL responder `200` con `Content-Type: text/event-stream`

#### Scenario: Método no permitido

- **WHEN** se usa un método distinto de `GET` sobre la ruta de logs
- **THEN** la respuesta SHALL ser `405` con la envoltura `{error}` habitual

#### Scenario: Sin sesión en modo OIDC

- **WHEN** en modo `oidc` se pide la ruta de logs sin sesión válida
- **THEN** SHALL responder `401` con la envoltura `{error}` y NO SHALL abrir el flujo

#### Scenario: Unidad sin contenedor

- **WHEN** se piden los logs de una unidad sin contenedor
- **THEN** SHALL responder un error explícito con la envoltura `{error}` (no un flujo vacío)
