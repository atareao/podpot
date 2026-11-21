# Tasks — unificar-error-persistencia

Refactor sin cambio de comportamiento. La red de seguridad son los tests existentes del mapeo de errores: primero se confirman en verde, luego se refactoriza, luego se vuelve a verde.

## 1. Caracterización (verde de partida)

- [ ] 1.1 Confirmar que `backend/src/error.rs` (`mod tests`) y `backend/src/api/quadlets.rs:804` fijan `internal` (`500`) para el fallo de persistencia. — Verificación: `cargo test` EXIT 0 antes de tocar código.

## 2. Refactor del dominio

- [ ] 2.1 Eliminar la variante `QuadletError::Registro` (`backend/src/quadlet/mod.rs:244`) y documentar en `Persistencia` que cubre creación y edición/borrado. — Verificación: `cargo check` sin usos huérfanos.
- [ ] 2.2 Repuntar las construcciones en `backend/src/quadlet/service.rs:209` a `QuadletError::Persistencia` y unificar los dos `map_err`. — Verificación: compila y el test de edición/borrado sigue verde.
- [ ] 2.3 Eliminar el brazo `QuadletError::Registro` de `traducir_quadlet` (`backend/src/error.rs:423`) y su test asociado. — Verificación: `cargo test` EXIT 0 y `Persistencia → internal/500` sigue verificado.
- [ ] 2.4 Asegurar cobertura de `Persistencia → internal/500` en el camino de edición/borrado (reutilizar/renombrar `quadlets.rs:804`). — Verificación: test verde ligado a `Persistencia`.

## 3. Limpieza y cierre

- [ ] 3.1 `rg "QuadletError::Registro"` sin resultados; `cargo clippy -- -D warnings` y `cargo fmt --check` limpios. — Verificación: salida vacía / EXIT 0.
- [ ] 3.2 Actualizar `docs/edicion-borrado-quadlets.md` (líneas 10, 50, 55) y `docs/FEATURE-002-creacion-quadlets.md:141`. — Verificación: doc refleja una única variante.
- [ ] 3.3 `just check-all` EXIT 0; `rust-reviewer` sin bloqueantes; `openspec validate --all` verde. — Verificación: informes y validación.
