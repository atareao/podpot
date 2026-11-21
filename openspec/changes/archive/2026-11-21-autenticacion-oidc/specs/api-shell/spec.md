# Spec Delta — api-shell

## Purpose

El servidor HTTP de podpot: arranque, envoltura JSON única de las respuestas, ruta de salud, manejo de errores y apagado ordenado. Este delta añade las rutas de autenticación, publica el modo de auth en la salud y formaliza la partición público/negocio tras el gate.

## MODIFIED Requirements

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

## ADDED Requirements

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
