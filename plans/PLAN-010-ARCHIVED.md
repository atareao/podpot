# PLAN-010 — Unificar las variantes de error Persistencia/Registro de QuadletError

- **Issue:** #20
- **Change OpenSpec:** `unificar-error-persistencia`
- **Rama:** `refactor/unificar-error-persistencia` (desde `development`)
- **Tipo:** ♻️ refactor — sin cambio del contrato público `{error:{code,message}}`

## 1. Objetivo

Eliminar la variante duplicada `QuadletError::Registro` y unificar todo fallo de persistencia del ciclo de vida de una quadlet (creación, edición y borrado) en `QuadletError::Persistencia`. El mapeo HTTP NO cambia: `Persistencia → internal (500)`.

## 2. Mapa arquitectónico

- `backend/src/quadlet/mod.rs`
  - `:229` `QuadletError::Persistencia { file_name, causa: Box<sqlx::Error> }` — variante que SOBREVIVE (creación).
  - `:244` `QuadletError::Registro { file_name, causa }` — variante a ELIMINAR (mismo desenlace, usada en edición/borrado).
- `backend/src/quadlet/service.rs`
  - `:209` construcción de `QuadletError::Registro` (edición/borrado) → pasa a `Persistencia`.
  - `:225` construcción de `QuadletError::Persistencia` (creación) → se mantiene.
  - `:676` comentario de test «Registro tras un borrado» → ajustar redacción.
- `backend/src/error.rs`
  - `:401` brazo `QuadletError::Persistencia → internal (500)` → se mantiene.
  - `:423` brazo `QuadletError::Registro → internal (500)` → se ELIMINA, junto con su test en `mod tests`.
- Tests afectados: `backend/src/api/quadlets.rs:804` (`un_fallo_al_registrar_la_operacion_da_500`, asserta `internal`).
- Docs a actualizar: `docs/edicion-borrado-quadlets.md` (líneas 10, 50, 55) y `docs/FEATURE-002-creacion-quadlets.md:141`.

## 3. Tareas TDD (rojo→verde→refactor)

Este cambio es un **refactor sin cambio de comportamiento**: la red de seguridad son los tests existentes del mapeo de errores.

- [x] 3.1 **Verde de partida (caracterización):** confirmar que `backend/src/error.rs` (`mod tests`) y `backend/src/api/quadlets.rs:804` fijan `internal`/`500` para el fallo de persistencia. — Verificación: `cargo test` EXIT 0 antes de tocar código.
- [x] 3.2 **Dominio:** eliminar la variante `QuadletError::Registro` (`quadlet/mod.rs:244`) y documentar en `Persistencia` que cubre creación + edición/borrado. — Verificación: `cargo check` sin usos huérfanos.
- [x] 3.3 **Call sites:** en `quadlet/service.rs:209` emitir `QuadletError::Persistencia`; unificar los dos `map_err`. — Verificación: compila; el test de edición/borrado sigue verde.
- [x] 3.4 **Traducción:** eliminar el brazo `QuadletError::Registro` de `traducir_quadlet` (`error.rs:423`) y su test. — Verificación: `cargo test` EXIT 0; el test restante de `Persistencia` sigue assertando `internal`/`500`.
- [x] 3.5 **Cobertura:** asegurar que existe un test que verifica `Persistencia → internal/500` en el camino de edición/borrado (reutilizar/renombrar `quadlets.rs:804`). — Verificación: test verde y ligado a `Persistencia`.
- [x] 3.6 **Refactor/limpieza:** `rg "QuadletError::Registro"` vacío; `cargo clippy -- -D warnings` y `cargo fmt --check` limpios.
- [x] 3.7 **Docs:** actualizar `docs/edicion-borrado-quadlets.md` y `docs/FEATURE-002-creacion-quadlets.md` para reflejar una única variante.
- [x] 3.8 **Cierre:** `just check-all` EXIT 0; reviewers (`rust-reviewer`) sin bloqueantes; `openspec validate --all` verde.

## 4. Criterios de aceptación

- [x] No queda ninguna referencia a `QuadletError::Registro`.
- [x] El contrato HTTP es idéntico: `Persistencia → internal (500)`.
- [x] `just check-all` EXIT 0.
- [x] `openspec validate --all` verde.
- [x] `docs/<feature>.md` redactado al cerrar.

## 5. Riesgos

- Bajo. Solo afecta a superficie interna del enum; el mapeo HTTP no cambia.
- Riesgo de test huérfano si se borra el brazo sin borrar su test → cubierto por 3.4/3.6.
