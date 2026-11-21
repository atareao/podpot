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
