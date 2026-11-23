# Protección de ramas — development

## Configuración aplicada (2026-11-23)

Rama protegida: **development** (única rama remota, default).

### Reglas
| Regla | Valor |
|---|---|
| **Pull request obligatorio** | Sí (sin push directo) |
| **Status checks requeridos (strict)** | `Backend (fmt, clippy, test)` + `Frontend (typecheck, lint, test, build)` |
| **Aprobaciones requeridas** | 0 (desarrollo en solitario) |
| **Force-push** | Prohibido |
| **Borrar rama** | Prohibido |
| **Bloquear a admins** | Sí (`enforce_admins: true`) |
| **Historial lineal** | No requerido |
| **Bloquear creación de ramas** | No |

### Checks de CI vinculados
- `Backend (fmt, clippy, test)` — `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test` (backend/)
- `Frontend (typecheck, lint, test, build)` — `pnpm run typecheck` + `pnpm run lint` + `pnpm run test` + `pnpm run build` (frontend/)

Ambos definidos en `.github/workflows/ci.yml` y reconocidos como checks de GitHub Actions (`app_id: 15368`).

### Comportamiento
- Un push directo a `development` falla con `GH006: Protected branch update failed`.
- Un PR a `development` exige que **ambos checks** estén en verde (y al día, `strict: true`) antes de permitir merge.
- El dueño del repo también queda bloqueado (`enforce_admins: true`): evita accidentes.

### Futuro: bypass para automatizaciones (releases)
Cuando se cree `main` y `release.yml` necesite resincronizar `development` tras un release:
1. Crear un **ruleset** (no branch protection clásico) que permita a `github-actions` hacer push a `development`.
2. O añadir una **GitHub App** con permisos y meterla en `restrictions`/`bypass_actors` de la protección actual.
3. Documentar aquí el bypass elegido.

### Verificación
```sh
gh api repos/:owner/:repo/branches/development/protection
# Debe devolver los contextos exactos arriba y enforce_admins: true
```

### Cambiar la protección
Modificar vía API (ejemplo para ajustar aprobaciones a 1):
```sh
gh api --method PUT repos/:owner/:repo/branches/development/protection \
  -f 'required_pull_request_reviews[required_approving_review_count]=1'
```
