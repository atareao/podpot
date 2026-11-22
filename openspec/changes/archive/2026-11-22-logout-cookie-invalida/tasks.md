# Tasks — logout-cookie-invalida

## 1. Baseline

- [x] 1.1 Ejecutar `cargo test --lib` y confirmar verde.
- [x] 1.2 Crear la rama `feature/logout-cookie-invalida` desde `development`.

## 2. RED

- [x] 2.1 Añadir test de logout sin cookie: `200` + `Set-Cookie` de borrado.
- [x] 2.2 Añadir test de logout con cookie inválida: `200` + `Set-Cookie` de borrado.
- [x] 2.3 Actualizar el test de la partición: logout público.
- [x] 2.4 Ejecutar tests y confirmar los fallos esperados.

## 3. GREEN

- [x] 3.1 Mover `/auth/logout` a la mitad pública del router.
- [x] 3.2 Mantener el manejador y la cookie de borrado.
- [x] 3.3 Ejecutar tests focalizados y confirmar verde.

## 4. REFACTOR y verificación

- [x] 4.1 Actualizar comentarios de documentación (manejador, router y test).
- [x] 4.2 Ejecutar `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test --lib`.
- [x] 4.3 Ejecutar `just check-all`.

## 5. Cierre

- [x] 5.1 Crear `docs/logout-cookie-invalida.md`.
- [x] 5.2 Actualizar `HANDOFF.md`.
- [x] 5.3 Commit gitmoji, push y PR a `development`.
- [x] 5.4 Archivar el change y el plan tras el merge.
