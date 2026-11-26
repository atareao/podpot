# Spec Delta — unidades-en-vivo

## Purpose

Contrato de las unidades systemd expuestas al panel. Este delta añade el identificador de stack derivado del fichero quadlet al resumen publicado por la API. Complementa los requisitos existentes del canal SSE sin sustituirlos.

## ADDED Requirements

### Requirement: Resumen de unidad con stack

`UnitSummary` SHALL incluir un campo `stack: String` derivado de forma determinista de `file_name` y `kind`. Para unidades principales (`container`, `pod`, `kube`, `build`, `image`) el stack SHALL ser el nombre base del fichero sin extensión. Para unidades auxiliares (`volume`, `network`) el stack SHALL ser el nombre base sin el sufijo `-volume`/`-network` cuando éste esté presente y no vacíe el nombre; en caso contrario SHALL ser el nombre base completo. Si no hay `file_name` o el tipo no es reconocido, el stack SHALL derivarse del nombre de la unidad sin `.service`. El campo SHALL ser estable ante cambios de `active_state` o `sub_state`.

#### Scenario: Principal y auxiliares comparten stack

- **GIVEN** los ficheros `foo.container`, `foo.volume` y `foo.network`
- **WHEN** se lista `GET /api/v1/units`
- **THEN** `foo.service`, `foo-volume.service` y `foo-network.service` SHALL tener `stack == "foo"`

#### Scenario: Recurso auxiliar independiente

- **GIVEN** el fichero `backup.volume` sin un `backup.container` asociado
- **WHEN** se lista el estado
- **THEN** `backup-volume.service` SHALL tener `stack == "backup"` y formar su propio grupo

#### Scenario: Sufijo en la unidad principal no se recorta

- **GIVEN** el fichero `foo-volume.container`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `foo-volume` porque el tipo `container` manda sobre el sufijo del nombre

#### Scenario: Sin tipo reconocido

- **GIVEN** una unidad sin `file_name` o con `kind == null`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL derivarse del nombre de la unidad sin el sufijo `.service`, sin panic ni valor vacío

#### Scenario: Estabilidad en vivo

- **GIVEN** una unidad con `stack == "foo"`
- **WHEN** cambia su `active_state` por un evento en vivo
- **THEN** su `stack` SHALL permanecer `foo`
