# PLAN-026 — logout epoch

## Issue
- GitHub Issue: #58 (logout epoch: cerrar carrera logout↔refresh)

## Objetivo
Implementar mecanismo de logout epoch (revocación server-side) para cerrar la carrera entre logout y renovación de sesión.

## Tareas
- [x] 1.1 Crear issue #58
- [x] 1.2 Crear spec OpenSpec `logout-epoch`
- [x] 2.1 Tests integración: login→/units(200)→logout→/units(401)
- [x] 2.2 Implementar epoch/revocación en `backend/src/auth` (state.rs, session.rs, middleware.rs, api/auth.rs)
- [x] 3.1 Docs `docs/logout-epoch.md`
- [x] 4.1 Revisión @rust-reviewer aprobada con correcciones (naming `bloqueo_hasta`, docstrings, tests unitarios state.rs)
- [x] 5.1 PR #59 creado → `feature/logout-epoch` base `development`
- [ ] 5.2 Merge PR a `development` pendiente (CI verde, reviewer OK)
- [ ] 5.3 Archivar change OpenSpec y renombrar plan a ARCHIVED

## Decisión clave
Doble capa de revocación: mapa en *gate* (principal) + claim JWT en `should_renew` (defensa en profundidad). Sin cambios visibles en el contrato API.

## Estado actual
- PR #59 subido a origin/feature/logout-epoch. CI green (412 tests). Reviewer aprobado. Docs creados. Esperando merge.
