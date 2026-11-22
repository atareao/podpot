# PLAN-022 — Refrescar el ID token del proveedor

## Issue

#45 — Refrescar el ID token del proveedor con refresh_token.

## Objetivo

Que la sesión local se apoye en el proveedor cuando este emite `refresh_token`: al renovar, se pide un ID token nuevo y se verifica; si el proveedor revoca, se cierra la sesión.

## Contexto

- `backend/src/auth/oidc.rs`: `TokenResponse` solo guarda `id_token`; `access_token` y `refresh_token` se descartan.
- `backend/src/auth/session.rs`: claims `sub`, `email`, `iat`, `exp`; sin `jti`.
- `backend/src/auth/state.rs`: `AuthState` con cachés de discovery/JWKS y mapas de logins.
- `backend/src/auth/middleware.rs`: el *gate* renueva con `session::renew`; un fallo solo se registra.

## Diseño

- `TokenResponse` añade `refresh_token: Option<String>`.
- Claims añaden `jti: Option<String>`; las sesiones nuevas lo llevan, las antiguas siguen valiendo sin él.
- `AuthState` añade `refresh_tokens: Arc<Mutex<HashMap<String, String>>>` en memoria.
- Callback: guarda el `refresh_token` bajo el `jti` y emite la sesión.
- Gate: si hay `jti` y token, llama a `token_endpoint` con `grant_type=refresh_token`; verifica el ID token nuevo sin exigir `nonce`; emite cookie con el mismo `jti` y actualiza el token si rota.
- `invalid_grant` → borra el token y devuelve `401` con cookie de borrado.
- Red/5xx → registra y renueva localmente como hoy.

## TDD

- [ ] RED: captura, renovación, rotación, `invalid_grant`, proveedor caído y sin token.
- [ ] GREEN: captura, `jti`, mapa en memoria, refresh en el gate, verificación sin `nonce`.
- [ ] REFACTOR: comentarios y documentación.
- [ ] Verificación: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --lib`, `just check-all`.

## Riesgos

- El `refresh_token` se pierde al reiniciar; la sesión local sigue válida pero sin refresh hasta el siguiente login.
- El refresh añade una petición al proveedor cada ~6 h; un proveedor lento retrasa la renovación.
- Un `invalid_grant` cierra la sesión aunque el usuario pudiera seguir: es el comportamiento deseado ante revocación.

## Cierre

- [ ] `docs/refresh-tokens-proveedor.md`.
- [ ] `HANDOFF.md` actualizado.
- [ ] Commit, push y PR a `development`.
- [ ] Change y plan archivados; issue #45 cerrada.
