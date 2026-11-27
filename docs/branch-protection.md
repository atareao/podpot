# Protección de ramas — `development` y `main`

## Configuración aplicada (2026-11-23)

Ambas ramas de larga vida están protegidas mediante **rulesets** (no branch
protection clásica, que no admite bypass).

| Rama | Ruleset | ID |
|---|---|---|
| `development` (default) | `development-protection` | 27879478 |
| `main` | `main-protection` | 27879853 |

### Reglas (idénticas en ambas)

| Regla | Valor |
|---|---|
| **Pull request obligatorio** | Sí (sin push directo) |
| **Status checks requeridos (strict)** | `Backend (fmt, clippy, test)` + `Frontend (typecheck, lint, test, build)` |
| **Aprobaciones requeridas** | 0 (desarrollo en solitario) |
| **Force-push** | Prohibido (`non_fast_forward`) |
| **Borrar rama** | Prohibido (`deletion`) |
| **Métodos de merge permitidos** | merge, squash, rebase |
| **Bypass** | Rol **admin** del repositorio (`RepositoryRole` id 5), `bypass_mode: always` |

### Checks de CI vinculados
- `Backend (fmt, clippy, test)` — `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test` (backend/)
- `Frontend (typecheck, lint, test, build)` — `pnpm run typecheck` + `pnpm run lint` + `pnpm run test` + `pnpm run build` (frontend/)

Ambos definidos en `.github/workflows/ci.yml` (GitHub Actions, `app_id: 15368`).

## Bypass para `release.yml` (PAT de admin)

`release.yml` empuja el commit de versión a `main` y resincroniza `development`
tras cada release. Ese push lo hace el job `prepare`, cuyo `actions/checkout`
usa ahora `token: ${{ secrets.RELEASE_TOKEN }}`.

### Por qué un PAT y no el `GITHUB_TOKEN`

En **repositorios personales** GitHub **no permite** que la integración de
GitHub Actions (ni `GITHUB_TOKEN`) sea bypass actor de un ruleset: la API
responde `422 Actor GitHub Actions integration must be part of the ruleset
source or owner organization`. Una **deploy key** como bypass **tampoco surte
efecto** en rulesets de rama (verificado empíricamente: push rechazado). La
única vía soportada es un actor con **rol admin** (o un App de organización).

Por eso:
1. Los rulesets tienen como bypass `RepositoryRole` id **5 (admin)**.
2. El workflow usa `secrets.RELEASE_TOKEN`: un PAT de un admin del repo
   (permisos `Contents: read/write` y `Workflows: read/write`, limitado al repo
   `podpot`). Al autenticarse como admin, salta el ruleset.

El token vive además en `crypta` como `github_podpot_release_token`.

### Rotación del token

```sh
# 1. Genera un PAT fine-grained en https://github.com/settings/personal-access-tokens/new
#    (Contents: read/write, Workflows: read/write, solo el repo podpot)
# 2. Guárdalo en crypta y actualiza el secreto del repo
crypta set github_podpot_release_token --value 'github_pat_...'
gh secret set RELEASE_TOKEN --repo atareao/podpot \
  --body "$(crypta lookup github_podpot_release_token)"
```

## Verificación

```sh
gh api repos/:owner/:repo/rulesets --jq '.[] | {id,name}'
gh api repos/:owner/:repo/rulesets/27879478 --jq '{name,bypass_actors,rules:[.rules[].type]}'
gh api repos/:owner/:repo/rulesets/27879853 --jq '{name,bypass_actors,rules:[.rules[].type]}'
gh api repos/:owner/:repo/rules/branches/main --jq '[.[].type]'
```

## Cambiar las reglas

Los rulesets se editan con PUT (hay que enviar el objeto completo):

```sh
gh api repos/:owner/:repo/rulesets/27879853 > /tmp/rs.json
# editar /tmp/rs.json (p. ej. subir required_approving_review_count a 1)
gh api --method PUT repos/:owner/:repo/rulesets/27879853 --input /tmp/rs.json
```

## Notas

- `development` sigue siendo la rama **default** del repositorio.
- La branch protection clásica que había en `development` (PLAN-023) se retiró:
  la sustituye el ruleset, equivalente y además con soporte de bypass.
- El push de `release.yml` **no es atómico**: si el push de `main` falla pero el
  del tag tiene éxito, queda un tag huérfano que rompe la siguiente release
  (`fatal: tag already exists`). Si ocurre, bórralo: `git push origin :refs/tags/vX.Y.Z`.
