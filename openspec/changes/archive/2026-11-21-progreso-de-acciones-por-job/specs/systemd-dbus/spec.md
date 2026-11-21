# Spec Delta — systemd-dbus

## Purpose

El cliente D-Bus de podpot contra el manager de systemd del usuario. Este delta añade un puerto de eventos de progreso de job, alimentado por las señales `JobNew`/`JobRemoved` del manager, con doble de pruebas.

## ADDED Requirements

### Requirement: Eventos de progreso de job

La capa de systemd SHALL exponer un puerto de **eventos de progreso de job** que publique el ciclo de vida de los jobs del manager: al crearse un job (fase `new`) y al terminarse, con el resultado (fase `done`). La implementación real SHALL alimentarse de las señales `JobNew`/`JobRemoved` que systemd emite por D-Bus, sin sondeo, y SHALL publicar los eventos con independencia del origen del job. El doble de pruebas SHALL poder emitir estos eventos sin bus real. La publicación NO SHALL depender de que el panel haya solicitado el job.

El tipo de job NO viaja en esas señales (la firma real de `JobNew` es `(uos)`; la de `JobRemoved`, `(uoss)`), así que la implementación real SHALL leerlo de la propiedad `JobType` del objeto del job al procesar el alta y SHALL conservarlo para completar el evento de baja. Si no puede determinarlo —no vio el alta, o el objeto del job ya no existe cuando pregunta—, SHALL publicar el evento con el tipo a `unknown`, sin descartarlo.

#### Scenario: Alta de job reenviada

- **WHEN** el manager emite la señal de creación de un job
- **THEN** el puerto SHALL publicar un evento con la unidad, el tipo de job y la fase `new`

#### Scenario: El tipo de job se lee de la propiedad JobType

- **WHEN** el manager emite la señal de creación de un job y su objeto existe
- **THEN** el puerto SHALL publicar el evento con el tipo leído de la propiedad `JobType` de ese objeto, y SHALL recordarlo para el evento de baja del mismo job

#### Scenario: Tipo de job desconocido

- **WHEN** el tipo de un job no se puede determinar (no se vio su alta, o su objeto ya no existe cuando se pregunta)
- **THEN** el puerto SHALL publicar el evento igualmente, con el tipo a `unknown`

#### Scenario: Baja de job reenviada con su resultado

- **WHEN** el manager emite la señal de fin de un job
- **THEN** el puerto SHALL publicar un evento con la fase `done` y el resultado que systemd reporta, conservando el tipo que se obtuvo al procesar el alta

#### Scenario: El doble emite eventos sin bus

- **WHEN** una prueba usa el doble de systemd y este emite un evento de progreso de job
- **THEN** los suscriptores del puerto SHALL recibirlo, sin necesidad de un bus de sesión real
