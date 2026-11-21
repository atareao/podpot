# podpot — Instrucciones para agentes

> Las directivas globales del entorno (OpenSpec SDD + TDD, tags de estado, gitflow + gitmoji, planes/documentación, subagentes, ahorro de contexto) se inyectan en cada sesión desde `~/.config/opencode/AGENTS.md`. Aquí solo lo **específico de este repositorio**; no se repiten.

**Qué es podpot:** panel web para gestionar **quadlets de Podman** de la instancia **rootless** de systemd (`systemctl --user`). Dos funciones: (1) mostrar el estado de cada contenedor y controlarlo (start/stop/restart/…); (2) **crear** quadlets nuevas.

**Estado: greenfield con arquitectura validada.** No hay código aún. El *spike* de despliegue (abajo) ya está **en verde**: D-Bus al systemd de usuario funciona desde el contenedor con el montaje correcto.

## Arquitectura decidida

| Tema | Decisión |
|---|---|
| Backend | Rust + **Axum** |
| Frontend | React + TypeScript + **Ant Design** (Vite, pnpm, Vitest) |
| Interfaz con systemd | **D-Bus directo (`zbus`)** contra `org.freedesktop.systemd1` — **no** shell-out a `systemctl` |
| Alcance | **Solo rootless** (`--user`); nunca unidades de sistema/root |
| Persistencia | **SQLite** vía `sqlx`, migraciones embebidas, WAL |
| Despliegue | **Contenedor** rootless (patrón kesto) — **validado** (ver Spike) |
| Autenticación | Bind a **loopback** en dev; **OIDC opcional** (patrón kesto) para exposición |

### Por qué D-Bus y no shell-out

- Una quadlet **es** una unidad systemd: el estado autoritativo vive en **systemd**, no en Podman. **La API de Podman no puede accionar una quadlet**; solo systemd puede.
- qctl usa shell-out a `systemctl --user`, pero aquí se elige D-Bus por: estado **tipado** (sin parsear texto), **push** con `PropertiesChanged` para el panel en vivo (SSE), y no cargar el binario `systemctl` en la imagen.

## Spike validado (2026-11-21) — receta que funciona

Probado en esta máquina (podman 6.1.4, systemd **262.1-1-arch**, uid 1000, `Linger=yes`) sobre la unidad desechable `podpot-spike.service` y sobre quadlets reales (`centinela-db.service`): `GetVersion`, `Get(ActiveState)`, `StartUnit`/`StopUnit` y `Get(UnitFileState)` responden desde el contenedor.

**Receta obligatoria** (ninguna pieza es opcional):

```sh
podman run --rm --userns=keep-id \
  -v /run/user/$UID:/run/user/$UID \
  -e DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$UID/bus \
  -e XDG_RUNTIME_DIR=/run/user/$UID \
  podpot ...
```

- **`--userns=keep-id` es imprescindible.** Sin él, el bus montado existe pero **no responde** (`Did not receive a reply`): el uid del contenedor debe coincidir con el del host (1000). Es el fallo más fácil de no diagnosticar.
- ❌ **No uses `--userns=keep-id` + `apk`/`apt` en runtime**: con keep-id el proceso corre como uid 1000 (no root) y la instalación de paquetes falla (`Unable to open log: Permission denied`). **Bakea todas las dependencias en la imagen y corre como el usuario no-root del host.**
- El bus de usuario (`/run/user/<uid>/bus`) es `srw-rw-rw-`; se puede montar el socket o el directorio `/run/user/<uid>` completo.
- En el host hace falta `loginctl enable-linger $USER` (ya está activo aquí) para que el usuario/systemd existan sin sesión.

## Trampas que un agente rompería por defecto

- **Nombres de unidad escapados en D-Bus** (`foo.service` → `foo_2eservice`; `-` → `_2d`; `.` → `_2e`). No pases el basename crudo a las rutas `/org/freedesktop/systemd1/unit/*`.
- **`StartUnit`/`StopUnit`/`RestartUnit` son asíncronos**: devuelven un *job* (`/org/freedesktop/systemd1/job/<id>`). Confirma por `PropertiesChanged`/consulta del job; no asumas éxito inmediato.
- **El nombre de unidad sale del basename**, no de `ContainerName=`: `<stem>.service` para `.container`, `<stem>-volume.service` para `.volume`, `<stem>-network.service` para `.network`. `ServiceName=` cambia el nombre de la **unidad**; `ContainerName=` solo el del contenedor Podman.
- Las quadlets aparecen como unidades **`UnitFileState=generated`** (no `enabled`/`disabled`): no las trates como unidades normales al filtrar.
- **Migraciones con prefijo numérico entero** (`0001_init.sql`), **no** forma Flyway `V001__x.sql`: `sqlx::migrate!()` parte por el primer `_` y hace `parts[0].parse::<i64>()`.
- **`foreign_keys` en las opciones de conexión del pool**, nunca en un `PRAGMA` suelto (el `ON DELETE CASCADE` no ocurre sin que nada lo delate).
- El descubrimiento de quadlets **solo mira el primer nivel** de los directorios fuente (divergencia consciente con el runtime real, que es recursivo). Heredado de qctl.

## Layout (decidido)

```text
backend/     Rust + Axum (D-Bus, SQLite, API REST + SSE)
frontend/    React + TS + Antd (Vite, pnpm, Vitest)
migrations/  esquema SQLite (sqlx embebido)
docs/        un documento por feature
plans/       planes de implementación
```

`backend/` + `frontend/`, **no** `src/` + `ui/` (que es la convención de kesto: **no la mezcles aquí**).

## Proceso (paridad con kesto, sin repetir el detalle)

Todo pasa por un **`.justfile`** con `check` (fmt → clippy → test), `check-all` (incluye `ui-*`) y el ciclo gitflow/OpenSpec/planes. Frontend con **pnpm**. Los detalles del ciclo están en las directivas globales y en `/data/rust/kesto/AGENTS.md`; no los dupliques aquí.

## Referencias del entorno (leer antes de diseñar)

| Ruta | Qué reutilizar |
|---|---|
| `/data/rust/qctl` | Lógica de quadlets: descubrimiento (`./quadlets/` + `.`, primer nivel), enlaces en `$HOME/.config/containers/systemd`, stacks root `~/.config/containers/available`, nombres de unidad, colisiones de basename, pruebas con shims |
| `/data/rust/kesto` | Patrón de app completa: Axum + `sqlx`/SQLite + OIDC + SSE, `compose.yaml`, `.justfile`, gitflow, `Dockerfile`, layout `src/`+`ui/` (referencia, no se copia) |
