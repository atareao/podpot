# Spec Delta — ui

## Purpose

El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas. Este delta añade el progreso de las acciones por unidad, consumido del canal de trabajos.

## MODIFIED Requirements

### Requirement: Control desde el panel

El panel SHALL permitir arrancar, parar y reiniciar una unidad y SHALL reflejar el resultado, refrescando el estado tras la acción. Mientras una acción está en curso SHALL mostrar su progreso por unidad, según el canal de trabajos (ver «Progreso de las acciones en el panel»).

#### Scenario: Arrancar desde el panel

- **WHEN** el usuario pulsa «arrancar» en una quadlet parada
- **THEN** el panel SHALL llamar a la API, mostrar el resultado y refrescar el estado de esa unidad

#### Scenario: Progreso mientras la acción está en curso

- **WHEN** una acción sobre una unidad tiene un job en curso
- **THEN** el panel SHALL mostrar en esa unidad un indicador de que la acción está en curso

#### Scenario: Error de acción

- **WHEN** la API responde con error a una acción
- **THEN** el panel SHALL mostrar un mensaje de error y mantener el estado real de la unidad

## ADDED Requirements

### Requirement: Progreso de las acciones en el panel

El panel SHALL suscribirse al canal SSE de trabajos (`GET /api/v1/jobs/stream`) y mantener, por unidad, el conjunto de jobs en curso (alta al recibir la fase `new`, baja al recibir la fase `done`). SHALL mostrar por unidad el indicador «en curso» con el tipo de acción mientras haya un job para esa unidad. La suscripción SHALL cerrarse al desmontar el panel y SHALL reconectar automáticamente si la conexión se corta. Si el canal no está disponible, el panel SHALL seguir funcionando con su comportamiento actual, sin el indicador de progreso.

#### Scenario: Indicador al crearse un job

- **WHEN** llega un evento de fase `new` para una unidad
- **THEN** el panel SHALL mostrar esa unidad como «en curso» con el tipo de acción

#### Scenario: Indicador al terminar el job

- **WHEN** llega un evento de fase `done` para una unidad
- **THEN** el panel SHALL dejar de mostrar el indicador de progreso de esa unidad

#### Scenario: Baja al desmontar

- **WHEN** el panel se desmonta
- **THEN** SHALL cerrar la suscripción al canal de trabajos

#### Scenario: Canal no disponible

- **WHEN** el canal de trabajos no puede establecerse o se corta
- **THEN** el panel SHALL seguir funcionando sin el indicador de progreso y SHALL reintentar la suscripción
