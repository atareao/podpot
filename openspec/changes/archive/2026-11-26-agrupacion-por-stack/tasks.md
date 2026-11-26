# Tareas — agrupacion-por-stack

- [x] B1 `[RED]` Tests unitarios de `stack_name`: principal + auxiliares, auxiliar huérfano, `.container` con sufijo `-volume`, `kind: None`, `file_name` ausente, estabilidad ante `active_state`. Verificar `cargo test` en rojo para lo nuevo.
- [x] B2 `[GREEN]` Módulo puro de derivación en backend, campo `stack: String` en `UnitSummary`, población en `GET /api/v1/units`. `cargo test` + `cargo check`.
- [x] B3 `[REFACTOR]` `cargo fmt --all` + `cargo clippy -- -D warnings` + `cargo test`.
- [x] F1 `[RED]` Tests Vitest de `groupByStack` y filtros (incluye lista vacía y orden determinista). Verificar `npx vitest run` en rojo para lo nuevo.
- [x] F2 `[GREEN]` `stack: string` en `frontend/src/types/api.ts`, helper de agrupación y render en `<TablaDeUnidades>` con cabeceras y filtros. `npx vitest run` + `npx tsc --noEmit`.
- [x] F3 `[REFACTOR]` `pnpm run lint` + tests + build.
- [x] S1 Escribir y presentar esta propuesta al usuario; NO escribir código ni tests hasta su aprobación.
- [x] C1 Auditoría `@rust-reviewer` y `@react-reviewer`, con corrección de hallazgos.
- [x] C2 `just check-all`, `docs/agrupacion-por-stack.md`, PR a `development`, archivado del plan y del change, HANDOFF actualizado.
