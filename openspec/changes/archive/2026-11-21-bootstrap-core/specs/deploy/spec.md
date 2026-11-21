# Spec Delta — deploy

## Purpose

El empaquetado de podpot como contenedor rootless capaz de hablar con el systemd y el Podman del host.

## ADDED Requirements

### Requirement: Imagen autocontenida

La imagen SHALL incluir todas las dependencias en tiempo de construcción y correr como un usuario no-root cuyo uid coincida con el del host; NUNCA SHALL instalar paquetes en tiempo de ejecución.

#### Scenario: Arranque de la imagen

- **WHEN** el contenedor arranca con los montajes de producción
- **THEN** el proceso SHALL iniciarse sin instalar nada y `GET /api/v1/health` SHALL responder `200`

### Requirement: Montajes del host

El despliegue de referencia SHALL montar el bus de usuario, el directorio de quadlets y el socket de Podman, y SHALL usar `--userns=keep-id` (uid coincidente), sin el cual el bus no responde.

#### Scenario: Bus accesible

- **WHEN** el contenedor corre con `--userns=keep-id` y el bus de usuario montado
- **THEN** el cliente D-Bus SHALL poder consultar el manager de usuario

#### Scenario: Bus ausente

- **WHEN** el contenedor corre sin el bus de usuario montado
- **THEN** la aplicación SHALL informar de forma explícita de que no puede alcanzar systemd, en vez de fallar de forma opaca

### Requirement: Panel incorporado

La imagen SHALL incorporar el build del panel y servirlo desde el propio backend (un único contenedor autocontenido), sin depender de un servidor web externo.

#### Scenario: Panel servido por el contenedor

- **WHEN** el contenedor arranca con los montajes de producción
- **THEN** `GET /` SHALL devolver el panel y `GET /api/v1/health` SHALL responder `200`
