# PLAN-027 — Agrupación de quadlets por stack

- Issue: #62 — https://github.com/atareao/podpot/issues/62
- Rama: `feature/agrupacion-por-stack` (desde `development`)
- Change de OpenSpec: `openspec/changes/agrupacion-por-stack`
- Documentación resultante: `docs/agrupacion-por-stack.md`

## 1. Objetivo

El panel muestra hoy cada unidad systemd como una fila plana, de modo que una aplicación formada por `foo.container`, `foo-volume.service` y `foo-network.service` aparece como tres entradas sin relación visible. Se quiere un listado **agrupado por stack**, manteniendo el estado en vivo y añadiendo filtros secundarios por tipo y estado. La agrupación se deriva de los datos que ya existen: no se escanean directorios ni se introducen metadatos nuevos.

## 2. Baseline (hallazgos confirmados)

- `UnitStatus` (`backend/src/systemd/mod.rs:469-483`): `name`, `active_state`, `sub_state`, `unit_file_state`, `origin`. `origin`/`SourcePath` no se exponen en la API (`#[serde(skip)]`).
- `UnitSummary` (`backend/src/api/units.rs:22-42`): `name`, `active_state`, `sub_state`, `unit_file_state`, `file_name`, `kind: Option<QuadletKind>` (derivado de la extensión). Espejo TS en `frontend/src/types/api.ts:25-47`.
- Descubrimiento: `ListUnits` + `GetUnitFileState=generated` + `SourcePath` por D-Bus (`backend/src/systemd/dbus.rs:385-429`); no hay `source_dir` en el contrato.
- Nombres de unidad: `.container` → `<stem>.service`; `.volume` → `<stem>-volume.service`; `.network` → `<stem>-network.service`; `.pod` → `<stem>-pod.service` (`backend/src/quadlet/mod.rs:99-118`).
- API: `GET /api/v1/units` (`backend/src/api/units.rs:99-111`); cliente `listarUnidades()` (`frontend/src/api/units.ts:5`); tabla `<TablaDeUnidades>` (`frontend/src/pages/unidades.tsx:269-278`, componente en `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx:67-125`). Sin agrupación hoy.
- No existe hoy ningún campo `group`/`tag`/`label`/`stack` por unidad.
- qctl detecta stacks por subdirectorio de `~/.config/containers/available`; aquí no aplica porque ese dato no está en el contrato.

## 3. Mapa arquitectónico

```
systemd (D-Bus, usuario) → ListUnits / GetUnitFileState
  → UnitStatus { name, active_state, unit_file_state, ... }
  → UnitSummary { ..., file_name, kind, stack }        [NUEVO campo]
  → GET /api/v1/units  →  listarUnidades()  →  UnitSummary (TS)
  → groupByStack()                                     [NUEVO helper puro]
  → <TablaDeUnidades> : cabeceras de stack + filas + filtros tipo/estado
```

SSE existente (`unidades-en-vivo`) sigue alimentando el mismo listado: la agrupación es una derivación pura del estado actual y se recalcula sola.

## 4. Decisiones de diseño

- **D1.** El campo `stack` se calcula en el **backend** con una función pura y se expone en `UnitSummary`. Así hay una sola fuente de verdad, el contrato queda tipado y no se duplica la regla en el frontend.
- **D2.** El frontend solo agrupa por el campo `stack` recibido; no reinterpreta nombres de unidad.
- **D3.** No se tocan el descubrimiento D-Bus, los nombres de unidad ni la persistencia: no hay migración SQLite.
- **D4.** Los filtros por tipo (`kind`) y estado (`active_state`) son secundarios y combinables; la vista sin filtros muestra todos los stacks.
- **D5.** El orden es determinista: stacks ordenados por nombre (case-insensitive) y, dentro de cada stack, unidades ordenadas por tipo principal → auxiliares → nombre. Esto evita que un evento SSE mueva filas de sitio.
- **D6.** Sin grupos explícitos del usuario (`X-Podpot-Group=`, `groups.toml`): queda fuera de alcance para una iteración futura.

## 5. Regla de derivación del stack

Función pura `stack_name(file_name: Option<&str>, kind: Option<QuadletKind>, unit_name: &str) -> String`:

1. `stem` = `file_name` sin la extensión reconocida (`.container`, `.volume`, `.network`, `.pod`, `.image`, `.kube`, `.build`).
2. Si `kind` es principal (`container`, `pod`, `kube`, `build`, `image`): `stack = stem`.
3. Si `kind` es auxiliar (`volume`, `network`): `stack = stem` con el sufijo `-volume` / `-network` eliminado **solo si está presente y queda un prefijo no vacío**; en caso contrario `stack = stem`. Así `foo-volume.volume` → `foo`, pero `backup.volume` → `backup`.
4. Si no hay `file_name`, la extensión no está reconocida o es incoherente con `kind`: fallback a `unit_name` sin `.service`. Las extensiones `kube`/`image`/`build` se tratan como principales aunque `kind` sea `None`.
5. Se preserva la grafía original; la comparación/ordenación es case-insensitive.

Casos límite a cubrir: stem que termina en `-volume` siendo un `.container` (no se recorta, porque el tipo manda), unidad auxiliar huérfana (forma su propio stack), y unidad sin extensión reconocida.

## 6. Tareas técnicas (TDD)

**Backend**
- B1 `[RED]` Tests unitarios de la función pura de derivación: principal con auxiliares, auxiliar huérfano, `.container` con sufijo `-volume`, `kind: None`, `file_name` ausente, estabilidad ante cambio de `active_state`. Ejecutar `cargo test`.
- B2 `[GREEN]` Implementar la función en un módulo de dominio (`backend/src/quadlet/stack.rs` o junto a `QuadletKind`), añadir `stack: String` a `UnitSummary` y poblarlo en el handler de `GET /api/v1/units`. Ejecutar `cargo test` y `cargo check`.
- B3 `[REFACTOR]` `cargo fmt --all`, `cargo clippy -- -D warnings`, `cargo test`.

**Frontend**
- F1 `[RED]` Tests Vitest de `groupByStack`: agrupa auxiliares con su principal, conserva auxiliares huérfanos, orden determinista, filtros por tipo/estado combinables, lista vacía. Ejecutar `npx vitest run`.
- F2 `[GREEN]` Añadir `stack: string` a `UnitSummary` en `frontend/src/types/api.ts`, crear el helper puro de agrupación y renderizar cabeceras de stack + filas en `<TablaDeUnidades>`, con los filtros. Ejecutar `npx vitest run` y `npx tsc --noEmit`.
- F3 `[REFACTOR]` `pnpm run lint`, `npx vitest run`, build.

**Spec, auditoría y cierre**
- S1 Escribir la propuesta OpenSpec y los deltas (`unidades-en-vivo`, `ui`) con escenarios Given/When/Then; **esperar aprobación del usuario antes de escribir código o tests**.
- C1 Auditoría con `@rust-reviewer` y `@react-reviewer`; devolver hallazgos al dev correspondiente.
- C2 `just check-all` en verde; crear `docs/agrupacion-por-stack.md`; marcar tareas; PR a `development` con gitmoji `✨ feat(ui): group quadlets by stack`; archivar el plan a `plans/PLAN-027-ARCHIVED.md` y el change OpenSpec.

## 7. Criterios de aceptación

- [ ] `GET /api/v1/units` devuelve `stack` para cada unidad.
- [ ] Unidades de un mismo stack aparecen juntas con cabecera identificativa.
- [ ] El estado de cada unidad sigue visible y se actualiza en vivo.
- [ ] Filtros por tipo y por estado funcionan y son combinables.
- [ ] Sin regresión en creación/edición de quadlets ni en autenticación.
- [ ] Tests backend/frontend y `just check-all` en verde.

## 8. Riesgos y mitigaciones

- **Colisiones de basename** entre quadlets de directorios distintos: ya existen en qctl; la agrupación es por nombre derivado, sin cambiar el descubrimiento.
- **SSE reordena filas**: mitigado con orden determinista (D5); `stack` no depende del estado.
- **An expanded table con filtros**: usar los mecanismos de filtro de Antd sin perder accesibilidad ni el estado de expansion.
- **Compatibilidad de contrato**: campo nuevo aditivo; ningún consumidor actual lo rompe.

## 9. Definición de done

Tests RED→GREEN→REFACTOR documentados, spec OpenSpec aprobada y luego archivada, `just check-all` en verde, reviewers sin hallazgos abiertos, docs escrita, PR mergeada en `development`, HANDOFF actualizado.
