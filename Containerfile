# syntax=docker/dockerfile:1
# ═══════════════════════════════════════════════════════════════════════════
# podpot — imagen de producción multi-stage
#
#   etapa 1 (ui-builder):    compila el panel de `frontend/` (pnpm)  → frontend/dist
#   etapa 2 (rust-builder):  compila el binario de release           → backend/target/release/podpot
#   etapa 3 (runtime):       solo binario + panel, usuario no-root, sin toolchain
#
# Reglas del spike (AGENTS.md §Spike validado): la imagen corre como usuario
# no-root cuyo uid coincide con el del host (por defecto 1000) y NO instala
# nada en runtime; TODAS las dependencias se hornean aquí.
#
# El HEALTHCHECK solo sobrevive si se construye en formato Docker:
#   podman build --format docker -t localhost/podpot:dev .
# (el formato OCI de buildah lo descarta en silencio, igual que en kesto).
# ═══════════════════════════════════════════════════════════════════════════

# ── Etapa 1: panel (React + Antd) ──────────────────────────────────────────
FROM node:24-bookworm-slim AS ui-builder

WORKDIR /app/frontend

# pnpm 12 por corepack. Los manifiestos primero, para cachear la instalación.
COPY frontend/package.json frontend/pnpm-lock.yaml frontend/pnpm-workspace.yaml ./
RUN corepack enable && corepack prepare pnpm@12 --activate

# Instalación reproducible: el lockfile manda.
RUN pnpm install --frozen-lockfile

# El resto del panel, y el build de producción (`tsc -b && vite build`).
COPY frontend/ ./
RUN pnpm run build

# ── Etapa 2: binario de release de Rust ────────────────────────────────────
FROM rust:1.97-bookworm AS rust-builder

WORKDIR /app

# Caché de dependencias: se compilan las dependencias contra un `main.rs`/`lib.rs`
# vacíos, de modo que esta capa se reutiliza mientras Cargo.toml/Cargo.lock no
# cambien. El crate tiene lib + bin, así que se estuban los dos.
COPY backend/Cargo.toml backend/Cargo.lock ./backend/
RUN mkdir -p backend/src \
    && echo '' > backend/src/lib.rs \
    && echo 'fn main() {}' > backend/src/main.rs \
    && cargo build --release --locked --manifest-path backend/Cargo.toml \
    && rm -rf backend/src

# `migrations/` es imprescindible: `sqlx::migrate!("./migrations")`
# (backend/src/db.rs) lee el directorio en tiempo de compilación y lo incrusta
# en el binario.
COPY backend/src/ ./backend/src/
COPY backend/migrations/ ./backend/migrations/
# `touch` de TODO el árbol: los ficheros copiados heredan el mtime del host, que
# puede ser anterior al artefacto del stub; sin refrescarlo, cargo daría la capa
# de caché por buena y no recompilaría la librería de verdad.
RUN find backend/src -type f -exec touch {} + \
    && cargo build --release --locked --manifest-path backend/Cargo.toml

# ── Etapa 3: runtime mínimo ────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Solo lo que el HEALTHCHECK necesita (curl + CA raíz). Se instala en BUILD,
# NUNCA en runtime: es la regla «imagen autocontenida» del spec.
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# El panel lo sirve el propio backend (una sola imagen autocontenida).
COPY --from=ui-builder /app/frontend/dist /app/static
COPY --from=rust-builder /app/backend/target/release/podpot /app/podpot

# uid/gid que coincide con el del host (spike: 1000). Con `--userns=keep-id` el
# proceso corre con el uid del host; si el tuyo no es 1000, reconstrúyela con
# `--build-arg PODPOT_UID=$(id -u) --build-arg PODPOT_GID=$(id -g)`.
ARG PODPOT_UID=1000
ARG PODPOT_GID=1000

# Usuario no-root y directorio de datos (SQLite) en su propiedad.
RUN groupadd --gid "${PODPOT_GID}" podpot \
    && useradd --uid "${PODPOT_UID}" --gid "${PODPOT_GID}" --no-create-home --home-dir /app podpot \
    && mkdir -p /app/data \
    && chown -R "${PODPOT_UID}:${PODPOT_GID}" /app

USER ${PODPOT_UID}:${PODPOT_GID}

# El panel se sirve desde aquí dentro del backend (aún no cableado en esta
# entrega; ver limitaciones). El bind por defecto es loopback, así que el
# contenedor lo sobreescribe a 0.0.0.0 (ver compose.yaml).
ENV PODPOT_STATIC_DIR=/app/static

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s \
    CMD curl -fsS http://127.0.0.1:8080/api/v1/health || exit 1

CMD ["/app/podpot"]
