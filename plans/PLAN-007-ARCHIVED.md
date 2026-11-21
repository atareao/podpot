# PLAN-007 — logs de los contenedores de las quadlets en el panel (SSE)

- **Issue:** #14 — feat: logs de los contenedores de las quadlets en el panel (SSE)
- **Change OpenSpec:** `openspec/changes/registros-de-quadlets`
- **Capability nueva:** `registros` (deltas en `config`, `api-shell`, `ui`)
- **Estado:** en curso

## Objetivo

Ver en el panel los logs del contenedor de una quadlet, en vivo (SSE), sin salir del panel. Fuente: la API de Podman por su socket (sin shell-out).

## Arquitectura (dónde toca)

- `backend/src/registros/` (nuevo): cliente Podman por socket unix (`hyper` + `hyperlocal`), mapeo unidad→contenedor por label `PODMAN_SYSTEMD_UNIT=<unidad>.service`, demux del stream de logs (función pura) y puerto `FuenteDeRegistros` + doble.
- `backend/src/api/logs.rs` (nuevo): `GET /api/v1/units/{name}/logs/stream` (`text/event-stream`), evento `log` por línea (envoltura `{data:{line,stream}}`), cola inicial + `follow`, `retry`/latido/terminal, cierre por `watch`.
- `backend/src/config.rs`: `PODPOT_PODMAN_SOCKET` (default `$XDG_RUNTIME_DIR/podman/podman.sock`).
- `backend/src/state.rs`: `AppState` gana la fuente de registros.
- `backend/src/api/mod.rs`: ruta en la mitad **protegida** (gate).
- `backend/Cargo.toml`: `hyper` (client,http1) + `hyperlocal` + `http-body-util`. **Sin** bollard (146 paquetes vs 51).
- `frontend/src/`: hook/cliente del canal de logs + drawer del visor abierto desde una acción de fila en `TablaDeUnidades`.

## Decisiones clave

- Fuente = **API de Podman por socket** (el socket ya está montado bajo `/run/user/<uid>`); **no** shell-out ni journald.
- Mapeo por **label** `PODMAN_SYSTEMD_UNIT` (verificado en la máquina), no por nombre de fichero.
- Cliente ligero `hyper`+`hyperlocal`; demux del marco Docker como función pura (probada aparte).
- Puerto `FuenteDeRegistros` con doble (patrón `SystemdClient`/`FakeSystemd`).
- SSE reutiliza el patrón del canal `units`; el `EventSource` del visor se abre/cierra con el drawer (no canal permanente).
- Unidad sin contenedor → error envuelto explícito (no flujo vacío).

## Tareas (TDD por bloques)

- [ ] 1. Config (`PODPOT_PODMAN_SOCKET`) y dependencias.
- [ ] 2. Cliente Podman (demux puro; listar por label; seguir logs; sin contenedor → error).
- [ ] 3. Puerto `FuenteDeRegistros` + doble.
- [ ] 4. API SSE (cola inicial, eventos `log`, `retry`/latido/terminal/cierre, errores, gate).
- [ ] 5. Frontend (hook del canal + drawer del visor).
- [ ] 6. Cierre (`just check-all`, reviewers, `openspec validate`, `docs/registros-de-quadlets.md`).

## Criterios de aceptación

- Abrir los logs de una quadlet en el panel muestra la cola inicial y las líneas nuevas en vivo.
- La fuente es la API de Podman por socket (sin binarios); unidad sin contenedor da error explícito.
- La ruta está protegida por el gate como el resto de negocio.
- `just check-all` EXIT 0; `openspec validate --all` en verde; reviewers sin bloqueantes.

## Fuera de alcance

- Progreso de acciones por job (ya cubierto por el panel en vivo); descarga a fichero; búsqueda/filtrado en el visor; logs de unidades que no son contenedores.
