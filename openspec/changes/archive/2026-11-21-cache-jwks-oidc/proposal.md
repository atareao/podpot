# Proposal

## Why

En modo `oidc`, `verificar_id_token` (`backend/src/auth/oidc.rs:377`) llama de forma incondicional a `fetch_jwks(&discovery.jwks_uri)` en **cada callback** del flujo de autorización: una petición HTTP al proveedor por cada inicio de sesión, aunque la clave del `kid` no haya cambiado y el conjunto se acabe de descargar. El descubrimiento del proveedor ya se cachea (`discovery_cached`, `oidc.rs:128`, sobre `AuthState.discovery: Arc<RwLock<Option<Discovery>>>`); el JWKS es el único documento del flujo que se sigue pidiendo siempre. Es el follow-up registrado en `HANDOFF.md` («OIDC: caché de JWKS (se pide en cada callback)»).

## What Changes

- Se añade una **caché del JWKS** en `AuthState`, análoga a la del descubrimiento: `jwks: Arc<RwLock<Option<…>>>` sobre el `JwkSet` de `jsonwebtoken`.
- Se introduce `jwks_cached(auth: &AuthState)` en `backend/src/auth/oidc.rs`, con el mismo patrón que `discovery_cached`: lee la caché; si está vacía o caducada, pide el `jwks_uri` y la reescribe.
- La caché SHALL tener **frescura acotada** (TTL): transcurrido el TTL se vuelve a pedir el `jwks_uri`, de modo que no se sirva indefinidamente una clave retirada.
- Si el ID token trae un `kid` que el conjunto cacheado no publica, se **refresca** el `jwks_uri` una sola vez antes de rechazar (protección ante rotación de claves dentro del TTL).
- `verificar_id_token` pasa a resolver el JWKS por la caché (se ajusta su firma/llamador en `backend/src/api/auth.rs:168`).
- El contrato HTTP NO cambia: el rechazo de un ID token sigue siendo `401` genérico y un fallo del proveedor al refrescar el JWKS sigue siendo `BadGateway` (`502`) con la envoltura `{error:{code,message}}`.

## Capabilities

### Modified Capabilities

- `autenticacion`: se documenta que el JWKS del proveedor se cachea con frescura acotada y se reutiliza entre callbacks, refrescándose ante un `kid` desconocido, sin alterar el contrato de rechazo ni de error del flujo OIDC.

## Impact

- Backend (`backend/src/`): `auth/state.rs` (`AuthState`), `auth/oidc.rs` (`fetch_jwks`, `verificar_id_token`, nuevo `jwks_cached`), `api/auth.rs` (llamador) y `test_support.rs` (doble del proveedor con contador de peticiones al `/jwks`).
- Sin cambios en el frontend, en el contrato de la API, en el despliegue ni en la configuración.
