# unidades-en-vivo Specification

## Purpose
El canal de eventos que mantiene el panel al día: un endpoint SSE que empuja la instantánea de las quadlets cuando systemd cambia, alimentado por un vigilante de señales D-Bus.

## Requirements

### Requirement: Canal SSE de estado de quadlets

El servidor SHALL exponer `GET /api/v1/units/stream` como un flujo `text/event-stream`. Al establecer la conexión SHALL enviar de inmediato un evento `units` con la instantánea actual de las quadlets (misma forma de datos que `GET /api/v1/units`). Después SHALL emitir un nuevo evento `units` con la instantánea recalculada cada vez que cambie el estado de las quadlets. El flujo SHALL indicar un `retry` y SHALL enviar latidos periódicos para sostener la conexión a través de proxies.

#### Scenario: Instantánea inicial al conectar

- **WHEN** un cliente abre `GET /api/v1/units/stream`
- **THEN** SHALL recibir un evento `units` con la lista actual de quadlets sin esperar a que ocurra ningún cambio

#### Scenario: Empuje al cambiar el estado

- **WHEN** cambia el estado de una quadlet (arranca, para, falla) estando la conexión abierta
- **THEN** SHALL recibirse un evento `units` con la instantánea recalculada que refleja el cambio

#### Scenario: Latido y reconexión

- **WHEN** no hay cambios durante un tiempo
- **THEN** el flujo SHALL seguir enviando latidos y SHALL haber indicado un `retry` para que el cliente reconecte si la conexión se corta

### Requirement: El canal se alimenta de las señales de systemd

El canal SHALL alimentarse de las señales que systemd emite por D-Bus (cambios de propiedades de las unidades y alta/baja de unidades), no de un sondeo periódico. Las señales SHALL coalescerse para no recalcular la instantánea en ráfaga.

#### Scenario: Cambio detectado por señal

- **WHEN** systemd emite una señal de cambio de una unidad
- **THEN** el servidor SHALL recalcular la instantánea y publicarla en el canal

#### Scenario: Ráfaga de señales

- **WHEN** llegan varias señales seguidas en un intervalo corto
- **THEN** el servidor SHALL coalescerlas en una sola publicación de la instantánea

### Requirement: Cierre ordenado del canal

El servidor SHALL terminar los flujos SSE abiertos al apagarse, sin bloquear el apagado ordenado.

#### Scenario: Apagado con un cliente conectado

- **WHEN** el servidor recibe una señal de terminación con un cliente suscrito al stream
- **THEN** SHALL cerrar el flujo y completar el apagado sin quedarse esperando a la conexión

### Requirement: Resumen de unidad con stack

El resumen de cada unidad SHALL incluir un campo `stack: String` derivado de forma determinista de `file_name` y `kind`. La regla base SHALL ser el primer segmento del nombre base del fichero (stem) antes del primer `-`, conservando la grafía original; si no hay `-`, SHALL ser el stem completo. Las unidades auxiliares (`volume`, `network`) y las principales (`container`, `pod`, `kube`, `build`, `image`) SHALL caer en el mismo stack cuando compartan prefijo; no se recorta ningún sufijo de tipo separadamente. Si no hay `file_name` o el tipo no es reconocido, el stack SHALL derivarse del nombre de la unidad sin `.service` aplicando la misma regla de prefijo. Un `groups.toml` en el directorio de la fuente SHALL poder sobrescribir el resultado. El campo SHALL ser estable ante cambios de `active_state` o `sub_state`.

**Regla detallada (prefijo + override):**
- **Regla base**: el stack SHALL ser el primer segmento del stem antes del primer `-` (case-insensitive para la decisión, conservando la grafía original del primer segmento). Si no hay `-`, el stack SHALL ser el stem completo.
- **Override explícito**: un archivo `groups.toml` en el directorio de la fuente del quadlet SHALL permitir sobrescribir el stack mediante una tabla `[mappings]` con claves `file_name` o `unit_name`. El override SHALL tener prioridad sobre la regla base.
- **Fallbacks**: si no hay `file_name`, se SHALL usar `unit_name` sin `.service` y sin sufijo de tipo. El campo `stack` SHALL ser estable y no SHALL depender de `active_state` ni `sub_state`.

#### Scenario: Principal y auxiliares comparten stack

- **GIVEN** los ficheros `foo.container`, `foo.volume` y `foo.network`
- **WHEN** se lista `GET /api/v1/units`
- **THEN** `foo.service`, `foo-volume.service` y `foo-network.service` SHALL tener `stack == "foo"`

#### Scenario: Recurso auxiliar independiente

- **GIVEN** el fichero `backup.volume` sin un `backup.container` asociado
- **WHEN** se lista el estado
- **THEN** `backup-volume.service` SHALL tener `stack == "backup"` y formar su propio grupo

#### Scenario: Sufijo en la unidad principal no se recorta

- **GIVEN** el fichero `foo-volume.container`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `foo` (prefijo antes del primer `-`)

#### Scenario: Sin tipo reconocido

- **GIVEN** una unidad sin `file_name` o con `kind == null`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL derivarse del nombre de la unidad sin el sufijo `.service`, sin panic ni valor vacío

#### Scenario: Estabilidad en vivo

- **GIVEN** una unidad con `stack == "foo"`
- **WHEN** cambia su `active_state` por un evento en vivo
- **THEN** su `stack` SHALL permanecer `foo`

#### Scenario: Regla base con prefijo

- **GIVEN** una unidad con `file_name = "atareao-mariadb.container"`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `"atareao"`

#### Scenario: Regla base sin prefijo

- **GIVEN** una unidad con `file_name = "standalone.container"`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `"standalone"`

#### Scenario: Fallback a unit_name sin file_name

- **GIVEN** una unidad sin `file_name` pero con `unit_name = "centinela-app.service"`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `"centinela"`

#### Scenario: Fallback a unit_name sin sufijo

- **GIVEN** una unidad sin `file_name` pero con `unit_name = "singleton.service"`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `"singleton"`

#### Scenario: Override explícito en groups.toml

- **GIVEN** `groups.toml` contiene `"atareao-mariadb.container" = "db-cluster"`
- **WHEN** se calcula el stack para `atareao-mariadb.container`
- **THEN** el stack SHALL ser `"db-cluster"` (override gana sobre regla base)

#### Scenario: Override explícito para unit_name

- **GIVEN** `groups.toml` contiene `"centinela-app.service" = "monitoring"`
- **WHEN** se calcula el stack para `centinela-app.service`
- **THEN** el stack SHALL ser `"monitoring"` (override gana sobre regla base)
