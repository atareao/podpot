# PLAN-023 — Proteger development: PR obligatorio + checks de CI

## Referencia
- Issue: **#48** — «Proteger development: PR obligatorio + checks de CI en verde».

## Estado
- [x] Completado (aplicado en vivo y fusionado en PR #49).

## Objetivo
Aplicar branch protection a la rama `development` (única rama remota, default) de forma que:
1. No se permita push directo ni force-push.
2. Se exijan dos status checks en verde y al día: **Backend (fmt, clippy, test)** y **Frontend (typecheck, lint, test, build)**.
3. La rama no sea borrable.
4. No se requieran aprobaciones (solo desarrollo), pero sí un PR para fusionar.

## Tareas
- [x] 1. Crear rama `feature/branch-protection-development` desde `development`.
- [x] 2. Verificar nombres exactos de los checks en `ci.yml` y que han corrido al menos una vez.
- [x] 3. Aplicar protección vía GitHub API (PUT `/repos/:owner/:repo/branches/development/protection`).
- [x] 4. Verificar lectura (`gh api .../protection`).
- [x] 5. Probar push directo rechazado (`GH006`) y PR con checks.
- [x] 6. Crear `docs/branch-protection.md`.
- [x] 7. Commit/PR a `development` con docs+plan (PR #49 fusionado).
- [x] 8. Renombrar plan a `PLAN-023-ARCHIVED.md`.

## Verificación final
- Push directo a `development` bloqueado (GH006).
- Checks obligatorios: `Backend (fmt, clippy, test)` + `Frontend (typecheck, lint, test, build)` (strict).
- `enforce_admins: true` (incluye al dueño).
- PR #49 (docs+branch-protection.md+PLAN-023) fusionado a `development`.
