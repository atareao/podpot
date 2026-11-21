# Git Flow — podpot

Este proyecto sigue **Git Flow** con versionado semántico. Igual que el resto de
proyectos del entorno, se usa **siempre**: nada se commitea directamente en
`main` ni en `development`. Repo: `github.com/atareao/podpot`.

**Estado actual:** remoto en `github.com/atareao/podpot`, ramas `main` y
`development`, y pipeline de GitHub Actions (`ci.yml` + `release.yml`).

## Ramas

| Rama | Propósito | Base |
|---|---|---|
| `main` | Producción. Cada merge aquí **dispara una release**. | — |
| `development` | Integración de features en curso. Rama activa de trabajo. | `main` |
| `feature/<kebab-case>` | Nuevas funcionalidades. | `development` |
| `hotfix/<kebab-case>` | Correcciones urgentes a producción. | `main` |

## Flujo diario

### Features

```bash
git checkout development && git pull --ff-only
git checkout -b feature/mi-feature
# ... trabajo ...
git commit -m "✨ feat(units): exponer el estado real de cada quadlet"
git push -u origin feature/mi-feature
gh pr create --base development --head feature/mi-feature
```

### Hotfixes

```bash
git checkout main && git pull --ff-only
git checkout -b hotfix/arreglo-critico
git commit -m "🚑️ hotfix: crash al listar quadlets sin bus de usuario"
git push -u origin hotfix/arreglo-critico
gh pr create --base main --head hotfix/arreglo-critico
# tras el merge, la sincronización de development la hace el workflow de release
```

### Recetas `gf-*` del `.justfile`

| Receta | Qué hace |
|---|---|
| `just gf-feature <nombre>` | Crea `feature/<nombre>` desde `development` al día. |
| `just gf-hotfix <desc>` | Crea `hotfix/<desc>` desde `main` al día. |
| `just gf-sync` | Trae `origin/development` a la rama actual. |
| `just gf-pr` | Empuja la rama actual y abre el PR (a `development`; a `main` si es `hotfix/*`). |
| `just gf-merge` | **Exige `gh pr checks` en verde** (por eso existe `ci.yml`), fusiona con `--merge --delete-branch` y deja la base al día. |
| `just gf-graph` | Árbol de ramas. |

## Releases

Están automatizadas: **un merge a `main` es una release**. No se etiqueta a mano.

1. PR `development` → `main`.
2. Al mergear, `.github/workflows/release.yml` (trigger: push a `main`) ejecuta
   tres jobs encadenados:
   - **`prepare`**: deduce el bump desde los conventional commits desde el
     último tag (`feat` → minor, `!:`/`BREAKING CHANGE` → major, resto →
     patch), sube la versión con **vampus** (reescribe `backend/Cargo.toml` y
     `.vampus.yml`, y refresca la entrada de `podpot` en `Cargo.lock`),
     regenera `CHANGELOG.md` con **git-cliff**, commitea `chore: release vX.Y.Z`,
     crea el tag anotado `vX.Y.Z`, empuja `main --follow-tags` y **sincroniza
     `development` con `main`**.
   - **`image`**: construye desde la raíz con el `Containerfile` y publica la
     imagen en **`ghcr.io/atareao/podpot`**, con los tags `X.Y.Z` y `latest`.
   - **`release`**: abre la GitHub Release con las notas generadas.
3. La guarda `if: "!startsWith(github.event.head_commit.message, 'chore: release')"`
   evita el bucle: el commit de release que empuja `prepare` no re-dispara la
   release.

### El entregable es la imagen, no crates.io

podpot se despliega como **contenedor**; su artefacto es la imagen en
`ghcr.io/atareao/podpot`. **No se publica en crates.io** (`backend/Cargo.toml`
lleva `publish = false`: es una aplicación, no una librería). El build de la
imagen va en el **mismo** workflow que el tag, y no en un workflow aparte
disparado por tag, porque **un tag empujado con el `GITHUB_TOKEN` por defecto no
dispara otro workflow** (limitación de GitHub).

### Requisitos

- **Sin secretos extra**: basta el `GITHUB_TOKEN` por defecto (`permissions:
  contents: write, packages: write`). Para que GHCR acepte el push, el paquete
  `ghcr.io/atareao/podpot` debe permitir escritura desde el workflow (lo hace por
  defecto al publicarlo desde el propio repo).

## Conventional Commits con gitmoji

El formato del mensaje determina el bump automático:

| Mensaje | Bump |
|---|---|
| `✨ feat: ...` | minor (0.Y.0) |
| `🐛 fix: ...` / `🚑️ hotfix: ...` | patch (0.0.Z) |
| `💥 feat!: ...` o `BREAKING CHANGE` | major (X.0.0) |
| cualquier otro tipo (`refactor`, `docs`, `chore`, `test`…) | patch (0.0.Z) |

| Tipo | Emoji | Tipo | Emoji |
|---|---|---|---|
| `feat` | ✨ | `style` | 💄 |
| `fix` | 🐛 | `test` | ✅ |
| `hotfix` | 🚑️ | `chore` | 🔧 |
| `docs` | 📝 | `ci` | 👷 |
| `refactor` | ♻️ | `revert` | ⏪️ |
| `perf` | ⚡ | `deps` | ⬆️ |
| `breaking` | 💥 | `release` | 🔖 |

## Formateo previo obligatorio

Antes de crear ramas, commitear o abrir PRs, formatea para que el CI no falle.
El layout de podpot es `backend/` (Rust) + `frontend/` (React):

```bash
just check       # backend: fmt --check → clippy -D warnings → test
just check-all   # + frontend: typecheck → typecheck:test → lint → test → build
```

Para aplicar formato en vez de comprobarlo: `just fmt-fix` y `just
justfile-fmt`. Si el formateo produce cambios, commitéalos aparte
(`🎨 style: apply formatting`).

## CI

| Workflow | Cuándo | Qué |
|---|---|---|
| `ci.yml` | PR a `main`/`development` y push a `development` | `backend` (fmt, clippy, test) y `frontend` (typecheck, lint, test, build) **en paralelo** |
| `release.yml` | push a `main` | `prepare` (bump + changelog + tag + sync) → `image` (ghcr.io) → GitHub Release |

Ambos workflows replican exactamente las recetas del `.justfile`: si `just
check-all` pasa en local, el PR pasa.

## Planes y documentación

- Un plan por trabajo en `plans/PLAN-NNN.md`; al cerrarlo se **renombra** (nunca
  se borra) a `plans/PLAN-NNN-ARCHIVED.md`. Recetas: `just plans`, `just
  plan-new`, `just plan-archive N`.
- Un documento por feature entregada en `docs/FEATURE-NNN-<slug>.md`
  (`just feature-new <slug>`).
- Convención completa en [AGENTS.md](./AGENTS.md).

## Resumen visual

```text
main        ──●───────────────────────●────────────
               \                      /
development ────●──●──●──●──●──●──────●
                 \    /   \  /
feature           ●──●     ●─●
```
