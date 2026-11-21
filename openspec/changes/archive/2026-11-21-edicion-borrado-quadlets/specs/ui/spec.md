# Spec Delta — ui

## Purpose

El panel de podpot (React + Ant Design). Este delta añade las acciones de consultar, editar y borrar una quadlet existente, con precarga de campos al editar, confirmación en el borrado y refresco del listado.

## ADDED Requirements

### Requirement: Consulta y precarga al editar una quadlet

El panel SHALL leer la definición de una quadlet existente antes de editarla (vía `GET /api/v1/quadlets/{name}`) y SHALL precargar el formulario con su tipo y sus campos reales. Al fallar la lectura SHALL mostrar el `error.message` devuelto por la API y NO SHALL abrir el formulario con valores inventados.

#### Scenario: Precarga de campos

- **WHEN** se abre la edición de una quadlet existente
- **THEN** el formulario SHALL precargarse con el tipo y los campos devueltos por la API

#### Scenario: Error de consulta

- **WHEN** la lectura de la quadlet falla
- **THEN** el panel SHALL mostrar el `error.message` devuelto por la API

### Requirement: Edición de quadlets desde el panel

El panel SHALL ofrecer la acción de editar una quadlet existente, reutilizando el formulario de creación precargado con el tipo y los campos de la quadlet seleccionada. Al guardar con éxito SHALL refrescar el listado; al fallar SHALL mostrar el `error.message` devuelto por la API.

#### Scenario: Edición correcta

- **WHEN** se guarda una edición válida desde el panel
- **THEN** la API SHALL responder con éxito y el listado SHALL refrescarse

#### Scenario: Error de edición

- **WHEN** la API rechaza una edición
- **THEN** el panel SHALL mostrar el `error.message` devuelto

### Requirement: Borrado de quadlets desde el panel

El panel SHALL ofrecer la acción de borrar una quadlet existente, solicitando confirmación antes de ejecutarla (acción destructiva). Al confirmar con éxito SHALL refrescar el listado; al fallar SHALL mostrar el `error.message` devuelto por la API.

#### Scenario: Confirmación antes de borrar

- **WHEN** se pulsa borrar sobre una quadlet
- **THEN** el panel SHALL pedir confirmación antes de enviar la petición

#### Scenario: Borrado confirmado

- **WHEN** el usuario confirma el borrado y la API responde con éxito
- **THEN** el listado SHALL refrescarse sin la quadlet borrada

#### Scenario: Borrado cancelado

- **WHEN** el usuario cancela la confirmación
- **THEN** NO SHALL enviarse petición alguna de borrado
