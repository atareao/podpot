# Proposal

## Why

podpot se ata a **loopback** y **no lleva autenticación** (decisión documentada en `config.rs`: «no lleva autenticación en esta entrega»). Para exponer el panel fuera del host hace falta autenticar. El entorno ya tiene un patrón probado y en producción: **kesto** (`/data/rust/kesto`), que escribe el flujo OIDC a mano y usa una sesión JWT en cookie.

## What Changes

- **Modo de autenticación configurable**: `Disabled` (por defecto; dev en loopback) | `Oidc`. Se activa con variables de entorno; sin ellas el panel sigue abierto **exactamente como hoy** (retrocompatible).
- **Flujo OIDC a mano** (como kesto; **sin** `openidconnect` ni `tower-sessions`, que no están en la caché offline): discovery (`/.well-known/openid-configuration`), `authorization_endpoint` con `state`/`nonce`, canje del código en el `token_endpoint`, y verificación del ID token contra el JWKS. Crates nuevos: `jsonwebtoken` (rust_crypto, use_pem), `reqwest` (rustls-tls, json), `axum-extra` (cookie), `rand`, `urlencoding`, `zeroize`.
- **Sesión = JWT en cookie** (`HttpOnly`, `SameSite=Strict`, `Path=/`, `Secure` solo si el redirect es https), firmado con un secreto de la config. **Sin estado en servidor** (no hay store de sesiones).
- **Rutas de autenticación**: `GET /api/v1/auth/login` (302 al proveedor), `GET /api/v1/auth/callback` (302 al panel con `Set-Cookie`), `POST /api/v1/auth/logout` (borra la cookie), `GET /api/v1/auth/session` (identidad actual).
- **Gate** (middleware `from_fn_with_state`) que protege toda la superficie de negocio **salvo** `GET /health`, `GET /auth/login` y `GET /auth/callback`. Sin credencial → **401 con la envoltura** `{error}` (nunca 302 ni HTML), para que el frontend lo distinga.
- **El SSE queda protegido por construcción**: cuelga de la mitad protegida, así que el 401 lo pone el gate **antes** de abrir el flujo; el handler no cambia.
- **`GET /health` publica `auth_mode`** (`oidc`|`disabled`) para que el panel sepa si debe exigir sesión.
- **Frontend**: pantalla de login (botón → `/api/v1/auth/login`), guarda de ruta que exige sesión cuando el modo es `oidc`, y reacción global al 401 (invalidar la sesión y volver a la portada).

## Capabilities

### New Capabilities

- `autenticacion`: el contrato del modo de auth, el flujo OIDC (login/callback/logout/session), la sesión en cookie, y el gate que protege la superficie de negocio.

### Modified Capabilities

- `config`: nuevas variables de entorno OIDC y secreto de sesión; validación «las tres OIDC juntas o ninguna».
- `api-shell`: la ruta de salud pasa a publicar `auth_mode`; se añaden las rutas de auth; se formaliza que la superficie de negocio se sirve tras el gate (401 envuelto).
- `ui`: pantalla de login, guarda de sesión y reacción al 401.

## Impact

- `backend/Cargo.toml`: 6 dependencias nuevas (todas presentes en kesto/caché).
- `backend/src/`: nuevo módulo `auth/` (`oidc.rs`, `session.rs`, `middleware.rs`) y `api/auth.rs`; `config.rs` (AuthMode/OidcSettings/secret); `state.rs` (auth opcional); `api/mod.rs` (partición público/negocio + gate); `api/health.rs` (`auth_mode`).
- Frontend (`frontend/src/`): `services`/`api` (helper de sesión, `esErrorDeSesion`), `App.tsx` (guarda + ruta de login), `clienteDeConsultas.ts` (401 global), `pages/` (portada/login).
- Contrato de API: **aditivo**; con `Disabled` (por defecto) nada cambia.
- Despliegue: sin cambios por defecto; para exponer, definir `PODPOT_OIDC_*` + `PODPOT_SECRET` y bind no-loopback.

## Fuera de alcance

- Autorización por rol/permiso (solo se verifica identidad autenticada).
- Múltiples proveedores o multi-tenant.
- Renovación de tokens / refresh contra el proveedor (la sesión local caduca y se reloguea).
