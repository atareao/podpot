# Spec Delta — autenticacion

## Purpose

El modo de autenticación de podpot: por defecto deshabilitado (dev en loopback), o mediante un proveedor OIDC con una sesión JWT en cookie y un gate que protege la superficie de negocio. Este delta añade la caché del JWKS del proveedor.

## ADDED Requirements

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
