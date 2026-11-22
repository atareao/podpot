# Proposal — dedupe-jwks-rotacion

## Why

El refresco por caducidad del JWKS ya se deduplica bajo el candado de escritura. El refresco **forzado** por rotación de claves no: N callbacks concurrentes con un `kid` que la caché no publica pueden encadenar N peticiones al `jwks_uri`, serializadas bajo el candado pero todas reales. Es el follow-up pendiente de `docs/cache-jwks-oidc.md`.

## What Changes

- El refresco forzado comprobará, ya bajo el candado, si otro callback ha refrescado el conjunto desde que este leyó la caché. Si es así, reutiliza ese conjunto sin volver a pedir el `jwks_uri`.
- Se conserva el resto del comportamiento: un `kid` que sigue sin aparecer tras el refresco se rechaza; un fallo del proveedor se propaga como `502` sin envenenar la caché; el refresco por caducidad sigue deduplicado.
- Sin cambios de contrato HTTP ni de frontend.

## Impact

- `backend/src/auth/oidc.rs`: la caché expone la marca de lectura y el refresco forzado la usa para deduplicar.
- Tests de concurrencia en el mismo módulo.
- `docs/cache-jwks-oidc.md`: se actualiza el follow-up a comportamiento implementado.
