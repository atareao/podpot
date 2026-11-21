# Design — progreso-de-acciones-por-job

## Contexto

El estado de una quadlet se lee por D-Bus y se empuja por el canal SSE `units` con las señales de systemd (`PropertiesChanged`, `UnitNew`/`UnitRemoved`). Ese canal refleja **estados** (`ActiveState`/`SubState`), incluidos los intermedios (`activating`/`deactivating`), pero no el **ciclo de vida de los jobs**: no dice qué acción se pidió, ni si el job sigue en curso, ni cómo terminó. La decisión de usar D-Bus directo (empuje tipado, sin parsear texto) es la que permite obtener ahora estos eventos sin sondeo.

## Decisiones

### Señales del manager, no polling

`org.freedesktop.systemd1.Manager` emite `JobNew(id, job, unit)` al crear un job y `JobRemoved(id, job, unit, result)` al terminarlo —las dos firmas están **medidas** contra el systemd de la máquina de desarrollo (262): `JobNew` es `(uos)` y **no** lleva el tipo de job, por más que la documentación de systemd lo dé por hecho en algún sitio—. El **tipo de job** (`start`/`stop`/`restart`/…) vive en la propiedad `JobType` del objeto del job (`org.freedesktop.systemd1.Job`), así que se lee de ahí con una llamada extra al recibir el alta, y se recuerda para el evento de baja —cuando el job ya no existe y la propiedad ya no se puede leer—. Esa lectura tiene una **carrera inevitable**: un job que nace y muere entre la señal y la pregunta deja su evento con `kind: "unknown"`; también queda en `unknown` el job del que no se vio el alta (el vigilante se resuscribe tras una caída del bus y se pierde el nacimiento), y en ningún caso se deja de publicar el evento.

Se escuchan esas señales y se reenvían. No hay sondeo, y cubre **todos** los jobs del usuario (los del panel y los que systemd arranca por su cuenta: reinicios automáticos, dependencias, otros clientes).

### Un puerto propio, alimentado como el vigilante

Se añade el puerto `ProgresoDeTrabajos` (suscripción a un `broadcast::Receiver<ProgresoDeJob>`), análogo a `UnitWatcher`. El cliente D-Bus real posee un `broadcast::Sender<ProgresoDeJob>` (como ya posee el de avisos de unidades) y publica `new` al ver `JobNew` y `done` con el resultado al ver `JobRemoved`. El doble de pruebas publica lo mismo sin bus. La publicación **no** se hace desde `action()`: así el canal cubre también los jobs que el panel no origina, y una única ruta (señal → evento) sirve para todo.

### Evento por transición, sin instantánea inicial

A diferencia del canal de estado (que envía una instantánea idempotente al conectar), el progreso de jobs es **edge-triggered**: cada evento describe una transición (`new`, `done`). No hay instantánea inicial porque un job «en curso» es efímero y el estado resultante ya lo entrega el canal `units`. El cliente mantiene el conjunto de jobs en curso a partir de `new`/`done`.

### Canal y endpoint propios

`GET /api/v1/jobs/stream`, con nombre de evento `job` y contenido `{data}` (misma envoltura que el resto). Se descarta **fundirlo** con el canal de unidades: son preocupaciones distintas (estado idempotente frente a transiciones), con ciclos de vida independientes y pruebas más simples por separado; el coste de una segunda conexión `EventSource` es despreciable en un panel de un solo usuario.

### El resultado se entrega por el canal; la acción sigue siendo síncrona

No se cambia `POST /api/v1/units/{name}/{action}`: sigue esperando el job y devolviendo su resultado (es la fuente de verdad de errores HTTP y de la notificación). El canal de trabajos es **aditivo**: pinta el progreso mientras la acción está en vuelo y muestra los jobs de origen ajeno. Hacer la acción asíncrona (202 + finalización por SSE) se descarta: perdería el mapeo directo de errores a HTTP y obligaría a manejar la caída del canal como caso de «acción colgada», con más superficie y menos robustez.

### Suscripción única por conexión

`Subscribe()` se pide **por conexión**, no por regla, y podpot tiene dos tareas escuchando por la misma: el vigilante de unidades y el de trabajos. La segunda en llamar recibe `org.freedesktop.systemd1.AlreadySubscribed`, que no es un fallo sino la confirmación de que la conexión ya recibe señales; ambos manantiales lo piden a través de un mismo helper que lo toma por bueno. Sin él, la segunda escucha se quedaba en un bucle de reintentos y no llegaba a registrar sus reglas —fue exactamente el fallo que destapó el test contra el bus real—.

### Cierre ordenado

El flujo escucha el mismo `watch` de apagado que el canal de unidades y termina cuando el servidor se apaga, sin bloquear `with_graceful_shutdown`.

### Frontend: suscripción única y estado por unidad

Un hook `EventSource('/api/v1/jobs/stream')` mantiene, en memoria, el mapa de jobs en curso por unidad (se añade en `new`, se quita en `done`). La tabla muestra el indicador «en curso» para las unidades con job, con el tipo de acción. Se cierra en el cleanup del efecto; si el canal no está, el panel degrada al comportamiento actual (botón deshabilitado + notificación al resolverse la petición).

## Alternativas descartadas

- **Publicar desde `action()`**: solo cubriría las acciones del panel y se perdería el job ya en curso por reinicio automático de systemd; además duplicaría la ruta de publicación.
- **Sondeo periódico de jobs (`ListJobs`)**: niega la ventaja de D-Bus (empuje) y añade latencia; descartado.
- **Fundir el canal de trabajos en el de unidades**: menos conexiones, pero mezcla estado idempotente con transiciones, complica el handler y las pruebas, y no aporta valor real.
- **Acción asíncrona (202) con finalización por SSE**: más «moderna», pero degrada el manejo de errores y la robustez; descartada.
- **Deltas de estado en vez de transiciones de job**: el canal `units` ya entrega el estado final idempotente; el job solo aporta la transición.
