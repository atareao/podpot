# ui Specification

## Purpose
El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas.

## Requirements

### Requirement: Listado de quadlets

El panel SHALL mostrar un listado con cada unidad quadlet y su estado actual, obtenido de la API.

#### Scenario: Vista de estado

- **WHEN** el usuario abre el panel
- **THEN** SHALL ver cada quadlet con su estado (en ejecución, parada, fallida, …)

### Requirement: Control desde el panel

El panel SHALL permitir arrancar, parar y reiniciar una unidad y SHALL reflejar el resultado, refrescando el estado tras la acción.

#### Scenario: Arrancar desde el panel

- **WHEN** el usuario pulsa «arrancar» en una quadlet parada
- **THEN** el panel SHALL llamar a la API, mostrar el resultado y refrescar el estado de esa unidad

#### Scenario: Error de acción

- **WHEN** la API responde con error a una acción
- **THEN** el panel SHALL mostrar un mensaje de error y mantener el estado real de la unidad
