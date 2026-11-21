# Spec Delta — ui

## Purpose

El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas. Este delta añade el formulario de creación de quadlets nuevas.

## ADDED Requirements

### Requirement: Creación desde el panel

El panel SHALL ofrecer un formulario de creación (Ant Design) con un selector de tipo de quadlet y los campos mínimos del tipo elegido. Al crear con éxito SHALL refrescar el listado de quadlets; al fallar SHALL mostrar el `error.message` devuelto por la API.

#### Scenario: Creación correcta desde el panel

- **WHEN** el usuario rellena el formulario y confirma la creación
- **THEN** el panel SHALL llamar a la API y, al recibir éxito, refrescar el listado de quadlets

#### Scenario: Selector de tipo

- **WHEN** el usuario elige un tipo en el formulario
- **THEN** SHALL mostrarse los campos mínimos correspondientes a ese tipo

#### Scenario: Error de creación

- **WHEN** la API responde con error a la creación
- **THEN** el panel SHALL mostrar el `error.message` recibido sin refrescar el listado como si hubiera tenido éxito
