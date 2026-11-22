# PLAN-018 — Logout tolerante a cookie inválida

- **Issue:** #37 (https://github.com/atareao/podpot/issues/37)
- **Change OpenSpec:** `logout-cookie-invalida`
- **Rama:** `feature/logout-cookie-invalida` desde `development`
- **Alcance:** backend. Sin cambios de frontend ni de contrato de datos.

## Objetivo

Que `POST /api/v1/auth/logout` borre siempre la cookie de sesión, aunque la cookie exista pero no verifique (caducada, firma inválida o secreto rotado) o falte. Hoy el *gate* responde `401` antes de llegar al manejador y la cookie no se borra: el usuario queda atrapado con una cookie inservible.

## Contexto y decisiones

- El `logout` es idempotente: su único efecto es borrar la cookie. No necesita identidad.
- Se saca del *gate* de sesión y se declara ruta pública en modo `oidc`, junto a salud, login y callback.
- Sigue siendo `POST` (nunca `GET`) y conserva la `Set-Cookie` con el mismo `Path` y `Max-Age=0`.
- El resto de rutas de negocio siguen protegidas y devuelven `401` sin sesión válida.
- Riesgo de CSRF de logout: bajo (máximo, forzar un cierre de sesión); se acepta porque la operación es idempotente y no destructiva.
- Sin cambios de frontend: `cerrarLaSesion()` ya hace `POST /auth/logout`.

## Archivos implicados

- `backend/src/api/mod.rs` (partición de rutas y *gate*)
- `backend/src/api/auth.rs` (manejador y tests)
- `backend/src/api/mod.rs` (tests de la partición)
- `openspec/specs/autenticacion/spec.md` y `openspec/specs/api-shell/spec.md` (vía el change)

## Tareas (SDD + TDD)

### 1. Baseline

- [ ] 1.1 Ejecutar `cargo test` y confirmar verde.
- [ ] 1.2 Confirmar rama limpia y crear `feature/logout-cookie-invalida` desde `development`.

### 2. RED

- [ ] 2.1 Añadir tests: logout con cookie inválida → `200` + `Set-Cookie` de borrado.
- [ ] 2.2 Añadir tests: logout sin cookie → `200` + `Set-Cookie` de borrado.
- [ ] 2.3 Actualizar el test de la partición: logout deja de ser ruta protegida.
- [ ] 2.4 Ejecutar los tests y confirmar los fallos esperados.

### 3. GREEN

- [ ] 3.1 Mover `/auth/logout` a la mitad pública del router.
- [ ] 3.2 Mantener el manejador `logout` y la cookie de borrado tal cual.
- [ ] 3.3 Ejecutar tests focalizados y confirmar verde.

### 4. REFACTOR y verificación

- [ ] 4.1 Revisar que el resto de rutas sigue devolviendo `401` sin sesión.
- [ ] 4.2 Ejecutar `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test`.
- [ ] 4.3 Ejecutar `just check-all`.

### 5. Cierre

- [ ] 5.1 Crear `docs/logout-cookie-invalida.md`.
- [ ] 5.2 Actualizar `HANDOFF.md`.
- [ ] 5.3 Commit gitmoji, push y PR a `development`.
- [ ] 5.4 Archivar el change y el plan tras el merge.

## Criterios de aceptación

- [ ] `POST /auth/logout` con cookie inválida devuelve `200` y una `Set-Cookie` de borrado.
- [ ] `POST /auth/logout` sin cookie devuelve `200` y una `Set-Cookie` de borrado.
- [ ] El resto de rutas de negocio siguen devolviendo `401` sin sesión válida.
- [ ] Tests, typecheck, lint y `just check-all` en verde.
