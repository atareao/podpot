# PLAN-020 — Deduplicar el refresco forzado del JWKS

## Issue

#41 — Deduplicar el refresco forzado del JWKS entre callbacks concurrentes.

## Objetivo

Que N callbacks concurrentes con un `kid` rotado paguen una sola petición extra al `jwks_uri`, reutilizando el conjunto que ya refrescó otro callback.

## Contexto

- `backend/src/auth/oidc.rs`: `JwksCacheado` guarda `conjunto` y `leido_en`; `refrescar_jwks` ya deduplica el refresco por caducidad bajo el candado de escritura.
- El refresco forzado se saltaba la comprobación de frescura, así que cada callback concurrente volvía a pedir el conjunto.
- `docs/cache-jwks-oidc.md` dejaba este caso como follow-up.

## Diseño

- Devolver del camino de lectura el conjunto **y** su `leido_en`.
- Pasar al refresco forzado la marca observada.
- Bajo el candado, si la entrada actual tiene `leido_en` posterior a la observada, devolverla sin pedir el `jwks_uri`.
- Mantener el refresco por caducidad igual.

## TDD

- [x] RED: test concurrente con `kid` rotado (smoke end-to-end) y test determinista con la marca de lectura; antes de la deduplicación, el segundo refresco forzado repetía la petición.
- [x] GREEN: dedupe con la marca de lectura.
- [x] REFACTOR: comentarios y documentación.
- [x] Verificación: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --lib`, `just check-all`.

## Riesgos

- La marca usa `Instant`; dos refrescos en el mismo instante podrían no deduplicarse. Es inocuo: como mucho se paga una petición de más.
- No cambia el contrato HTTP ni el comportamiento ante fallos del proveedor.

## Cierre

- [x] `docs/dedupe-jwks-rotacion.md`.
- [ ] `HANDOFF.md` actualizado.
- [ ] Commit, push y PR a `development`.
- [ ] Change y plan archivados; issue #41 cerrada.
