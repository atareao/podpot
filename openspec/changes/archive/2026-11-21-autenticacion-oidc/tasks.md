# Tasks — autenticacion-oidc

TDD por bloques: primero las pruebas (rojo), luego la implementación mínima (verde), luego refactor. Orden por dependencia: config → sesión → OIDC → gate/rutas → salud → SSE → frontend → cierre.

## 1. Configuración

- [x] 1.1 `AuthMode`/`OidcSettings`/`secret` en `config.rs`; variables `PODPOT_OIDC_*` y `PODPOT_SECRET`. — Verificación: tests de las tres-juntas/ninguna/parcial/redirect por defecto/secreto faltante (rojo→verde).
- [x] 1.2 Dependencias nuevas en `Cargo.toml` (`jsonwebtoken`, `reqwest`, `axum-extra` cookie, `rand`, `urlencoding`, `zeroize`). — Verificación: `cargo check` compila.

## 2. Sesión

- [x] 2.1 `auth/session.rs`: `Claims`, `issue`, `verify` (HS256, con `leeway`), `session_cookie` (`HttpOnly`/`SameSite=Strict`/`Secure` según https). — Verificación: tests de emisión/verificación, caducidad, firma inválida y atributos de la cookie.
- [x] 2.2 `auth/state.rs`/`AuthState`: cache de discovery, registro de `state`/`nonce` de un solo uso con caducidad. — Verificación: tests de state desconocido/caducado/reutilizado.

## 3. Flujo OIDC

- [x] 3.1 Discovery (`/.well-known/openid-configuration`) con timeout y cache. — Verificación: test con servidor simulado (o doble) que devuelve el documento.
- [x] 3.2 `build_authorization_url` (con `state`+`nonce`) y `canjear_codigo`. — Verificación: test que comprueba los parámetros de la URL y el canje.
- [x] 3.3 `verificar_id_token` (firma vía JWKS + `nonce` + `exp`). — Verificación: tests de token válido, firma inválida, `nonce` distinto.

## 4. Rutas y gate

- [x] 4.1 `api/auth.rs`: `login` (302), `callback` (302 + `Set-Cookie`), `logout` (borra cookie), `session` (identidad). — Verificación: tests de integración de las cuatro rutas.
- [x] 4.2 `auth/middleware.rs`: gate que verifica la cookie y, sin sesión, responde **401 envuelto** (nunca 302/HTML). — Verificación: test de ruta protegida sin/con sesión.
- [x] 4.3 `api/mod.rs`: partición público/negocio; `route_layer(from_fn_with_state)` tras `method_not_allowed_fallback`. — Verificación: tests de 401 en negocio, 200 en salud/login/callback, 405 preservado.
- [x] 4.4 Modo `disabled`: no se monta el gate; la superficie queda accesible. — Verificación: test de superficie accesible sin sesión en `disabled`.

## 5. Salud y SSE

- [x] 5.1 `GET /health` publica `auth_mode`. — Verificación: test que comprueba `oidc`/`disabled` según config.
- [x] 5.2 El SSE queda en la mitad protegida: sin sesión → 401 y no se abre el flujo; con sesión → 200 `text/event-stream`. — Verificación: tests de integración del stream en ambos casos.

## 6. Frontend

- [x] 6.1 Helper de sesión (`esErrorDeSesion`, consulta de salud/sesión). — Verificación: tests de las funciones.
- [x] 6.2 Portada pública con botón de login; guarda de ruta según `auth_mode`. — Verificación: tests (disabled deja pasar; oidc sin sesión muestra portada).
- [x] 6.3 Reacción global al 401 (invalidar sesión → portada). — Verificación: test de `clienteDeConsultas` con error 401.

## 7. Cierre

- [x] 7.1 `just check-all` en verde y `clippy -D warnings` limpio. — Verificación: salida EXIT 0.
- [x] 7.2 Reviewers (`rust-reviewer`, `react-reviewer`) sin bloqueantes. — Verificación: informes de review.
- [x] 7.3 `openspec validate --all` en verde y `docs/autenticacion-oidc.md` redactado. — Verificación: validación OK y doc existente.
