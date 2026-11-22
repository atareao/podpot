# Tasks — dedupe-jwks-rotacion

## 1. Baseline

- [x] 1.1 Ejecutar `cargo test --lib` y confirmar verde.
- [x] 1.2 Crear la rama `feature/dedupe-jwks-rotacion` desde `development`.

## 2. RED

- [x] 2.1 Añadir test de N verificaciones concurrentes con un `kid` rotado: una sola petición extra al `jwks_uri`.
- [x] 2.2 Confirmar el RED con el test determinista de la marca: antes de la deduplicación, el segundo refresco forzado repetía la petición. El test concurrente es un smoke end-to-end, no el RED determinista: con `Barrier` puede leer la caché vieja y deduplicar, así que su fallo depende del entrelazado.

## 3. GREEN

- [x] 3.1 Exponer la marca de lectura de la caché junto al conjunto.
- [x] 3.2 En el refresco forzado, reutilizar el conjunto si otro callback ya refrescó desde la lectura observada.
- [x] 3.3 Ejecutar tests focalizados y confirmar verde.

## 4. REFACTOR y verificación

- [x] 4.1 Actualizar comentarios y `docs/cache-jwks-oidc.md`.
- [x] 4.2 Ejecutar `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test --lib`.
- [x] 4.3 Ejecutar `just check-all`.

## 5. Cierre

- [x] 5.1 Crear `docs/dedupe-jwks-rotacion.md`.
- [ ] 5.2 Actualizar `HANDOFF.md`.
- [ ] 5.3 Commit gitmoji, push y PR a `development`.
- [ ] 5.4 Archivar el change y el plan tras el merge.
