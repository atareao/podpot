# PLAN-021 — Cubrir el descubrimiento caído durante el refresco del JWKS

## Issue

#43 — Cubrir el descubrimiento caído durante el refresco del JWKS.

## Objetivo

Cubrir con test el camino en el que `refrescar_bajo_candado` necesita el descubrimiento y este falla: `502` con la envoltura existente, sin sesión y sin envenenar la caché anterior.

## Contexto

- `backend/src/auth/oidc.rs`: `refrescar_bajo_candado` resuelve primero `discovery_cached` y después `fetch_jwks`; el fallo del descubrimiento se propaga antes de escribir la caché.
- `backend/src/test_support.rs`: `ProveedorFalso` sirve el descubrimiento con un `Mock` fijo; falta poder tumbar ese endpoint.
- `docs/cache-jwks-oidc.md`: deja este caso como follow-up.

## Diseño

- Añadir al doble `caido_descubrimiento: Arc<AtomicBool>` y `tumbar_el_descubrimiento()`, y que el handler del descubrimiento consulte la bandera.
- Test: sembrar la caché de JWKS, vaciar la caché del descubrimiento, tumbar el descubrimiento, rotar la clave, verificar → `502`; y comprobar que la caché de JWKS anterior sigue sirviendo.

## TDD

- [x] RED: test del escenario.
- [x] GREEN: sin cambios de producción (el camino ya era correcto).
- [x] REFACTOR: comentarios y documentación.
- [x] Verificación: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --lib`, `just check-all`.

## Riesgos

- El test nace verde: es caracterización, no cambio de conducta. Documentado como tal.
- No cambia el contrato HTTP ni el frontend.

## Cierre

- [x] `docs/discovery-caido-en-refresco-jwks.md`.
- [ ] `HANDOFF.md` actualizado.
- [ ] Commit, push y PR a `development`.
- [ ] Change y plan archivados; issue #43 cerrada.
