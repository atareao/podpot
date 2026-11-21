# Design — unidades-en-vivo

## Contexto

El estado autoritativo de una quadlet vive en **systemd**, no en Podman, y se consulta por D-Bus. Hoy `DbusSystemdClient` ya usa `zbus` y consume un signal (`JobRemoved`, al esperar una acción), pero el trait `SystemdClient` no expone ningún flujo de eventos y `AppState` no tiene canal. El panel (`react-query`) refetchea a mano.

## Decisiones

### Señales reales, no polling

El vigilante se alimenta de las señales de D-Bus de systemd, no de un sondeo periódico: es la razón por la que se eligió D-Bus. Se escuchan:

- `org.freedesktop.DBus.Properties` `PropertiesChanged` sobre el espacio de rutas `/org/freedesktop/systemd1/unit/` (cambios de `ActiveState`/`SubState` de una unidad),
- `org.freedesktop.systemd1.Manager` `UnitNew`/`UnitRemoved` (aparece/desaparece una unidad),
- `UnitFilesChanged` (un `daemon-reload` tras crear/editar/borrar una quadlet).

Antes de escuchar se llama `Subscribe()` en el manager: sin él, systemd no emite señales de unidades que nadie observa. Las señales se **coalescen** (debounce corto) para no recalcular en ráfaga.

### El vigilante avisa «algo cambió»; el snapshot se recalcula

El puerto `UnitWatcher` no transporta datos de unidad, solo **notificaciones** (`broadcast::Receiver<()>`). Sobre cada aviso, el backend recalcula la instantánea con `SystemdClient::quadlets()` y la publica. Ventajas: una sola forma de construir la instantánea (la que ya existe y ya está probada), el doble de pruebas solo tiene que emitir avisos, y el puerto queda trivial. El coste (recalcular `ListUnits` + estados por señal) es despreciable para el número de quadlets de un usuario.

### El canal del `AppState` lleva la instantánea; la API la traduce

`AppState` gana `eventos: broadcast::Sender<Vec<UnitStatus>>`. Una tarea de fan-out (arrancada en `main`) consume los avisos del vigilante, recalcula y publica la instantánea **de dominio**. La capa API (ruta SSE) traduce `UnitStatus` → `UnitSummary` (mismo recorte que `GET /api/v1/units`) y la serializa en el evento. Así el dominio no conoce HTTP y el contrato de datos del stream es **idéntico** al del listado.

### Un evento con la instantánea completa, no deltas

Cada evento `units` lleva la lista **completa** de quadlets (no un delta). Es idempotente, evita estados divergentes por un evento perdido y reutiliza el mismo tipo que el listado; el frontend simplemente reemplaza su dato. El tamaño es pequeño.

### Envoltura: el SSE es la excepción

`api-shell` exige envoltura JSON única en las rutas de negocio. El stream **no** puede cumplirla (es una respuesta continua). Se **modifica** el requirement para exceptuar explícitamente el stream, y se documenta que el **contenido de cada evento** sí reutiliza la envoltura `{data}` para que el cliente no necesite un parser distinto.

### Cierre ordenado

Un SSE perpetuo colgaría `axum::serve(...).with_graceful_shutdown(...)`, que espera las conexiones en curso. El stream escucha un canal de cierre (o el mismo `broadcast` con un mensaje de fin) y termina cuando el servidor se apaga. Se documenta en `api-shell` (apagado ordenado) y en la capability.

### Frontend: `EventSource` + `react-query`

Se engancha un `EventSource('/api/v1/units/stream')` (mismo origen; Vite ya proxya `/api` en dev, así que no hay CORS ni config nueva). Al recibir un evento, `queryClient.setQueryData(claves.units, instantanea)`. `EventSource` reconecta solo; se cierra en el cleanup del efecto. No se sustituye el `GET` inicial: el SSE envía la instantánea al conectar y también sirve de primer pintado.

## Alternativas descartadas

- **Polling periódico del backend**: niega la ventaja de D-Bus (estado tipado y push) y añade latencia; descartado salvo como red de seguridad futura.
- **Deltas por unidad** (`Cambio`/`Eliminada`): más eficiente en teoría, pero frágil ante eventos perdidos y con más superficie de test; la instantánea completa es idempotente.
- **WebSocket**: bidireccional, innecesario; el flujo es de servidor a cliente.
- **Long-polling**: más complejo que SSE y sin reconexión automática del navegador.
