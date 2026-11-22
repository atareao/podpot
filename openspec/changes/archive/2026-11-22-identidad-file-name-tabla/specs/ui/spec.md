# Spec Delta — ui

## Purpose

El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas. Este delta precisa la identidad **mostrada** de la fila del listado: la tabla identifica cada quadlet por su `file_name`, dejando el nombre de unidad como dato secundario.

## MODIFIED Requirements

### Requirement: Listado de quadlets

El panel SHALL mostrar un listado con cada unidad quadlet y su estado actual, obtenido de la API. Cada quadlet SHALL llevar su identidad real —el `file_name` publicado por la API— que es la clave con la que se la opera. La tabla SHALL mostrar el `file_name` como identidad primaria de cada fila, con el nombre de unidad systemd (`name`) como dato secundario visible, y SHALL usar el `file_name` en los nombres accesibles de las acciones y del progreso. La comparación interna del progreso y de la acción en curso SHALL seguir usando el nombre de unidad, porque es el identificador que publican el canal de trabajos y la API de acciones.

#### Scenario: Vista de estado

- **WHEN** el usuario abre el panel
- **THEN** SHALL ver cada quadlet con su estado (en ejecución, parada, fallida, …)

#### Scenario: Identidad publicada por la API

- **WHEN** el panel recibe el listado de `GET /api/v1/units`
- **THEN** SHALL tomar el `file_name` de cada quadlet como su identidad, sin reconstruirlo a partir del nombre de unidad

#### Scenario: Identidad primaria de la fila

- **WHEN** el panel muestra el listado de quadlets
- **THEN** cada fila SHALL mostrar el `file_name` como identidad primaria
- **AND** SHALL mostrar el nombre de unidad como dato secundario

#### Scenario: Nombres accesibles coherentes con la identidad

- **WHEN** una fila ofrece acciones o muestra progreso
- **THEN** los nombres accesibles de las acciones y del progreso SHALL usar el `file_name` de la quadlet

#### Scenario: Estado de acción en curso por nombre de unidad

- **WHEN** una acción está en curso sobre una unidad
- **THEN** la tabla SHALL casar el indicador de progreso y el bloqueo de acciones por el nombre de unidad, que es el identificador que publican el canal de trabajos y la API de acciones

---
