# PLAN-006 — autenticación OIDC opcional (patrón kesto) para exponer el panel

- **Issue:** #12 — feat: autenticación OIDC opcional (patrón kesto) para exponer el panel
- **Change OpenSpec:** `openspec/changes/autenticacion-oidc`
- **Capability nueva:** `autenticacion` (deltas en `config`, `api-shell`, `ui`)
- **Estado:** en curso

## Objetivo

Permitir exponer podpot fuera de loopback autenticando a los usuarios, reutilizando el patrón OIDC de kesto (flujo a mano + sesión JWT en cookie), de forma **opcional**: por defecto el panel sigue abierto como hoy.

## Arquitectura (dónde toca)

- `backend/src/config.rs`: `AuthMode` (`Disabled`|`Oidc`), `OidcSettings` (issuer, client_id, client_secret, redirect_url) y `secret` (firma de sesión). Vars `PODPOT_OIDC_*` y `PODPOT_SECRET`; regla «las tres OIDC juntas o ninguna».
- `backend/src/auth/`: `oidc.rs` (discovery, authorization URL con `state`/`nonce`, canje, verificación de ID token), `session.rs` (JWT HS256 en cookie `HttpOnly`/`SameSite=Strict`/`Secure` si https), `middleware.rs` (gate que responde 401 envuelto).
- `backend/src/api/auth.rs`: `GET /auth/login` (302), `GET /auth/callback` (302 + `Set-Cookie`), `POST /auth/logout`, `GET /auth/session`.
- `backend/src/api/mod.rs`: partición público/negocio; `route_layer(from_fn_with_state)` tras `method_not_allowed_fallback`.
- `backend/src/api/health.rs`: publica `auth_mode`.
- `backend/src/state.rs`: `AppState.auth: Option<AuthState>`.
- `backend/Cargo.toml`: `jsonwebtoken`, `reqwest` (rustls-tls,json), `axum-extra` (cookie), `rand`, `urlencoding`, `zeroize`. **Sin** `openidconnect`/`tower-sessions`.
- `frontend/src/`: helper de sesión + `esErrorDeSesion`, portada/login, guarda de ruta según `auth_mode`, reacción global al 401 en `clienteDeConsultas`.

## Decisiones clave

- `Disabled` por defecto → retrocompatible (dev en loopback sin gate).
- Flujo OIDC escrito a mano (patrón kesto), sin crates ausentes de la caché offline.
- Sesión = JWT en cookie, sin estado en servidor.
- El gate devuelve **401 envuelto** (nunca 302/HTML); el SSE queda protegido por estar en la mitad protegida (no se toca el handler).
- `/health` publica `auth_mode` para que el panel decida si exigir sesión.

## Tareas (TDD por bloques)

- [ ] 1. Configuración (`AuthMode`/`OidcSettings`/`secret`, vars, dependencias).
- [ ] 2. Sesión (JWT + cookie + `AuthState` con `state`/`nonce`).
- [ ] 3. Flujo OIDC (discovery, authorization URL, canje, verificación de ID token).
- [ ] 4. Rutas y gate (login/callback/logout/session; partición público/negocio; `disabled` sin gate).
- [ ] 5. Salud (`auth_mode`) y SSE (401 sin sesión, 200 con sesión).
- [ ] 6. Frontend (helper de sesión, portada/login, guarda, 401 global).
- [ ] 7. Cierre (`just check-all`, reviewers, `openspec validate`, `docs/autenticacion-oidc.md`).

## Criterios de aceptación

- En modo `oidc`, sin sesión la superficie de negocio responde 401 envuelto (incluido el stream); con sesión, funciona.
- El flujo login→callback→sesión funciona contra un proveedor real (o simulado en tests).
- En modo `disabled` (por defecto), nada cambia respecto a hoy.
- `just check-all` EXIT 0; `openspec validate --all` en verde; reviewers sin bloqueantes.

## Fuera de alcance

- Autorización por rol/permiso; múltiples proveedores; refresh de tokens contra el proveedor.
