# autenticacion-oidc — inicio de sesión OIDC opcional

## Qué es

Autenticación **opcional** para poder exponer el panel fuera de loopback. Por defecto (`disabled`) la app sigue abierta como hoy; con las variables del proveedor (`oidc`) exige sesión para toda la superficie de negocio mediante un **flujo OIDC** y una **sesión JWT en cookie**. Reutiliza el patrón ya probado de **kesto**.

Issue: #12 · Change OpenSpec: `openspec/changes/archive/2026-11-21-autenticacion-oidc` · Plan: `plans/PLAN-006-ARCHIVED.md`.

## Ficheros implicados

Backend (`backend/src/`):
- `config.rs` — `AuthMode` (`Disabled`|`Oidc`), `OidcSettings`, `secret`; vars `PODPOT_OIDC_*` y `PODPOT_SECRET`; regla «las tres OIDC juntas o ninguna»; `MIN_SECRET_LEN`.
- `secret.rs` — `SecretString` (redacta en `Debug`/`Display`).
- `auth/oidc.rs` — discovery, `authorization_endpoint` (`state`+`nonce`), canje del código, verificación del ID token (firma RS256 vía JWKS, `iss`/`aud`/`exp`/`nonce`).
- `auth/session.rs` — JWT HS256 en cookie `HttpOnly`/`SameSite=Strict`/`Secure` si https; `Claims{sub,email,iat,exp}` con `deny_unknown_fields`.
- `auth/state.rs` — cache de discovery y `state`/`nonce` de un solo uso (TTL 5 min).
- `auth/middleware.rs` — el `gate`: verifica la cookie y, sin sesión, responde **401 envuelto**.
- `api/auth.rs` — `GET /auth/login`, `GET /auth/callback`, `POST /auth/logout`, `GET /auth/session`.
- `api/mod.rs` — partición público/negocio y `route_layer` del gate.
- `api/health.rs` — publica `auth_mode` y `version`.

Frontend (`frontend/src/`):
- `api/sesion.ts` + `hooks/useSesion.ts` — salud/sesión/cierre.
- `pages/portada.tsx` — portada pública con botón-enlace a `/api/v1/auth/login`.
- `components/SoloConSesion/SoloConSesion.tsx` — guarda de ruta.
- `api/clienteDeConsultas.ts` — reacción global al 401; `api/envelope.ts` — `esErrorDeSesion`.

## Cómo funciona

1. **Modo**: al arrancar, si están las tres variables OIDC → `oidc` (y se exige `PODPOT_SECRET`); si no hay ninguna → `disabled`; si es parcial → error de arranque nombrando la variable que falta.
2. **Login**: `GET /api/v1/auth/login` redirige (302) al proveedor con un `state` de un solo uso y un `nonce`. El callback valida el `state`, canjea el código, verifica el ID token (firma contra el JWKS, `iss`/`aud`/`exp`, `nonce`) y emite la cookie de sesión, redirigiendo al panel.
3. **Gate**: en modo `oidc`, la superficie de negocio (unidades, acciones, quadlets, **`/units/stream`**, logout, session) se sirve tras el gate; sin sesión válida responde **401 con la envoltura** `{error}` (nunca 302/HTML). La salud, el login y el callback son públicos. El SSE queda protegido por construcción: sin sesión no se abre el flujo.
4. **Frontend**: consulta `/health` (`auth_mode`); si es `oidc` y no hay sesión, muestra la portada con el botón de login (navegación real); si es `disabled`, deja pasar. Ante un `401`, invalida la sesión y vuelve a la portada.

## Contrato

- `GET /api/v1/health` (público) → `{data:{status, auth_mode, version}}`.
- `GET /api/v1/auth/login` → `302` al proveedor.
- `GET /api/v1/auth/callback` → `302` al panel + `Set-Cookie` de sesión.
- `POST /api/v1/auth/logout` (protegida) → borra la cookie.
- `GET /api/v1/auth/session` (protegida) → identidad de la sesión.
- Cualquier ruta de negocio sin sesión (modo `oidc`) → `401` envuelto.

## Configuración / despliegue

Variables (modo `oidc`): `PODPOT_OIDC_ISSUER_URL`, `PODPOT_OIDC_CLIENT_ID`, `PODPOT_OIDC_CLIENT_SECRET`, `PODPOT_OIDC_REDIRECT_URL` (default `http://127.0.0.1:8080/api/v1/auth/callback`), y `PODPOT_SECRET` (≥ 32 bytes, firma de la sesión). Sin ellas, `disabled` (dev en loopback).

Para exponer: define las variables, usa un `redirect_url` con el host público, pon `BIND_ADDRESS` no-loopback y sirve tras TLS (la cookie se marca `Secure` automáticamente si el redirect es https).

## Decisiones

- **`Disabled` por defecto**: retrocompatible; no cambia el dev en loopback.
- **Flujo OIDC a mano** (patrón kesto): `jsonwebtoken` + `reqwest` + `axum-extra` cookie; **sin** `openidconnect`/`tower-sessions` (fuera de la caché offline).
- **Sesión JWT en cookie**, sin estado en servidor.
- **Gate con 401 envuelto** (API, no HTML); el SSE lo cubre por estar en la mitad protegida.
- **`method_not_allowed_fallback` tras el gate**: un método no permitido da 405, no 401.
- Defensas candadas por tests negativos (con mutación): algoritmo **fijado** a RS256 (rechaza `alg:none` y la confusión HS256), `iss`/`aud`/`exp`/`iat`/`kid`, `nonce`, `deny_unknown_fields` de los claims y longitud mínima del secreto.

## Follow-ups

- **JWKS sin caché**: se pide en cada callback (paridad con kesto); considerar cachearlo si se expone en producción.
- **Logout con cookie inválida**: el gate responde 401, así que el cliente debe descartar la cookie en cliente.
- **Autorización por rol/permiso** y **refresh de tokens** contra el proveedor (fuera de alcance).
- Endurecer el tope del mapa de `state` si `/auth/login` llega a estar muy expuesto.
