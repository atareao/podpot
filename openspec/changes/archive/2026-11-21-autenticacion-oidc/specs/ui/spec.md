# Spec Delta — ui

## Purpose

El panel web (React + Ant Design). Este delta añade el inicio de sesión: portada pública con botón de login, guarda de ruta que exige sesión cuando el modo es `oidc`, y reacción global a la pérdida de sesión.

## ADDED Requirements

### Requirement: Inicio de sesión en el panel

El panel SHALL consultar el modo de autenticación anunciado por la salud. Cuando el modo sea `oidc`, SHALL exigir sesión para acceder a la vista de unidades: sin sesión, SHALL mostrar una portada pública con un botón que lleve al inicio de sesión del servidor. Cuando el modo sea `disabled`, SHALL permitir el acceso sin sesión (dev). Al perder la sesión (respuesta `401`), el panel SHALL volver a la portada pública.

#### Scenario: Modo deshabilitado no exige sesión

- **WHEN** la salud anuncia modo `disabled`
- **THEN** el panel SHALL mostrar la vista de unidades sin exigir sesión

#### Scenario: Modo OIDC sin sesión

- **WHEN** la salud anuncia modo `oidc` y no hay sesión
- **THEN** el panel SHALL mostrar la portada pública con el botón de inicio de sesión, no la vista de unidades

#### Scenario: Botón de inicio de sesión

- **WHEN** el usuario pulsa el botón de inicio de sesión
- **THEN** SHALL navegar al inicio de sesión del servidor

#### Scenario: Sesión perdida durante el uso

- **WHEN** una petición del panel recibe `401`
- **THEN** el panel SHALL considerarse sin sesión y volver a la portada pública
