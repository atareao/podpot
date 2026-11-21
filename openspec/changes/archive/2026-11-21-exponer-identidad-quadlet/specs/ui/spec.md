# Spec Delta — ui

## Purpose

El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas. Este delta fija que el panel usa la identidad real (nombre de fichero) publicada por la API en lugar de deducirla del nombre de unidad.

## MODIFIED Requirements

### Requirement: Listado de quadlets

El panel SHALL mostrar un listado con cada unidad quadlet y su estado actual, obtenido de la API. Cada quadlet SHALL llevar su identidad real —el `file_name` publicado por la API— que es la clave con la que se la opera.

#### Scenario: Vista de estado

- **WHEN** el usuario abre el panel
- **THEN** SHALL ver cada quadlet con su estado (en ejecución, parada, fallida, …)

#### Scenario: Identidad publicada por la API

- **WHEN** el panel recibe el listado de `GET /api/v1/units`
- **THEN** SHALL tomar el `file_name` de cada quadlet como su identidad, sin reconstruirlo a partir del nombre de unidad

## ADDED Requirements

### Requirement: Identidad de quadlet para las acciones

El panel SHALL usar el `file_name` que publica la API como identidad de la quadlet para las acciones de editar y borrar. El panel NO SHALL deducir el nombre de fichero invirtiendo el nombre de unidad, porque esa regla inversa no es inyectiva y puede apuntar al fichero equivocado.

#### Scenario: Editar y borrar usan la identidad publicada

- **WHEN** el usuario edita o borra una quadlet del listado
- **THEN** el panel SHALL operar sobre el `file_name` recibido de la API para esa quadlet

#### Scenario: Dos quadlets con nombres de unidad colisionados

- **WHEN** el listado contiene dos quadlets cuyos nombres de unidad no permiten distinguirlas (p. ej. `web-volume.service`)
- **THEN** el panel SHALL operar sobre el `file_name` correcto de cada una, sin confundirlas
