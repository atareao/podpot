# Tasks — discovery-caido-en-refresco-jwks

## 1. Baseline

- [x] 1.1 Ejecutar `cargo test --lib` y confirmar verde.
- [x] 1.2 Crear la rama `feature/discovery-caido-en-refresco-jwks` desde `development`.

## 2. RED

- [x] 2.1 Añadir `tumbar_el_descubrimiento` al `ProveedorFalso`.
- [x] 2.2 Test: descubrimiento caído durante el refresco del JWKS → `502`, sin sesión, caché intacta.
- [x] 2.3 Confirmado: el test pasa de entrada (caracterización del camino ya implementado); no es un fallo por compilación.

## 3. GREEN

- [x] 3.1 Sin cambios de producción: el camino ya propagaba el error sin escribir la caché.
- [x] 3.2 Ejecutar tests focalizados y confirmar verde.

## 4. REFACTOR y verificación

- [x] 4.1 Actualizar `docs/cache-jwks-oidc.md` (follow-up retirado por estar cubierto).
- [x] 4.2 Ejecutar `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test --lib`.
- [x] 4.3 Ejecutar `just check-all`.

## 5. Cierre

- [x] 5.1 Crear `docs/discovery-caido-en-refresco-jwks.md`.
- [ ] 5.2 Actualizar `HANDOFF.md`.
- [ ] 5.3 Commit gitmoji, push y PR a `development`.
- [ ] 5.4 Archivar el change y el plan tras el merge.
