# Proposal — refresh-tokens-proveedor

## Why

Follow-up OIDC pendiente. Hoy el canje del código guarda solo el `id_token` y descarta `access_token` y `refresh_token`; la sesión local se renueva con su propio JWT. Así, podpot no puede saber si el proveedor sigue considerando válida la sesión: un usuario revocado en el IdP conserva su sesión local hasta que caduque.

## What Changes

- El canje captura el `refresh_token` cuando el proveedor lo emite y lo asocia a la sesión mediante un `jti`.
- El `refresh_token` se guarda **solo en memoria**, nunca en disco ni en la cookie.
- La renovación deslizante, si hay `refresh_token`, pide un `id_token` nuevo al proveedor con `grant_type=refresh_token` antes de emitir la cookie.
- El ID token nuevo se verifica con la misma exigencia que el del callback, salvo el `nonce` (puede no venir en un refresh).
- Si el proveedor devuelve un `refresh_token` nuevo, se sustituye el anterior.
- Si el proveedor responde `invalid_grant`, se descarta el token y se fuerza el cierre de sesión (`401` con la envoltura y cookie borrada).
- Si el refresh falla por red o por un error del proveedor distinto de `invalid_grant`, se mantiene la sesión local y se renueva como hasta ahora.
- Sin `refresh_token`, el comportamiento es el actual.

## Impact

- `backend/src/auth/oidc.rs`: `TokenResponse` captura `refresh_token` y `Canje` lo devuelve; verificación del ID token de refresh sin `nonce`. Aquí viven los tests del flujo (captura, renovación, rotación, revocación, concurrencia).
- `backend/src/auth/session.rs`: `jti` en los claims; emisión con `jti`.
- `backend/src/auth/state.rs`: mapa en memoria `jti → (refresh_token, último uso)` acotado por TTL, *tombstone* de `jti` revocados y candado por `jti` para serializar el refresh.
- `backend/src/auth/middleware.rs`: refresh del proveedor en la renovación, serializado por `jti`; `401` y borrado ante `invalid_grant`, con la revocación pegajosa.
- `docs/refresh-tokens-proveedor.md` y `docs/autenticacion-oidc.md`.

## Riesgos aceptados

- El `refresh_token` se pierde al reiniciar el proceso: la sesión local sigue válida, pero sin refresh del proveedor hasta el siguiente login.
- El refresh añade una petición al proveedor en cada renovación (cada ~6 h); si el proveedor está lento, la renovación se retrasa.
- Un `invalid_grant` cierra la sesión aunque el usuario pudiera seguir trabajando: es el comportamiento deseado ante revocación.
