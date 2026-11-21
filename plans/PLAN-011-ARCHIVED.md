# PLAN-011 — Caché de JWKS en el módulo OIDC

- **Issue**: #23 — «Caché de JWKS en el módulo OIDC»
- **Feature**: `cache-jwks-oidc`
- **Tipo**: refactor / rendimiento (♻️), acotado a backend
- **Estado**: implementado (RED/GREEN/REFACTOR y tests en verde) — pendiente de PR a `development` y archivado del plan

## Objetivo

Dejar de pedir el `jwks_uri` al proveedor en **cada callback** del flujo OIDC. Hoy
`verificar_id_token` llama a `fetch_jwks(&discovery.jwks_uri)` de forma incondicional,
una petición HTTP por login, aunque la clave del `kid` no haya cambiado. Se añade una
caché del `JwkSet` análoga a la caché de descubrimiento ya existente.

## Mapa arquitectónico (puntos de integración reales)

- `backend/src/auth/state.rs` — `AuthState` ya cachea el descubrimiento en
  `pub discovery: Arc<RwLock<Option<Discovery>>>` (línea 87), construido en `AuthState::new`
  (línea 108). **Aquí se añade la caché del JWKS** siguiendo el mismo patrón.
- `backend/src/auth/oidc.rs` — `discovery_cached(auth: &AuthState)` (línea 128) es el
  **precedente exacto**: lee la caché, y si está vacía llama a `fetch_discovery` y la
  escribe. `fetch_jwks(url)` (línea 331) es la función que hoy se llama sin caché desde
  `verificar_id_token` (línea 377). `verificar_id_token` recibe hoy
  `(settings: &OidcSettings, discovery: &Discovery, id_token, nonce_esperado)`.
- `backend/src/api/auth.rs:168` — el único llamador de producción de `verificar_id_token`
  (el callback), que ya tiene `auth: &AuthState` en la mano.
- `backend/src/test_support.rs` — doble del proveedor con ruta `/jwks` (línea 647+) y clave
  de firma real vía `wiremock`; es el sitio para instrumentar conteo de peticiones y rotación
  de clave.

## Decisiones a cerrar en la spec (OpenSpec)

1. **Estrategia de frescura de la caché.** La caché NO puede servir indefinidamente una clave
   retirada por el proveedor. Opciones a especificar:
   - TTL fijo (p. ej. `JWKS_TTL`, tipo `Duration`, reutilizando el criterio de `INTERCAMBIO_TIMEOUT`),
     tras el cual se vuelve a pedir.
   - **Revalidación ante `kid` ausente**: si el ID token trae un `kid` que el conjunto cacheado
     no publica, forzar un refresco (una sola vez) antes de rechazar. Protege ante rotación de
     claves dentro del TTL.
   - Decidir la combinación (probablemente TTL + refresco condicional por `kid` desconocido).
2. **Firma de `verificar_id_token`.** Evaluar cambiar la firma para recibir `&AuthState` (como
   `discovery_cached`) en lugar de `settings` + `discovery` sueltos, o introducir
   `jwks_cached(auth)` y pasar el `JwkSet` ya resuelto. La spec fija la que se elija.
3. **Tipo cacheado.** `JwkSet` (`jsonwebtoken::jwk::JwkSet`) tras `Arc<RwLock<Option<…>>>`,
   igual que `Discovery`.

## Tareas (TDD — cada una con su test primero)

1. [x] Abrir el change OpenSpec `openspec/changes/<feature>/` con proposal + spec delta (capability
       `autenticacion`/`oidc`) + tasks. Escenarios BDD: reutilización dentro del TTL, expiración,
       `kid` desconocido → refresco, y proveedor caído en refresco → `502`.
2. [x] **RED**: tests en `backend/src/auth/oidc.rs` (y `test_support.rs` para el conteo de
       peticiones al `/jwks` del doble) que exijan: (a) dos callbacks seguidos dentro del TTL →
       **una sola** petición al `jwks_uri`; (b) tras el TTL → segunda petición; (c) `kid` que el
       conjunto cacheado no publica → se pide de nuevo y, si el refresco lo publica, se acepta;
       (d) refresco que falla → `AppError::BadGateway` (`502`).
3. [x] **GREEN**: añadir `jwks: Arc<RwLock<Option<…>>>` a `AuthState` (+ su inicialización en
       `new`) y `jwks_cached(auth)` en `oidc.rs`; cablear `verificar_id_token` y el llamador
       `api/auth.rs`. Mínimo cambio para pasar los tests.
4. [x] **REFACTOR**: alinear nombres/documentación con el estilo del módulo (comentarios en
       español explicando por qué TTL y por qué refresco condicional), `cargo fmt`, `cargo clippy
       -- -D warnings`.
5. [x] `plans/PLAN-011.md` → marcar tareas y renombrar a `PLAN-011-ARCHIVED.md` al cerrar; crear
       `docs/cache-jwks-oidc.md`.
6. [x] Cerrar con PR a `development` (gitflow) referenciando el issue #23.

## Fuera de alcance

- Cambios en el contrato HTTP visible al cliente (sigue rechazando con `401`/`internal`).
- Caché/refresco del **documento de descubrimiento** (evaluar aparte, ya tiene su `discovery_cached`
  perpetuo).
- Autorización por rol, refresh de tokens.

## Riesgos

- Servir una clave rotada más allá de su validez si el TTL es demasiado largo: mitigado por el
  refresco ante `kid` desconocido.
- Condición de carrera en el refresco concurrente (varios callbacks a la vez): la spec debe fijar
  si se admite duplicar la petición o se serializa con el `RwLock`.
