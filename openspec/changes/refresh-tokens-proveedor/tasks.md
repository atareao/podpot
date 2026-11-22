# Tasks — refresh-tokens-proveedor

## 1. Baseline

- [x] 1.1 Ejecutar `cargo test --lib` y confirmar verde.
- [x] 1.2 Crear la rama `feature/refresh-tokens-proveedor` desde `development`.

## 2. RED

- [x] 2.1 Test de captura: el canje con `refresh_token` emite sesión con `jti` y asocia el token.
- [x] 2.2 Test de renovación: sesión en ventana con `refresh_token` pide al `token_endpoint` y emite cookie nueva.
- [x] 2.3 Test de rotación: un `refresh_token` nuevo sustituye al anterior.
- [x] 2.4 Test de `invalid_grant`: `401` y cookie borrada.
- [x] 2.5 Test de proveedor caído: se mantiene la sesión local.
- [x] 2.6 Test sin `refresh_token`: renovación local sin cambios.
- [ ] 2.7 Confirmar que los tests fallan por el comportamiento actual y no por compilación. _(No verificable retroactivamente: los tests se escribieron junto a la implementación.)_

## 3. GREEN

- [x] 3.1 `TokenResponse` captura `refresh_token`.
- [x] 3.2 `jti` en los claims y en la emisión de sesión.
- [x] 3.3 Mapa en memoria `jti → refresh_token` en `AuthState`.
- [x] 3.4 Refresh del proveedor en el *gate*; `401` y borrado ante `invalid_grant`.
- [x] 3.5 Verificación del ID token de refresh sin `nonce`.
- [x] 3.6 Ejecutar tests focalizados y confirmar verde.

## 4. REFACTOR y verificación

- [x] 4.1 Actualizar comentarios y `docs/autenticacion-oidc.md`.
- [x] 4.2 Ejecutar `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y `cargo test --lib`.
- [x] 4.3 Ejecutar `just check-all`.

## 5. Cierre

- [x] 5.1 Crear `docs/refresh-tokens-proveedor.md`.
- [ ] 5.2 Actualizar `HANDOFF.md`. _(Fuera del alcance de esta sesión por indicación expresa.)_
- [ ] 5.3 Commit gitmoji, push y PR a `development`. _(Requiere autorización del usuario; no se hace desde esta sesión.)_
- [ ] 5.4 Archivar el change y el plan tras el merge. _(Depende de 5.3.)_
