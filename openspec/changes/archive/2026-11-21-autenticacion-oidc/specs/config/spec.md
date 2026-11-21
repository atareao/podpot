# Spec Delta — config

## Purpose

La configuración de podpot leída del entorno. Este delta añade las variables del proveedor OIDC y el secreto de firma de la sesión, con la regla «las tres OIDC juntas o ninguna».

## ADDED Requirements

### Requirement: Variables de autenticación

La configuración SHALL reconocer las variables del proveedor OIDC `PODPOT_OIDC_ISSUER_URL`, `PODPOT_OIDC_CLIENT_ID`, `PODPOT_OIDC_CLIENT_SECRET` y `PODPOT_OIDC_REDIRECT_URL` (esta última con valor por defecto), y la variable del secreto de firma de sesión `PODPOT_SECRET`. La presencia de las tres primeras SHALL determinar el modo `oidc`; su ausencia total, el modo `disabled`. Una definición parcial SHALL ser un error de arranque.

#### Scenario: Las tres definidas activan OIDC

- **WHEN** `PODPOT_OIDC_ISSUER_URL`, `PODPOT_OIDC_CLIENT_ID` y `PODPOT_OIDC_CLIENT_SECRET` están definidas
- **THEN** el modo de autenticación SHALL ser `oidc` y SHALL exigirse `PODPOT_SECRET` para firmar la sesión

#### Scenario: Ninguna definida

- **WHEN** ninguna de las tres variables OIDC está definida
- **THEN** el modo de autenticación SHALL ser `disabled` y la aplicación SHALL arrancar sin exigir `PODPOT_SECRET`

#### Scenario: Definición parcial

- **WHEN** se define alguna de las tres variables OIDC pero no todas
- **THEN** el arranque SHALL fallar nombrando la variable que falta

#### Scenario: Redirect por defecto

- **WHEN** `PODPOT_OIDC_REDIRECT_URL` no está definida y el modo es `oidc`
- **THEN** SHALL usarse un valor por defecto coherente con el bind local

#### Scenario: Falta el secreto en modo OIDC

- **WHEN** el modo es `oidc` y `PODPOT_SECRET` no está definida
- **THEN** el arranque SHALL fallar nombrando `PODPOT_SECRET`
