# PLAN-026 — logout epoch ✅ ARCHIVADO

## Issue
- GitHub Issue: #58 (logout epoch: cerrar carrera logout↔refresh)
- PR: #59 (`✨ feat(auth): logout epoch`) → merged a `development`

## Objetivo
Implementar mecanismo de logout epoch (revocación server-side) para cerrar la carrera entre logout y renovación de sesión.

## Tareas completadas
- [x] 1.1 Crear issue #58
- [x] 1.2 Crear spec OpenSpec `logout-epoch`
- [x] 2.1 Tests integración: login→/units(200)→logout→/units(401) → recibe 401, no 200 con renewal
- [x] 2.2 Implementar epoch/revocación en `backend/src/auth`
- [x] 2.3 Correcciones reviewer: naming `bloqueo_hasta`, docstrings, 4 tests unitarios en `state.rs`
- [x] 3.1 Docs `docs/logout-epoch.md`
- [x] 4.1 Revisión @rust-reviewer aprobada
- [x] 5.1 PR #59 creado y mergeado
- [x] 5.2 Spec base actualizado con los 4 nuevos escenarios
- [x] 5.3 Plan renombrado a ARCHIVED

## Resumen técnico
Doble capa de revocación: mapa en *gate* principal + claim JWT `logout_epoch` defensa en profundidad. Sin cambios visibles en el contrato API.
- Archivos modificados: `session.rs` (+188), `state.rs` (+145), `auth.rs` (+100), `middleware.rs` (+19)
- Tests: 412 total (incluye 10 nuevos), todos verdes
- CI: verde

## Estado final
Archivado — todo completado. Cierre del follow-up de `refresh-sesion-deslizante`.
