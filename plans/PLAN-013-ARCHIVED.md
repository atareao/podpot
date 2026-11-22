# PLAN-013 — Identidad `file_name` en TablaDeUnidades

- **Issue**: #27 (https://github.com/atareao/podpot/issues/27)
- **Change OpenSpec**: `openspec/changes/identidad-file-name-tabla/`
- **Tipo**: refactor (♻️) — alineación de la UI con la spec `ui`
- **Branches**: `development` → `feature/identidad-file-name-tabla` → PR a `development`

## Objetivo

Eliminar la asimetría `name`/`file_name`: la tabla debe identificar cada quadlet por su `file_name` (identidad real publicada por la API), con el nombre de unidad systemd como dato secundario.

## Contexto

`GET /api/v1/units` publica `file_name` y `kind` desde el issue #6. La spec `ui` ya exige `file_name` como identidad y las acciones de editar/borrar ya lo usan. Pero `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx` sigue usando `name` como identidad visible (columna «Unidad», `aria-label` de acciones y progreso, índices de `enCurso`/`progresoPorUnidad`).

## Tareas (TDD)

1. Baseline: `pnpm test` en `frontend/` para confirmar verde de partida.
2. RED: añadir/ajustar tests en `TablaDeUnidades.test.tsx` que exijan identidad por `file_name` (fila + nombres accesibles + índices). Ejecutar y confirmar rojo.
3. GREEN: implementar la identidad `file_name` en `TablaDeUnidades.tsx` (y `pages/unidades.tsx` si procede). Ejecutar tests hasta verde.
4. REFACTOR: `npx tsc --noEmit`, lint y `just check-all` en verde.
5. Auditar con `@react-reviewer`.
6. Documentar en `docs/identidad-file-name-tabla.md`; PR a `development` referenciando el issue #27; archivar el change y renombrar este plan a `plans/PLAN-013-ARCHIVED.md`.

## Criterios de aceptación

- La fila se identifica por `file_name`; `name` aparece como dato secundario.
- Nombres accesibles (acciones y progreso) coherentes con la identidad.
- Tests frontend verdes; `tsc`/lint/`check-all` verdes.
- `openspec validate --all` verde.
