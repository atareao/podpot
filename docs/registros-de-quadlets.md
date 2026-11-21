# registros-de-quadlets — logs de los contenedores de las quadlets en el panel

## Qué es

Ver en el panel los **logs del contenedor de una quadlet, en vivo, sin salir del panel**. La feature `unidades-en-vivo` lo había declarado explícitamente **fuera de alcance** (SSE de logs); esta feature lo cubre.

Issue: #14 · Change OpenSpec: `openspec/changes/registros-de-quadlets` · Plan: `plans/PLAN-007.md`.

## Ficheros implicados

Backend (`backend/src/`):
- `registros/` (nuevo):
  - `mod.rs` — puerto `FuenteDeRegistros`, `RegistrosError` y constantes `COLA_INICIAL=200` y `CAPACIDAD_DE_REGISTROS`.
  - `demux.rs` — **demultiplexador incremental** del marco Docker (`Demux`, `Origen` stdout/stderr, `LineaDeRegistro`); tope `TOPE_DE_MARCO`=1 MiB y error `DemuxError::MarcoDesmesurado`.
  - `podman.rs` — `PodmanRegistros` sobre `hyper`+`hyperlocal` por socket unix; mapeo unidad→contenedor por la label `PODMAN_SYSTEMD_UNIT=<unidad>.service`; inspección de `Config.Tty`; `follow=true&tail=<cola>`; timeouts `PLAZO_DE_PETICION`=5 s; cancelación por `emisor.closed()`.
  - `fake.rs` — `FakeRegistros` (doble para probar sin socket).
- `api/logs.rs` (nuevo) — `GET /api/v1/units/{name}/logs/stream` (`text/event-stream`).
- `config.rs` — variable `PODPOT_PODMAN_SOCKET`.
- `state.rs` — `AppState` gana la fuente de registros.
- `main.rs`, `api/mod.rs` — la ruta se registra en la mitad **protegida**.
- `error.rs` — `AppError::Registros`; códigos `unit_without_container`=404, `podman_unavailable`=503, `podman_response`=502.
- `tests/registros_real.rs` — tests `#[ignore]` contra el socket real.
- `tests/arranque.rs` — arranque con la fuente cableada.
- `Cargo.toml` — `hyper` (client,http1), `hyperlocal` (client), `hyper-util` (client-legacy,http1,tokio) y `http-body-util`.

Frontend (`frontend/src/`):
- `hooks/useRegistrosDeUnidad.ts` — `EventSource` al canal; escucha el evento con nombre `log`; desenvuelve `{data:{line,stream}}`; buffer por lote cada 150 ms con `startTransition`; tope `MAXIMO_DE_LINEAS`=2000; limpia el aviso en `open`; `close()` en el cleanup.
- `components/VisorDeRegistros/` — Drawer del visor.
- `types/api.ts` — `OrigenDeRegistro` / `LineaDeRegistro`.
- `components/TablaDeUnidades/TablaDeUnidades.tsx` — acción «Logs» por fila.
- `pages/unidades.tsx` — monta el visor.
- `estilos.css` — estilos del visor.

## Cómo funciona

1. El panel abre un **Drawer de logs** desde la acción «Logs» de una fila; el hook abre el `EventSource` **solo mientras el visor está abierto** y lo cierra al cerrarlo.
2. El backend localiza el contenedor de la unidad por la **label de systemd**, inspecciona si usa TTY y hace `GET .../logs?follow=true&stdout=true&stderr=true&tail=200` por el socket de Podman.
3. Envía primero la **cola inicial** y luego un evento `log` por línea con su origen; además `retry`, latido y un evento terminal `log-end`; el flujo se cierra con el apagado del servidor.
4. Si el contenedor **no** usa TTY el stream viene **multiplexado** (marco Docker: 1 byte de origen + 3 de relleno + 4 de longitud BE + payload) y se demultiplexa; si usa TTY es **crudo**.

**Sin shell-out** (no `journalctl` ni `podman logs`): la fuente es la API de Podman por su socket.

## Variables / configuración

- `PODPOT_PODMAN_SOCKET` — ruta del socket de la API de Podman. Defecto: `$XDG_RUNTIME_DIR/podman/podman.sock`; si no hay `XDG_RUNTIME_DIR`, deriva `/run/user/<uid>` del propietario de `$HOME`.
- Cola inicial: 200 líneas (constante `COLA_INICIAL`).
- En el cliente: tope de 2000 líneas en el visor (`MAXIMO_DE_LINEAS`).

## Cómo habilitar / desplegar

- El socket ya es accesible bajo `/run/user/<uid>` (montado en `compose.yaml`/`Containerfile`); **no requiere montajes nuevos**.
- La ruta queda en la mitad **protegida** (gate OIDC por construcción).

## Decisiones

- **Fuente = API de Podman por socket** (no shell-out, no journald — no hay lectura del journal por D-Bus de usuario).
- **Mapeo por label** `PODMAN_SYSTEMD_UNIT` (no por nombre de fichero).
- Cliente ligero `hyper`+`hyperlocal` en vez de bollard (51 vs 146 paquetes, medido offline).
- Demux como **parser incremental puro** (probado con marcos fabricados).
- Puerto `FuenteDeRegistros` + doble para probar sin socket.
- Visor con `EventSource` **bajo demanda** (no canal permanente).
- El **plazo vencido** se mapea a `podman_unavailable` (503): desde fuera, un Podman que no contesta es un Podman que no está.

## Follow-ups

- Logs de unidades que no son contenedores (`.volume`/`.network`) — fuera de alcance.
- Sin descarga a fichero ni búsqueda/filtrado en el visor.
- El `EventSource` es uno por visor abierto.
