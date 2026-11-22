# Spec Delta — api-shell

## REMOVED Requirements

### Requirement: Rutas de autenticación

**Reason**: El logout deja de ser una ruta protegida; se reemplaza por una requirement que refleja el nuevo contrato.

**Migration**: Usar la nueva requirement «Rutas de autenticación con logout público».

## ADDED Requirements

### Requirement: Rutas de autenticación con logout público

El servidor SHALL exponer `GET /api/v1/auth/login`, `GET /api/v1/auth/callback` y `POST /api/v1/auth/logout` como rutas públicas, y `GET /api/v1/auth/session` como ruta protegida (ver la capability `autenticacion`).

#### Scenario: Login y callback públicos

- **WHEN** en modo `oidc` se piden `GET /api/v1/auth/login` o `GET /api/v1/auth/callback`
- **THEN** SHALL atenderse sin exigir sesión

#### Scenario: Sesión protegida

- **WHEN** en modo `oidc` se pide `GET /api/v1/auth/session` sin sesión válida
- **THEN** SHALL responder `401` con la envoltura `{error}`

#### Scenario: Logout público

- **WHEN** en modo `oidc` se pide `POST /api/v1/auth/logout` sin sesión válida
- **THEN** SHALL responder con éxito y SHALL emitir la cookie de borrado

## MODIFIED Requirements

### Requirement: Partición público/negocio

En modo `oidc`, el servidor SHALL servir la superficie de negocio tras un gate que exige sesión, y SHALL mantener públicas únicamente la salud y las rutas de login, callback y logout. Un método no permitido sobre una ruta de negocio SHALL seguir respondiendo `405` con la envoltura, con independencia de la sesión.

#### Scenario: Superficie de negocio protegida

- **WHEN** en modo `oidc` se pide cualquier ruta de negocio sin sesión válida
- **THEN** SHALL responder `401` con la envoltura `{error}`

#### Scenario: Método no permitido conserva 405

- **WHEN** se usa un método no permitido sobre una ruta de negocio existente
- **THEN** SHALL responder `405` con la envoltura, preservando el fallback existente
