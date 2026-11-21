# Tasks — registros-de-quadlets

TDD por bloques: primero las pruebas (rojo), luego la implementación mínima (verde), luego refactor. Orden por dependencia: config/deps → cliente Podman (socket + demux) → puerto + doble → API SSE → frontend → cierre.

## 1. Configuración y dependencias

- [x] 1.1 `PODPOT_PODMAN_SOCKET` en `config.rs` (default `$XDG_RUNTIME_DIR/podman/podman.sock`). — Verificación: tests de valor por defecto y explícito (rojo→verde).
- [x] 1.2 Añadir `hyper` (client,http1) + `hyperlocal` + `http-body-util` a `Cargo.toml`. — Verificación: `cargo check --offline` compila.

## 2. Cliente Podman (socket + demux)

- [x] 2.1 Demux del stream de logs (marco Docker → líneas con origen) como **función pura**. — Verificación: tests con marcos fabricados (stdout/stderr, multilínea, marco partido).
- [x] 2.2 Cliente por socket unix: listar contenedores por label `PODMAN_SYSTEMD_UNIT=<unidad>` y seguir logs (`follow`). — Verificación: test con un servidor unix simulado (o doble) que responda listado y stream.
- [x] 2.3 Unidad sin contenedor → error explícito. — Verificación: test de listado vacío → error.

## 3. Puerto y doble

- [x] 3.1 Puerto `FuenteDeRegistros` (devuelve `mpsc::Receiver<LineaDeRegistro>`); `AppState` gana la fuente. — Verificación: test que construye el estado con el doble.
- [x] 3.2 Doble de pruebas que entrega líneas sin socket. — Verificación: test que emite desde el doble y recibe.

## 4. API SSE

- [x] 4.1 `GET /api/v1/units/{name}/logs/stream` (`text/event-stream`): cola inicial + evento `log` por línea con la envoltura `{data:{line,stream}}`. — Verificación: test de integración que lee la cola inicial y, tras emitir en el doble, la línea nueva.
- [x] 4.2 `retry`, latido, evento terminal y cierre con el servidor. — Verificación: test de `retry`/latido y de cierre por el `watch`.
- [x] 4.3 Unidad sin contenedor → error envuelto (no se abre el flujo); `405` con envoltura; protegida por el gate. — Verificación: tests de integración de los tres casos.
- [x] 4.4 Registrar la ruta en la mitad **protegida** de `api/mod.rs`. — Verificación: test de 401 sin sesión en modo `oidc`.

## 5. Frontend

- [x] 5.1 Cliente/hook del canal de logs (evento `log` con nombre; `close()` al cerrar). — Verificación: test con `EventSource` simulado.
- [x] 5.2 Drawer del visor, abierto desde una acción de fila en `TablaDeUnidades`; muestra líneas y distingue origen. — Verificación: test de componente (abrir → líneas; cerrar → baja; error → aviso).

## 6. Cierre

- [x] 6.1 `just check-all` en verde y `clippy -D warnings` limpio. — Verificación: salida EXIT 0.
- [x] 6.2 Reviewers (`rust-reviewer`, `react-reviewer`) sin bloqueantes. — Verificación: informes de review.
- [x] 6.3 `openspec validate --all` en verde y `docs/registros-de-quadlets.md` redactado. — Verificación: validación OK y doc existente.
