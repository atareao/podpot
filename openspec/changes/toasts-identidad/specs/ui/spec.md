# Spec Delta — ui

## Purpose

El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas. Este delta alinea los avisos de creación, edición y acción con la identidad publicada por la API.

## ADDED Requirements

### Requirement: Avisos con la identidad publicada

El panel SHALL identificar la quadlet en los avisos de creación, edición y acción con el `file_name` publicado por la API, no con el nombre de unidad. El aviso de acción SHALL usar el término «Quadlet» para ser coherente con la identidad mostrada.

#### Scenario: Aviso de creación con la identidad publicada

- **WHEN** una creación termina con éxito
- **THEN** el aviso SHALL identificar la quadlet por su `file_name`

#### Scenario: Aviso de edición con la identidad publicada

- **WHEN** una edición termina con éxito
- **THEN** el aviso SHALL identificar la quadlet por su `file_name`

#### Scenario: Aviso de acción con la identidad publicada

- **WHEN** una acción de control termina con éxito
- **THEN** el aviso SHALL identificar la quadlet por su `file_name`
- **AND** SHALL usar el término «Quadlet»
