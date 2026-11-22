# Logout tolerante a cookie inválida

## Qué es

Cierre del follow-up OIDC: `POST /api/v1/auth/logout` borra siempre la cookie de sesión, aunque la cookie exista pero no verifique (caducada, firma inválida o secreto rotado) o falte. Antes, el *gate* de sesión devolvía `401` antes de llegar al manejador y la cookie no se borraba: el usuario quedaba atrapado con una cookie inservible.

- Issue: #37
- Change OpenSpec: `logout-cookie-invalida`
- Plan: `plans/PLAN-018.md`

## Ficheros implicados

- `backend/src/api/mod.rs`: `/auth/logout` pasa a la mitad pública del router.
- `backend/src/api/auth.rs`: manejador `logout` sin cambios funcionales; tests de cookie válida, inválida y ausente.
- `openspec/specs/autenticacion/spec.md` y `openspec/specs/api-shell/spec.md`: contrato actualizado.

## Cómo funciona

- El logout es idempotente: su único efecto es borrar la cookie.
- Se declara ruta pública en modo `oidc`, junto a salud, login y callback.
- El manejador emite la cookie de borrado con el mismo `Path` y `Max-Age=0`, sin depender de que la cookie esté presente.
- El resto de rutas de negocio sigue bajo el *gate*: sin sesión válida devuelven `401`.
- El logout sigue siendo `POST` (nunca `GET`).

## Variables y configuración

No aplica: no hay nuevas variables de entorno ni configuración.

## Cómo habilitar o desplegar

No requiere despliegue específico. Se publica con el build normal del backend.

## Decisiones

- Sacar el logout del *gate* es más simple y honesto que hacer que el *gate* emita una `Set-Cookie` de borrado en el `401`: el logout no necesita identidad y debe funcionar siempre.
- Riesgo aceptado: CSRF de logout. Un `POST` cross-site puede forzar el borrado de la cookie; el impacto es bajo (re-login forzado) y `SameSite=Strict` limita el envío de la cookie. La operación es idempotente y no destructiva.
- El contrato `POST /auth/logout` sin sesión válida cambia de `401` a `200` con `Set-Cookie` de borrado.

## Verificación

- `cargo test --lib` → 372 tests en verde.
- `cargo fmt --check` y `cargo clippy -- -D warnings` → sin errores.
- `just check-all` → verde, incluido frontend y build.

## Follow-ups

- Revisar si conviene añadir una defensa CSRF adicional (por ejemplo, comprobar `Origin` o exigir un token) si el logout público se considera demasiado permisivo.
- El resto de follow-ups OIDC (refresh de tokens y dedupe del JWKS) siguen abiertos.
