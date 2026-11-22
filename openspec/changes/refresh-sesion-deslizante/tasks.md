# Tasks — refresh-sesion-deslizante

## 1. Baseline

- [ ] 1.1 Ejecutar `cargo test --lib` y confirmar verde.
- [ ] 1.2 Crear la rama `feature/refresh-sesion` desde `development`.

## 2. RED

- [ ] 2.1 Tests unitarios en `session.rs`: sesión reciente no se renueva; sesión dentro de la ventana se renueva; la renovada conserva `sub` y `email` y alarga `exp`.
- [ ] 2.2 Tests de integración en el *gate*: petición con sesión próxima a caducar devuelve `200` + `Set-Cookie` válida; con sesión reciente no emite `Set-Cookie`.
- [ ] 2.3 Test negativo: sesión caducada da `401` y no emite `Set-Cookie`.
- [ ] 2.4 Ejecutar los tests y confirmar los fallos esperados.

## 3. GREEN

- [ ] 3.1 Añadir la ventana de renovación y la emisión con identidad en `session.rs`.
- [ ] 3.2 Hacer que el *gate* añada la cookie renovada a la respuesta.
- [ ] 3.3 Ejecutar tests focalizados y confirmar verde.

## 4. REFACTOR y verificación

- [ ] 4.1 Revisar comentarios y documentación del módulo.
- [ ] 4.2 Ejecutar `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test --lib`.
- [ ] 4.3 Ejecutar `just check-all`.

## 5. Cierre

- [ ] 5.1 Crear `docs/refresh-sesion-deslizante.md`.
- [ ] 5.2 Actualizar `HANDOFF.md`.
- [ ] 5.3 Commit gitmoji, push y PR a `development`.
- [ ] 5.4 Archivar el change y el plan tras el merge.
