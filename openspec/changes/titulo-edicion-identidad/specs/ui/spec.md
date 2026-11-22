# Spec Delta — ui

## Purpose

El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas. Este delta alinea el título de la tarjeta de edición con la identidad publicada por la API.

## MODIFIED Requirements

### Requirement: Edición de quadlets desde el panel

El panel SHALL ofrecer la acción de editar una quadlet existente, reutilizando el formulario de creación precargado con el tipo y los campos de la quadlet seleccionada. El título de la tarjeta de edición SHALL identificar la quadlet por su `file_name` publicado por la API. Al guardar con éxito SHALL refrescar el listado; al fallar SHALL mostrar el `error.message` devuelto por la API.

#### Scenario: Edición correcta

- **WHEN** se guarda una edición válida desde el panel
- **THEN** la API SHALL responder con éxito y el listado SHALL refrescarse

#### Scenario: Título de edición con la identidad publicada

- **WHEN** el panel abre la edición de una quadlet
- **THEN** el título de la tarjeta SHALL identificar la quadlet por su `file_name`
- **AND** NO SHALL usar el nombre de unidad como identidad principal cuando el `file_name` esté disponible

#### Scenario: Error de edición

- **WHEN** la API rechaza una edición
- **THEN** el panel SHALL mostrar el `error.message` devuelto
