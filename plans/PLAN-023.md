# PLAN-023 — Proteger development: PR obligatorio + checks de CI

## Referencia
- Issue: **#48** — «Proteger development: PR obligatorio + checks de CI en verde».

## Objetivo
Aplicar branch protection a la rama `development` (única rama remota, default) de forma que:
1. No se permita push directo ni force-push.
2. Se exijan dos status checks en verde y al día: **Backend (fmt, clippy, test)** y **Frontend (typecheck, lint, test, build)**.
3. La rama no sea borrable.
4. No se requieran aprobaciones (solo desarrollo), pero sí un PR para fusionar.

## Tareas
1. [ ] Crear rama `feature/branch-protection-development` desde `development`.
2. [ ] Verificar nombres exactos de los checks en `ci.yml` y que han corrido al menos una vez.
   - `Backend (fmt, clippy, test)`
   - `Frontend (typecheck, lint, test, build)`
3. [ ] Aplicar protección vía GitHub API (`gh api --method PUT /repos/:owner/:repo/branches/development/protection`) con payload:
   ```json
   {
     "required_pull_request_reviews": {"required_approving_review_count": 0},
     "required_status_checks": {
       "strict": true,
       "contexts": ["Backend (fmt, clippy, test)", "Frontend (typecheck, lint, test, build)"]
     },
     "enforce_admins": true,
     "restrictions": null,
     "required_linear_history": false,
     "allow_force_pushes": false,
     "allow_deletions": false,
     "block_creations": false
   }
   ```
   Nota: `enforce_admins: true` bloquea también al dueño (intencional: impide accidentes). Si luego `release.yml` necesita pushear a `development`, se añadirá bypass mediante rulesets.
4. [ ] Verificar lectura: `gh api repos/:owner/:repo/branches/development/protection`.
5. [ ] Probar: intentar push directo a `development` (debe fallar 403); crear PR con docs y fusionar (debe exigir checks).
6. [ ] Crear `docs/branch-protection.md` describiendo la configuración y el bypass futuro para releases.
7. [ ] Commit/PR a `development` con los docs+plan, `fmt`/`clippy`/`test` verde.
8. [ ] Renombrar plan a `PLAN-023-ARCHIVED.md` tras merge.

## Archivos a crear/modificar
- `docs/branch-protection.md` (nuevo)
- `plans/PLAN-023-ARCHIVED.md` (tras merge).

## Notas
- `main` no existe; se deja para otro issue.
- El workflow `release.yml` está en `on.push: branches: [main]` y no se dispara.
- La protección se aplica en vivo (no es código); los docs son el registro.

## Riesgos
- `enforce_admins: true` bloquea cualquier push directo del admin (incluido `gh` CLI del dueño). Intencional.
- Si en el futuro `release.yml` o automatizaciones deben empujar a `development`, se requerirá un ruleset con bypass para `github-actions` o una app. Documentado en `docs/branch-protection.md`.