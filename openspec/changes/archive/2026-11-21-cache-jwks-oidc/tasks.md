# Tasks — cache-jwks-oidc

TDD. Cambio acotado a backend, sin cambio del contrato HTTP. Se sigue el patrón ya probado de la caché de descubrimiento (`discovery_cached`).

## 1. RED — tests que fallan

- [x] 1.1 Instrumentar el doble del proveedor en `backend/src/test_support.rs` para contar las peticiones al `/jwks` y poder rotar la clave. — Verificación: `cargo test` compila con el contador.
- [x] 1.2 Test: dos callbacks válidos dentro del TTL → **una sola** petición al `jwks_uri`. — Verificación: falla hoy (hoy hay una por callback).
- [x] 1.3 Test: transcurrido el TTL → se vuelve a pedir el `jwks_uri`. — Verificación: falla hoy.
- [x] 1.4 Test: `kid` que el conjunto cacheado no publica y el refresco lo publica → se acepta el ID token. — Verificación: falla hoy.
- [x] 1.5 Test: refresco del `jwks_uri` que falla → `BadGateway` (`502`) y sin sesión. — Verificación: falla hoy.
- [x] 1.6 `cargo test` EXIT ≠ 0 para lo nuevo y verde para lo existente.

## 2. GREEN — implementación mínima

- [x] 2.1 Añadir `jwks: Arc<RwLock<Option<…>>>` a `AuthState` (`backend/src/auth/state.rs`) e inicializarlo en `AuthState::new`. — Verificación: `cargo check`.
- [x] 2.2 Añadir `jwks_cached(auth: &AuthState)` en `backend/src/auth/oidc.rs` con TTL y refresco ante `kid` desconocido; cablear `verificar_id_token` y el llamador `backend/src/api/auth.rs:168`. — Verificación: los tests 1.2–1.5 en verde.
- [x] 2.3 `cargo test` EXIT 0 (los 366 existentes + los nuevos).

## 3. REFACTOR — limpieza

- [x] 3.1 Alinear nombres, comentarios y documentación del módulo con el estilo existente (por qué TTL, por qué refresco condicional, qué pasa con la concurrencia del refresco). — Verificación: lectura.
- [x] 3.2 `cargo fmt --check` y `cargo clippy -- -D warnings` limpios. — Verificación: EXIT 0.
- [x] 3.3 `just check-all` EXIT 0; `rust-reviewer` sin bloqueantes; `openspec validate --all` verde.
- [x] 3.4 Crear `docs/cache-jwks-oidc.md`; marcar tareas y renombrar el plan a `plans/PLAN-011-ARCHIVED.md`; PR a `development` referenciando el issue #23.
