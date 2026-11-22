# Proposal — logout-cookie-invalida

## Why

`POST /api/v1/auth/logout` está detrás del *gate* de sesión. Si la cookie existe pero no verifica —caducada, firma inválida o secreto rotado—, el *gate* responde `401` antes de llegar al manejador y la cookie no se borra. El usuario queda atrapado: no puede cerrar sesión y el navegador sigue enviando una cookie inservible.

## What Changes

- `POST /api/v1/auth/logout` pasa a ser ruta pública en modo `oidc`, junto a salud, login y callback.
- El manejador borra siempre la cookie, con el mismo `Path` y `Max-Age=0`, aunque no haya sesión válida.
- Se mantiene `POST` (nunca `GET`) y el resto de rutas de negocio siguen protegidas.
- Se actualizan las specs `autenticacion` y `api-shell` y los tests de la partición.

## Impact

- Affected specs: `autenticacion`, `api-shell`
- Affected code: `backend/src/api/mod.rs`, `backend/src/api/auth.rs`
- Riesgo: bajo; el logout es idempotente y no destructivo. El único efecto nuevo es que un cliente sin sesión válida puede borrar su propia cookie.
