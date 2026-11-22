# Spec Delta — ui

## Purpose

El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas. Este delta alinea la confirmación de borrado con la identidad publicada por la API y precisa el contrato de identidad usado por la UI y sus pruebas.

## MODIFIED Requirements

### Requirement: Borrado de quadlets desde el panel

El panel SHALL ofrecer la acción de borrar una quadlet existente, solicitando confirmación antes de ejecutarla (acción destructiva). La confirmación SHALL identificar la quadlet con la misma identidad que la fila y los nombres accesibles, es decir, el `file_name` publicado por la API. Al confirmar con éxito SHALL refrescar el listado; al fallar SHALL mostrar el `error.message` devuelto por la API.

#### Scenario: Confirmación antes de borrar

- **WHEN** se pulsa borrar sobre una quadlet
- **THEN** el panel SHALL pedir confirmación antes de enviar la petición

#### Scenario: Confirmación con la identidad publicada

- **WHEN** el panel muestra la confirmación de borrado de una quadlet
- **THEN** el título y la descripción SHALL identificar la quadlet por su `file_name`
- **AND** NO SHALL usar el nombre de unidad como identidad principal cuando el `file_name` esté disponible

#### Scenario: Borrado confirmado

- **WHEN** el usuario confirma el borrado y la API responde con éxito
- **THEN** el listado SHALL refrescarse sin la quadlet borrada

#### Scenario: Borrado cancelado

- **WHEN** el usuario cancela la confirmación
- **THEN** NO SHALL enviarse petición alguna de borrado

### Requirement: Identidad de quadlet para las acciones

El panel SHALL usar el `file_name` que publica la API como identidad de la quadlet para las acciones de editar y borrar. El panel NO SHALL deducir el nombre de fichero invirtiendo el nombre de unidad, porque esa regla inversa no es inyectiva y puede apuntar al fichero equivocado. El contrato de la API SHALL tratar `file_name` como cadena no nula; la UI SHALL limitarse a un fallback defensivo ante cadena vacía.

#### Scenario: Editar y borrar usan la identidad publicada

- **WHEN** el usuario edita o borra una quadlet del listado
- **THEN** el panel SHALL operar sobre el `file_name` recibido de la API para esa quadlet

#### Scenario: Dos quadlets con nombres de unidad colisionados

- **WHEN** el listado contiene dos quadlets cuyos nombres de unidad no permiten distinguirlas (p. ej. `web-volume.service`)
- **THEN** el panel SHALL operar sobre el `file_name` correcto de cada una, sin confundirlas

#### Scenario: Fallback defensivo ante identidad vacía

- **WHEN** una quadlet llega con `file_name` vacío
- **THEN** la UI SHALL usar el nombre de unidad como identidad mostrada y accesible
- **AND** NO SHALL renderizar el literal `null`
