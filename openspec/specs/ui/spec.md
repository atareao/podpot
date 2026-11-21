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
