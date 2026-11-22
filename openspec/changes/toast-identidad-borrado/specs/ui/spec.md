# Spec Delta — ui

## Purpose

El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas. Este delta alinea el aviso de éxito del borrado con la identidad publicada por la API.

## MODIFIED Requirements

### Requirement: Borrado de quadlets desde el panel

El panel SHALL ofrecer la acción de borrar una quadlet existente, solicitando confirmación antes de ejecutarla (acción destructiva). La confirmación y el aviso de éxito SHALL identificar la quadlet con la misma identidad que la fila y los nombres accesibles, es decir, el `file_name` publicado por la API. Al confirmar con éxito SHALL refrescar el listado; al fallar SHALL mostrar el `error.message` devuelto por la API.

#### Scenario: Confirmación antes de borrar

- **WHEN** se pulsa borrar sobre una quadlet
- **THEN** el panel SHALL pedir confirmación antes de enviar la petición

#### Scenario: Confirmación con la identidad publicada

- **WHEN** el panel muestra la confirmación de borrado de una quadlet
- **THEN** el título y la descripción SHALL identificar la quadlet por su `file_name`
- **AND** NO SHALL usar el nombre de unidad como identidad principal cuando el `file_name` esté disponible

#### Scenario: Aviso de borrado con la identidad publicada

- **WHEN** el borrado se completa con éxito
- **THEN** el aviso SHALL identificar la quadlet por su `file_name`
- **AND** NO SHALL usar el nombre de unidad como identidad principal cuando el `file_name` esté disponible

#### Scenario: Borrado confirmado

- **WHEN** el usuario confirma el borrado y la API responde con éxito
- **THEN** el listado SHALL refrescarse sin la quadlet borrada

#### Scenario: Borrado cancelado

- **WHEN** el usuario cancela la confirmación
- **THEN** NO SHALL enviarse petición alguna de borrado

#### Scenario: Fallback defensivo ante identidad vacía

- **WHEN** una quadlet llega con `file_name` vacío
- **THEN** la confirmación y el aviso SHALL usar el nombre de unidad como identidad
- **AND** NO SHALL renderizar el literal `null`
