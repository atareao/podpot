# ═══════════════════════════════════════════════════════════════════════════
# podpot — recetas de desarrollo
# ═══════════════════════════════════════════════════════════════════════════
#
# `just --list` es el punto de entrada. Lo que hay que saber:
#
#   * Cada LÍNEA de una receta corre en su PROPIO shell: encadena con `&&` o usa
#     una receta con shebang (`#!/usr/bin/env bash`) si necesitas varias.
#   * El layout del repo es `backend/` (crate Rust único, binario `podpot`) +
#     `frontend/` (panel React + Antd, pnpm). Por eso las recetas de Rust entran
#     en `backend/` y las del panel en `frontend/` (no hay workspace Cargo en la
#     raíz).
#   * `just check` es exactamente lo que corre el CI del backend. Si pasa en
#     local, pasa en el PR.
#   * Podman-first: en esta máquina no hay `docker` ni `/var/run/docker.sock`.
#     El contenedor habla con el systemd de USUARIO por D-Bus; ver AGENTS.md.

registry := "ghcr.io"
user := "atareao"
name := "podpot"
port := "8080"

# Base de datos local (desarrollo, fuera del contenedor). El contenedor usa
# `sqlite:///app/data/podpot.db` (ver compose.yaml).
database_url := env_var_or_default("DATABASE_URL", "sqlite://data/podpot.db")
db_file := env_var_or_default("PODPOT_DB_FILE", "data/podpot.db")

# El bus y el socket de Podman viven bajo el runtime dir del usuario.
runtime_dir := `echo "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"`

# Podman construye en formato OCI por defecto, que NO admite la instrucción
# HEALTHCHECK del Containerfile y la descarta en silencio. Forzar el formato
# `docker` conserva la sonda de salud en la imagen construida en local.
export BUILDAH_FORMAT := 'docker'

# ── Punto de entrada ────────────────────────────────────────────────────────

# Sin argumentos: lista las recetas disponibles
default:
    @just --list

# ── Calidad (paridad exacta con el CI) ─────────────────────────────────────

# Los tres checks del backend, en su orden. Ejecútalo antes de cada commit.
check: fmt clippy test

# Los tres del backend más los cinco del panel.
check-all: check ui-check

# Formato: solo comprueba.
fmt:
    cd backend && cargo fmt --all -- --check

# Formato: aplica los cambios. Si toca algo, commit aparte (🎨 style: apply formatting).
fmt-fix:
    cd backend && cargo fmt --all

# Clippy con cero warnings, con los mismos flags que el CI.
clippy:
    cd backend && cargo clippy --all-targets -- -D warnings

# Tests unitarios (#[cfg(test)]) y de integración (backend/tests/).
#
# El crate arrastra `zbus`: las pruebas que tocan el bus real (backend/tests/
# systemd_real.rs) se saltan solas si el bus de usuario no está disponible.
test:
    cd backend && cargo test

# Rustdoc: falla con cualquier warning, enlaces rotos incluidos.
doc:
    cd backend && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

# Comprueba el formato del propio .justfile.
justfile-check:
    @just --fmt --check

# Aplica formato al .justfile.
justfile-fmt:
    @just --fmt

# ── Panel (React + Antd, en frontend/) ─────────────────────────────────────

# Instala las dependencias del panel.
ui-install:
    cd frontend && pnpm install

# Servidor de desarrollo del panel, con recarga en caliente.
ui-dev:
    cd frontend && pnpm run dev

# Type-check del panel (`tsc --noEmit`; script `typecheck` de frontend/package.json).
ui-typecheck:
    cd frontend && pnpm run typecheck

# Type-check de los tests del panel (script `typecheck:test`, tsconfig.test.json).
ui-typecheck-tests:
    cd frontend && pnpm run typecheck:test

# Lint del panel.
ui-lint:
    cd frontend && pnpm run lint

# Tests del panel (Vitest, una sola pasada).
ui-test:
    cd frontend && pnpm test

# Build de producción del panel (genera frontend/dist).
ui-build:
    cd frontend && pnpm run build

# Los pasos del CI para el panel, en su orden (typecheck app → typecheck tests →
# lint → test → build).
ui-check: ui-typecheck ui-typecheck-tests ui-lint ui-test ui-build

# ── Desarrollo ─────────────────────────────────────────────────────────────

# Levanta el backend en local. El panel va aparte, en otra terminal:
# `just ui-dev` (o se sirve compilado vía PODPOT_STATIC_DIR).
#
# `mkdir -p data` es necesario: el arranque no crea directorios y SQLite quiere
# el directorio del fichero ya en pie. El binario corre desde la RAÍZ para que
# `sqlite://data/podpot.db` (relativo) apunte a `./data/`.
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p data
    RUST_LOG="${RUST_LOG:-debug}" DATABASE_URL='{{ database_url }}' \
        cargo run --manifest-path backend/Cargo.toml

# ── Imagen y contenedor (podman-first) ─────────────────────────────────────

# Construye la imagen local. Fuerza `--format docker` para conservar el
# HEALTHCHECK (ver `BUILDAH_FORMAT` arriba).
#
# Si tu uid en el host NO es 1000, reconstrúyela con el mismo uid para que
# `--userns=keep-id` encaje:
#   podman build --format docker --build-arg PODPOT_UID=$(id -u) \
#     --build-arg PODPOT_GID=$(id -g) -t localhost/podpot:dev .
build:
    podman build --format docker -t localhost/podpot:dev .

# Ejecuta la imagen en un contenedor de prueba, con los montajes del spike.
#
# `--userns=keep-id` es IMPRESCINDIBLE: sin él el bus de usuario montado existe
# pero NO responde (`Did not receive a reply`). El proceso corre con el uid del
# host, y por eso la imagen se construye con ese uid.
#
# Se montan el bus de sesión (`/run/user/<uid>`), el socket de Podman rootless
# (que queda dentro del montaje anterior) y los quadlets del usuario en su
# MISMA ruta del host (`HOME` apunta al del host).
run:
    podman run --rm -d --name {{ name }} \
        -p {{ port }}:8080 \
        --userns=keep-id \
        -v {{ runtime_dir }}:{{ runtime_dir }} \
        -v "${HOME}/.config/containers/systemd:${HOME}/.config/containers/systemd" \
        -v podpot_data:/app/data \
        -e DBUS_SESSION_BUS_ADDRESS="unix:path={{ runtime_dir }}/bus" \
        -e XDG_RUNTIME_DIR="{{ runtime_dir }}" \
        -e BIND_ADDRESS="0.0.0.0:8080" \
        -e DATABASE_URL="sqlite:///app/data/podpot.db" \
        -e PODPOT_STATIC_DIR="/app/static" \
        -e HOME="${HOME}" \
        -e RUST_LOG="info" \
        localhost/podpot:dev
    @echo "→ http://127.0.0.1:{{ port }}"

# Sigue los logs del contenedor de prueba.
logs:
    podman logs -f {{ name }}

# Para y borra el contenedor de prueba.
stop:
    -podman rm -f {{ name }}

# Comprueba que el servicio responde en el endpoint de salud.
health:
    #!/usr/bin/env bash
    set -uo pipefail
    url="http://127.0.0.1:{{ port }}/api/v1/health"
    for i in $(seq 1 30); do
        code="$(curl -s -o /dev/null -w '%{http_code}' "${url}" || true)"
        if [ "${code}" != "000" ]; then
            echo "✅ Responde ${code} tras ${i} intento(s)."
            exit 0
        fi
        sleep 2
    done
    echo "❌ Sin respuesta HTTP en ${url} tras 60s."
    exit 1

# ── Gitflow (SIEMPRE: nada directo en main ni en development) ──────────────

# Crea una rama feature desde development.
gf-feature feature_name:
    git checkout development
    git pull --ff-only
    git checkout -b feature/{{ feature_name }}
    @echo "✅ feature/{{ feature_name }} creada desde development"

# Crea una rama hotfix desde main.
gf-hotfix desc:
    git checkout main
    git pull --ff-only
    git checkout -b hotfix/{{ desc }}
    @echo "✅ hotfix/{{ desc }} creada desde main"

# Trae development a la rama actual.
gf-sync:
    git fetch origin
    git merge origin/development

# Empuja la rama actual y abre el PR (a development; a main si es hotfix/*).
gf-pr:
    #!/usr/bin/env bash
    set -euo pipefail
    branch="$(git rev-parse --abbrev-ref HEAD)"
    case "${branch}" in
        feature/*|chore/*) base="development" ;;
        hotfix/*)          base="main" ;;
        *) echo "❌ Estás en '${branch}'. gf-pr solo actúa en feature/*, chore/* o hotfix/*."; exit 1 ;;
    esac
    git push -u origin "${branch}"
    gh pr create --base "${base}" --head "${branch}" --fill

# Fusiona en su base el PR de la rama actual, borra la rama y deja la base al día.
gf-merge:
    #!/usr/bin/env bash
    set -euo pipefail
    branch="$(git rev-parse --abbrev-ref HEAD)"
    case "${branch}" in
        feature/*|chore/*) base="development" ;;
        hotfix/*)          base="main" ;;
        *) echo "❌ Estás en '${branch}'. Solo se fusionan PRs de feature/*, chore/* o hotfix/*."; exit 1 ;;
    esac
    # Fusión condicionada al CI: `gh pr checks` sale distinto de cero si algún
    # check falla o sigue pendiente.
    gh pr checks >/dev/null 2>&1 || { echo "❌ El PR no tiene todos los checks en verde:"; gh pr checks; exit 1; }
    gh pr merge --merge --delete-branch
    git checkout "${base}"
    git pull --ff-only
    # Un hotfix entra en main y hay que devolverlo a la integración.
    if [ "${base}" = "main" ]; then
        git checkout development
        git pull --ff-only
        git merge origin/main --no-edit
        git push origin development
    fi
    echo "✅ PR de ${branch} fusionado en ${base}."

# Árbol de ramas.
gf-graph:
    git log --oneline --graph --all --decorate=short -30

# ── OpenSpec (protocolo SDD) ───────────────────────────────────────────────

# En este repo NO se escribe código ni tests antes de tener un change proposal
# aprobado en openspec/changes/.
#
# Puerta del protocolo: falla si no hay ningún change proposal activo.
check-spec:
    @ls openspec/changes/*/proposal.md >/dev/null 2>&1 || (echo "❌ No hay ningún change proposal activo en openspec/changes/." && echo "   Crea uno con: just spec-new <feature>" && echo "   Y después rellena openspec/changes/<feature>/{proposal,tasks}.md y specs/<capability>/spec.md" && exit 1)
    @echo "✅ Hay change proposal activo."

# Crea el directorio de un change proposal nuevo.
spec-new change_name:
    openspec new change {{ change_name }}

# Lista los change proposals activos.
spec-list:
    @openspec list

# Valida un change proposal en modo estricto.
spec-validate change_name:
    openspec validate {{ change_name }} --strict

# Estado de los artefactos de un change (qué falta por escribir).
spec-status change_name:
    openspec status --change {{ change_name }}

# Archiva un change completado y funde su delta en openspec/specs/.
spec-archive change_name:
    openspec archive --yes {{ change_name }}

# ── Planes y documentación (convención fija del repo) ─────────────────────
# Un plan por trabajo en plans/PLAN-NNN.md; al cerrarlo se RENOMBRA (nunca se
# borra) a `-ARCHIVED`. Un documento por feature en docs/FEATURE-NNN-<slug>.md.
# Ver AGENTS.md.

# Lista los planes con su estado.
plans:
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s nullglob
    files=(plans/PLAN-*.md)
    if [ ${#files[@]} -eq 0 ]; then
        echo "No hay planes todavía. La ruta del primero la da: just plan-new"
        exit 0
    fi
    printf '%-30s %s\n' "FICHERO" "ESTADO"
    for f in "${files[@]}"; do
        base="$(basename "${f}" .md)"
        case "${base}" in
            *-ARCHIVED) state="terminado" ;;
            *)          state="en curso" ;;
        esac
        printf '%-30s %s\n' "$(basename "${f}")" "${state}"
    done

# Imprime la ruta del siguiente plan (PLAN-NNN, contador sin huecos).
plan-new:
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s nullglob
    next=1
    for f in plans/PLAN-*.md; do
        n="${f#plans/PLAN-}"
        n="${n%%-*}"
        n="${n%.md}"
        [[ "${n}" =~ ^[0-9]+$ ]] || continue
        if (( 10#${n} >= next )); then
            next=$(( 10#${n} + 1 ))
        fi
    done
    printf 'plans/PLAN-%03d.md\n' "${next}"

# Marca un plan como terminado (renombra con git mv, nunca borra).
plan-archive num:
    #!/usr/bin/env bash
    set -euo pipefail
    # 10# evita que bash lea 010 como octal.
    n="$(printf '%03d' "$(( 10#{{ num }} ))")"
    file="plans/PLAN-${n}.md"
    test -f "${file}" || { echo "❌ No existe ${file}"; exit 1; }
    # git mv solo funciona si el plan ya está trackeado; si no, mv a secas.
    if git ls-files --error-unmatch "${file}" >/dev/null 2>&1; then
        git mv "${file}" "plans/PLAN-${n}-ARCHIVED.md"
    else
        mv "${file}" "plans/PLAN-${n}-ARCHIVED.md"
    fi
    echo "✅ plans/PLAN-${n}-ARCHIVED.md"

# Imprime la ruta del documento de la siguiente feature entregada.
feature-new slug:
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s nullglob
    next=1
    for f in docs/FEATURE-*.md; do
        n="${f#docs/FEATURE-}"
        n="${n%%-*}"
        [[ "${n}" =~ ^[0-9]+$ ]] || continue
        if (( 10#${n} >= next )); then
            next=$(( 10#${n} + 1 ))
        fi
    done
    printf 'docs/FEATURE-%03d-%s.md\n' "${next}" "{{ slug }}"

# ── Base de datos (SQLite + SQLx) ─────────────────────────────────────────

# Crea la base de datos local y aplica las migraciones.
#
# Las migraciones viven en `backend/migrations/` con prefijo numérico entero
# (`0001_init.sql`), no en la forma Flyway: de ahí el `--source` explícito.
db-setup:
    mkdir -p data
    sqlx database create --database-url {{ database_url }}
    sqlx migrate run --database-url {{ database_url }} --source backend/migrations

# Añade una migración vacía al directorio de migraciones del backend.
migrate-add migration_name:
    sqlx migrate add --source backend/migrations {{ migration_name }}

# Abre un shell sqlite3 sobre la base de datos local.
db-shell:
    sqlite3 {{ db_file }}

# Borra la base de datos local (irrecuperable). Recréala con: just db-setup.
db-reset:
    rm -f {{ db_file }} {{ db_file }}-wal {{ db_file }}-shm
    @echo "✅ Base de datos local borrada. Recréala con: just db-setup"

# ── Limpieza ───────────────────────────────────────────────────────────────

# Limpia los artefactos de compilación de Rust.
clean:
    cd backend && cargo clean

# Limpia además los del panel y la imagen local de podpot.
distclean: clean
    rm -rf frontend/dist frontend/node_modules
    -podman image rm localhost/podpot:dev
