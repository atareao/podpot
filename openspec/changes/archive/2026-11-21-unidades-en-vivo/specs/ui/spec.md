# Spec Delta — ui

## Purpose

El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas. Este delta hace que el panel sea en vivo: refleja los cambios de estado sin que el usuario actúe.

## ADDED Requirements

### Requirement: Panel en vivo

El panel SHALL mantenerse al día con el estado real de las quadlets suscribiéndose al canal SSE (`GET /api/v1/units/stream`). Al recibir un evento SHALL actualizar el listado con la instantánea recibida, sin esperar a una acción del usuario. La suscripción SHALL cerrarse al desmontar el panel y SHALL reconectar automáticamente si la conexión se corta. Si el canal no está disponible, el panel SHALL seguir funcionando con la carga inicial de `GET /api/v1/units`.

#### Scenario: Cambio reflejado sin acción del usuario

- **WHEN** cambia el estado de una quadlet fuera del panel (otro cliente, `systemctl --user`, un fallo)
- **THEN** el panel SHALL reflejar el nuevo estado al recibir el evento del canal

#### Scenario: Baja al desmontar

- **WHEN** el panel se desmonta
- **THEN** SHALL cerrar la suscripción al canal

#### Scenario: Canal no disponible

- **WHEN** el canal SSE no puede establecerse o se corta
- **THEN** el panel SHALL seguir mostrando el listado de la carga inicial y SHALL reintentar la suscripción
