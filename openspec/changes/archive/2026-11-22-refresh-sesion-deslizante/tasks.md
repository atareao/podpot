# Tasks — refresh-sesion-deslizante

## 1. Baseline

- [x] 1.1 Ejecutar `cargo test --lib` y confirmar verde.
- [x] 1.2 Crear la rama `feature/refresh-sesion` desde `development`.

## 2. RED

- [x] 2.1 Tests unitarios en `session.rs`: fuera de ventana, borde y dentro; `renew` conserva identidad.
- [x] 2.2 Tests de integración: renovación dentro de la ventana; siguiente petición con la cookie nueva; `/auth/session`.
- [x] 2.3 Tests de no renovación: sesión reciente y cookie caducada.
- [x] 2.4 Ejecutar tests y confirmar los fallos esperados.

## 3. GREEN

- [x] 3.1 Añadir `RENEWAL_WINDOW`, `should_renew` y `renew`.
- [x] 3.2 El *gate* añade la cookie renovada con `SET_COOKIE`.
- [x] 3.3 Ejecutar tests focalizados y confirmar verde.

## 4. REFACTOR y verificación

- [x] 4.1 Documentar el *gate* y los riesgos (sin tope absoluto, carreras).
- [x] 4.2 Cubrir SSE, firma inválida, cuerpo intacto y `Secure` con https.
- [x] 4.3 Ejecutar `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y `cargo test --lib`.
- [x] 4.4 Ejecutar `just check-all`.

## 5. Cierre

- [x] 5.1 Crear `docs/refresh-sesion-deslizante.md`.
- [x] 5.2 Actualizar `HANDOFF.md`.
- [x] 5.3 Commit gitmoji, push y PR a `development`.
- [x] 5.4 Archivar el change y el plan tras el merge.
