# PLAN-026 — logout epoch

## Issue
- GitHub Issue: #58 (logout epoch: cerrar carrera logout↔refresh)

## Objetivo
Implementar mecanismo de logout epoch (revocación server-side) para cerrar la carrera entre logout y renovación de sesión.

## Tareas
- [x] 1.1 Crear issue #58
- [ ] 1.2 Crear spec OpenSpec `logout-epoch`
- [ ] 2.1 Añadir tests de integración: logout + refresh en vuelo no resucite sesión
- [ ] 2.2 Implementar epoch/revocación en `backend/src/auth`
- [ ] 3.1 Actualizar `docs/logout-epoch.md`
- [ ] 4.1 Revisar con @rust-reviewer y @react-reviewer
- [ ] 5.1 Mergear PR a `development`
