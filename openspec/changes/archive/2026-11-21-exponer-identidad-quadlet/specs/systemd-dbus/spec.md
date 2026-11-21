# Spec Delta — systemd-dbus

## Purpose

La capa que traduce systemd a dominio. Este delta añade la exposición del nombre de fichero de la quadlet como dato derivado del `SourcePath`, para que el listado pueda publicar la identidad real sin que la capa API recalcule nada.

## ADDED Requirements

### Requirement: Nombre de fichero derivado de la procedencia

El estado de una unidad SHALL exponer el **nombre de fichero** de la quadlet derivado de su procedencia (`SourcePath`): el basename de la ruta, que es el nombre de fichero `<stem>.<kind>` (p. ej. `web.container`). El dato SHALL ser un accesor del dominio, testeable con el doble, y SHALL devolver «sin nombre» cuando la procedencia es desconocida (unidad transitoria, que no es quadlet).

#### Scenario: Quadlet con fichero

- **WHEN** una unidad proviene de un fichero de quadlet (p. ej. `SourcePath` termina en `/web.container`)
- **THEN** su nombre de fichero SHALL ser el basename de esa ruta (`web.container`)

#### Scenario: Procedencia desconocida

- **WHEN** la procedencia de una unidad es desconocida (unidad transitoria, sin fichero)
- **THEN** el nombre de fichero SHALL ser «sin nombre» (ausente), y la unidad NO SHALL considerarse quadlet
