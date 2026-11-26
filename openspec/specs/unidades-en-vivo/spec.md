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

`UnitSummary` SHALL incluir un campo `stack: String` derivado de forma determinista de `file_name` y `kind`. Para unidades principales (`container`, `pod`, `kube`, `build`, `image`) el stack SHALL ser el nombre base del fichero sin extensión. Para unidades auxiliares (`volume`, `network`) el stack SHALL ser el nombre base sin el sufijo `-volume`/`-network` cuando éste esté presente y no vacíe el nombre; en caso contrario SHALL ser el nombre base completo. Si no hay `file_name` o el tipo no es reconocido, el stack SHALL derivarse del nombre de la unidad sin `.service`. El campo SHALL ser estable ante cambios de `active_state` o `sub_state`.

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
- **THEN** el stack SHALL ser `foo-volume` porque el tipo `container` manda sobre el sufijo del nombre

#### Scenario: Sin tipo reconocido

- **GIVEN** una unidad sin `file_name` o con `kind == null`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL derivarse del nombre de la unidad sin el sufijo `.service`, sin panic ni valor vacío

#### Scenario: Estabilidad en vivo

- **GIVEN** una unidad con `stack == "foo"`
- **WHEN** cambia su `active_state` por un evento en vivo
- **THEN** su `stack` SHALL permanecer `foo`
