# progreso-de-trabajos Specification

## Purpose

El canal de progreso de los jobs de systemd: un endpoint SSE que empuja el ciclo de vida de cada job (creado, terminado y con qué resultado), alimentado por las señales D-Bus del manager, para que el panel muestre el avance de las acciones por unidad.

## ADDED Requirements

### Requirement: Canal SSE de progreso de jobs

El servidor SHALL exponer `GET /api/v1/jobs/stream` como un flujo `text/event-stream`. SHALL emitir un evento `job` por cada transición del ciclo de vida de un job de systemd: al crearse el job (fase `new`) y al terminarse (fase `done`). Cada evento SHALL transportar, al menos, el identificador del job, la unidad, el tipo de job (`start`/`stop`/`restart`/…; o `unknown` si no se puede determinar) y la fase; en la fase `done` SHALL incluir además el resultado (`done`/`failed`/…). El flujo SHALL indicar un `retry` y SHALL enviar latidos periódicos para sostener la conexión a través de proxies.

#### Scenario: Job recién creado

- **WHEN** systemd crea un job (p. ej. tras pedir arrancar una unidad)
- **THEN** SHALL emitirse un evento `job` con la unidad, el tipo de job y la fase `new`

#### Scenario: Tipo de job desconocido

- **WHEN** el tipo de un job no se puede determinar (no se vio su alta, o su objeto ya no existe cuando se pregunta)
- **THEN** SHALL emitirse igualmente el evento `job` con el campo del tipo a `unknown`, sin descartarlo ni inventarse un tipo

#### Scenario: Job terminado

- **WHEN** un job en curso termina
- **THEN** SHALL emitirse un evento `job` con la fase `done` y el resultado de systemd

#### Scenario: Latido y reconexión

- **WHEN** no hay transiciones durante un tiempo
- **THEN** el flujo SHALL seguir enviando latidos y SHALL haber indicado un `retry` para que el cliente reconecte si la conexión se corta

#### Scenario: Sin instantánea inicial

- **WHEN** un cliente abre `GET /api/v1/jobs/stream`
- **THEN** el flujo SHALL quedar abierto a la espera de transiciones, sin enviar una instantánea de jobs en curso

### Requirement: El canal se alimenta de las señales de jobs de systemd

El canal SHALL alimentarse de las señales de ciclo de vida de jobs que systemd emite por D-Bus (`JobNew`/`JobRemoved`), no de un sondeo periódico. SHALL cubrir los jobs con independencia de su origen, incluidos los que el panel no haya solicitado. Como el tipo de job no viaja en esas señales, el servidor SHALL obtenerlo de la propiedad `JobType` del objeto del job y SHALL conservarlo para el evento de baja; cuando no pueda determinarlo, SHALL publicar el evento con el tipo a `unknown` en vez de descartarlo.

#### Scenario: Señal de job reenviada como evento

- **WHEN** systemd emite por D-Bus una señal de alta o baja de job
- **THEN** el servidor SHALL publicarla en el canal como evento `job`

#### Scenario: El tipo de job se obtiene del objeto del job

- **WHEN** llega la señal de alta de un job
- **THEN** el servidor SHALL leer la propiedad `JobType` del objeto de ese job para conocer su tipo, y SHALL recordarlo para el evento de baja —que no lo trae—

#### Scenario: Job de origen ajeno al panel

- **WHEN** systemd arranca un job que el panel no solicitó (reinicio automático, dependencia)
- **THEN** el servidor SHALL emitir igualmente su evento `job`

### Requirement: Cierre ordenado del canal

El servidor SHALL terminar los flujos SSE de trabajos abiertos al apagarse, sin bloquear el apagado ordenado.

#### Scenario: Apagado con un cliente conectado

- **WHEN** el servidor recibe una señal de terminación con un cliente suscrito al canal de trabajos
- **THEN** SHALL cerrar el flujo y completar el apagado sin quedarse esperando a la conexión
