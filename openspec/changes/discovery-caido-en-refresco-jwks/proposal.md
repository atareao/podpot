# Proposal — discovery-caido-en-refresco-jwks

## Why

Cuando el refresco del JWKS necesita el `jwks_uri`, primero resuelve el descubrimiento con `discovery_cached`. Si el descubrimiento falla en ese instante, `refrescar_bajo_candado` propaga el error y no escribe la caché. El camino está implementado y correcto, pero no tiene test propio: es el follow-up registrado en `docs/cache-jwks-oidc.md`.

## What Changes

- El doble de pruebas (`ProveedorFalso`) gana la posibilidad de tumbar el descubrimiento, análoga a `tumbar_el_jwks`.
- Test que cubre: descubrimiento caído durante el refresco del JWKS → `502` con la envoltura existente, sin sesión y sin envenenar la caché anterior.
- Sin cambios de contrato HTTP ni de frontend.

## Impact

- `backend/src/test_support.rs`: `tumbar_el_descubrimiento`.
- `backend/src/auth/oidc.rs`: test del escenario.
- `docs/cache-jwks-oidc.md`: se actualiza el follow-up a comportamiento cubierto.
