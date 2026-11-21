# Design — registros-de-quadlets

## Contexto

El panel en vivo ya usa SSE (canal `units`) alimentado por señales D-Bus. Para los logs hay que leer los del **contenedor** de la quadlet. No hay lectura de journal por D-Bus de usuario; la vía alineada con la política del proyecto (sin shell-out) es la **API de Podman por su socket unix**, que ya está montado en el contenedor (`/run/user/<uid>` incluye `podman/podman.sock`).

## Decisiones

### Fuente: API de Podman por socket, no shell-out

Se habla con el socket `$XDG_RUNTIME_DIR/podman/podman.sock` usando la API compat (Docker-like) que Podman implementa. **No** se invoca `journalctl` ni `podman logs`: el proyecto eligió explícitamente no depender de binarios (`dbus.rs`). Verificado en esta máquina (podman 6.1.4): `/v1.40/containers/json` responde por el socket.

### Mapeo unidad → contenedor por label

Los contenedores creados bajo systemd llevan la label **`PODMAN_SYSTEMD_UNIT=<unidad>.service`** (verificado: `centinela-db.service`, `oxinbox-db.service`…). Se localiza el contenedor con:

```
GET /containers/json?all=true&filters={"label":["PODMAN_SYSTEMD_UNIT=<name>"]}
```

donde `<name>` es el nombre de la unidad (p. ej. `centinela-db.service`). `all=true` incluye contenedores parados (que aún tienen logs). Si no hay contenedor → error explícito (no se abre el flujo). Así no hay que leer `ContainerName=` del fichero: la label es la fuente de verdad.

### Cliente ligero: hyper + hyperlocal

Solo se necesitan dos llamadas (listar por label, seguir logs). Se escribe un cliente mínimo sobre `hyper` (client, http1) + `hyperlocal` (conector por socket unix), en vez de añadir **bollard** (árbol de deps 3× mayor: 146 vs 51 paquetes, medido offline). Da control total del stream y encaja con el minimalismo del proyecto.

### Demultiplexado del stream de logs como función pura

Si el contenedor **no** tiene TTY, Podman devuelve el stream **multiplexado** (marco Docker: 1 byte de origen — 0 stdin/1 stdout/2 stderr —, 3 de relleno, 4 de longitud big-endian, y el payload). Si **tiene** TTY, el stream es crudo. La decisión se toma inspeccionando el contenedor (`Config.Tty`). El demux se implementa como **función pura** (bytes → líneas con su origen) y se prueba aparte con marcos fabricados, sin socket.

### Puerto `FuenteDeRegistros` con doble

Como `SystemdClient`/`FakeSystemd`: un trait que devuelve un `tokio::sync::mpsc::Receiver<LineaDeRegistro>` (la implementación real lanza la tarea HTTP+demux; el doble lo alimenta desde el test). El handler SSE no conoce Podman; así los escenarios se prueban sin socket.

### SSE: evento `log` por línea, reutilizando el patrón existente

Mismo patrón que `api/stream.rs`: `Sse::new(...).keep_alive(...)`, `retry`, y cierre por el `watch` de apagado compartido. El evento se llama `log` y su contenido usa la envoltura `{data:{line,stream}}` (consistente con la excepción a la envoltura única ya fijada en `api-shell`). Se envía primero la cola inicial (`tail`, por defecto p. ej. 200 líneas) y luego las nuevas. Un evento terminal `log-end` marca el fin del stream (contenedor parado sin `follow`, o fin de la cola).

### Cierre ordenado

Igual que el canal `units`: el flujo escucha el `watch` de apagado y termina; no bloquea `with_graceful_shutdown`. La tarea HTTP+demux se cancela al cerrar el stream.

### Auth

La ruta cuelga de la mitad **protegida** (`negocio`), así que el gate la cubre por construcción (401 envuelto sin sesión, como el canal `units`).

### Frontend: un drawer, no un segundo canal permanente

Los logs solo interesan **mientras el visor está abierto**: el `EventSource` se abre al abrir el drawer y se cierra al cerrarlo (no un canal permanente por unidad). Así no se multiplican conexiones. Se reutiliza el patrón de `useUnidadesEnVivo` (evento con nombre, `close()` en cleanup).

## Alternativas descartadas

- **bollard**: correcto y disponible offline, pero 146 paquetes frente a 51; innecesario para dos llamadas.
- **Shell-out a `journalctl`/`podman logs`**: rompe la política del proyecto y complica el empaquetado (binarios en la imagen).
- **journald por ficheros**: exigiría montar `/run/log/journal` y permisos que hoy no están.
- **Canal SSE permanente de logs**: multiplicaría conexiones; el drawer bajo demanda es suficiente.
