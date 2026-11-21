# Design — autenticacion-oidc

## Contexto

podpot es hoy una API + panel **sin autenticación**, atado a loopback por decisión explícita. La superficie es: `/health` (público por naturaleza), el listado/acciones de unidades, la creación/edición/borrado de quadlets, y el **stream SSE** en vivo. El router no tiene ninguna capa middleware: es un punto de inserción limpio.

kesto resuelve lo mismo con un flujo OIDC **escrito a mano** (no `openidconnect`/`tower-sessions`, ausentes de la caché de Cargo offline) y una sesión **JWT en cookie** sin estado. Se reutiliza ese patrón tal cual, adaptado a los nombres y al modelo de config de podpot.

## Decisiones

### Modo `Disabled` por defecto → retrocompatible

Sin variables OIDC, `AuthMode::Disabled`: la app arranca como hoy, sin gate, sin rutas de auth activas, panel abierto. Es la garantía de que la entrega no cambia el comportamiento actual en dev (loopback). El modo `Oidc` se activa **solo** cuando están las tres variables del proveedor; una definición parcial es un **error de arranque** que nombra la variable que falta (nada de modos a medias).

### Flujo OIDC a mano (patrón kesto), sin crates pesados

Se replica kesto: `jsonwebtoken` (rust_crypto, use_pem) para verificar el ID token y firmar la sesión; `reqwest` (rustls-tls, json) para discovery/canje/JWKS; `axum-extra` (cookie) para `CookieJar`; `rand`/`urlencoding` para `state`/`nonce`. **No** se añaden `openidconnect` ni `tower-sessions`: no están en la caché y kesto ya demuestra que no hacen falta.

Se mantienen las mismas defensas de kesto: `state` de un solo uso con caducidad (anti-CSRF), `nonce` verificado en el ID token, `MARGEN_DE_RELOJ` para el reloj, `SecretString` (zeroize) para el secreto, y `HttpOnly`+`SameSite=Strict` en la cookie.

### Sesión = JWT en cookie, sin estado

`Claims{sub, email, iat, exp}` con `#[serde(deny_unknown_fields)]`, firmado **HS256** con `PODPOT_SECRET`. Cookie `HttpOnly`, `SameSite=Strict`, `Path=/`, y `Secure` **solo si** el `redirect_url` es https (dev en http debe poder loguear). No hay store de sesiones en servidor: el gate verifica la firma y la caducidad en cada petición. Caducidad alineada con kesto (8 h).

### Partición del router: lista blanca pública + mitad protegida

`api/mod.rs` separa la superficie en dos:

- **Público**: `GET /health`, `GET /auth/login`, `GET /auth/callback`.
- **Negocio** (tras el gate): unidades, acciones, quadlets, **`/units/stream`**, `POST /auth/logout`, `GET /auth/session`.

El gate se aplica con `.route_layer(from_fn_with_state(...))` **sobre la mitad protegida**, después del `method_not_allowed_fallback` (orden semántico: un método no permitido da 405, no 401). Sin credencial → **401 con la envoltura** `{error}`, nunca 302/HTML: es una API, y el frontend necesita distinguir «sin sesión» de «otro error».

### El SSE no cambia: lo cubre el gate por construcción

`/units/stream` vive en la mitad protegida, así que el gate corre **antes** del handler: si no hay sesión, responde 401 y **no se abre** el flujo `text/event-stream`. El handler de stream no sabe nada de auth. `EventSource` envía cookies del mismo origen, así que la sesión viaja sola.

### `auth_mode` en `/health`

El panel necesita saber si debe exigir sesión **antes** de llamar a rutas protegidas (si no, la primera carga daría 401 en modo Oidc sin haber intentado login). `/health` —público— publica `auth_mode` (`oidc`|`disabled`); el frontend decide con eso si monta la guarda.

### Frontend: portada/login + guarda + 401 global

- Portada pública con botón de login (`href="/api/v1/auth/login"`, navegación real → el 302 lo hace el servidor).
- Guarda de ruta: si `auth_mode === 'oidc'` y no hay sesión → redirige a la portada; si `disabled`, deja pasar (dev).
- Reacción global al 401 en `clienteDeConsultas` (`onError` de `QueryCache`/`MutationCache`): invalida la consulta de sesión → la guarda redirige. Mismo patrón que kesto.

## Alternativas descartadas

- **`openidconnect`/`tower-sessions`**: no están en la caché de Cargo (el repo compila offline); kesto ya demostró que el flujo a mano es suficiente.
- **Sesión con estado en servidor** (tabla o store): el JWT en cookie es más simple, sin estado, y ya probado.
- **302 al login desde el gate**: rompería la API (el frontend recibiría HTML); el gate devuelve 401 envuelto y el frontend decide.
- **Proteger el SSE dentro del handler**: innecesario; el gate lo cubre por estar en la mitad protegida.
- **Auth obligatoria siempre**: rompería el dev en loopback; `Disabled` por defecto lo preserva.
