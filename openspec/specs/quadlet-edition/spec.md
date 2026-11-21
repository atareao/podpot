# quadlet-edition Specification

## Purpose
La edición y el borrado de quadlets existentes desde podpot: identidad por nombre de fichero, reutilización del render tipado, validación y sanitización de valores, inmutabilidad del nombre, recarga de systemd por D-Bus, errores tipados con su mapeo HTTP y registro de cada operación.

## Requirements

### Requirement: Edición de una quadlet existente

La aplicación SHALL permitir actualizar una quadlet existente identificada por su nombre de fichero `<stem>.<kind>`, reescribiendo su fichero en el primer nivel del directorio configurado (`PODPOT_QUADLET_DIR`). El contenido SHALL renderizarse desde el struct tipado del tipo; la aplicación NUNCA SHALL aceptar contenido libre de fichero enviado por el cliente. La reescritura SHALL ser atómica (fichero temporal + `rename`). Si el fichero destino no existe, SHALL producirse `NotFound` (`not_found`, `404`) sin escribir nada.

#### Scenario: Edición correcta

- **WHEN** se envía una edición válida de una quadlet existente
- **THEN** el fichero SHALL reescribirse con el contenido renderizado desde el struct tipado
- **AND** SHALL solicitarse `daemon-reload` por D-Bus
- **AND** la operación SHALL registrarse

#### Scenario: Quadlet inexistente

- **WHEN** se edita un nombre de fichero que no existe en el directorio configurado
- **THEN** la respuesta SHALL ser `NotFound` (`not_found`, `404`)
- **AND** el directorio SHALL quedar intacto

### Requirement: Borrado de una quadlet existente

La aplicación SHALL permitir borrar una quadlet existente eliminando su fichero del primer nivel del directorio configurado (`PODPOT_QUADLET_DIR`). Tras el borrado SHALL solicitar `daemon-reload` por D-Bus y SHALL registrar la operación. Si el fichero no existe, SHALL producirse `NotFound` (`404`) sin efectos.

#### Scenario: Borrado correcto

- **WHEN** se borra una quadlet existente
- **THEN** su fichero SHALL desaparecer del directorio configurado
- **AND** SHALL solicitarse `daemon-reload` por D-Bus
- **AND** la operación SHALL registrarse

#### Scenario: Borrado de algo inexistente

- **WHEN** se borra un nombre de fichero que no existe en el directorio configurado
- **THEN** la respuesta SHALL ser `NotFound` (`not_found`, `404`) sin efectos

### Requirement: Validación y sanitización en edición

La edición SHALL aplicar las mismas reglas de validación y sanitización que la creación: rechazo de valores de campo que contengan `\n`, `\r`, `\0` o caracteres de control, con un error tipado `InvalidValue { field }` (`invalid_value`, `400`) y sin modificar el fichero. Un tipo (`kind`) no permitido SHALL producir `InvalidKind` (`400`).

#### Scenario: Valor con caracteres prohibidos

- **WHEN** se edita con un valor que contiene `\n`, `\r`, `\0` o un control
- **THEN** la respuesta SHALL ser `InvalidValue` (`invalid_value`, `400`)
- **AND** el fichero original SHALL quedar intacto

### Requirement: Inmutabilidad del nombre en la edición

La identidad de una quadlet SHALL ser su nombre de fichero `<stem>.<kind>`. La aplicación NUNCA SHALL renombrar una quadlet durante una edición. Un intento de cambiar el nombre con respecto al recurso indicado SHALL rechazarse con `InvalidName` (`invalid_name`, `400`).

#### Scenario: Intento de renombrado

- **WHEN** una edición pretende cambiar el nombre de fichero del recurso indicado
- **THEN** la respuesta SHALL ser `InvalidName` (`invalid_name`, `400`)
- **AND** SHALL descartarse la operación

### Requirement: Mapeo de errores de edición y borrado

La edición y el borrado SHALL usar errores tipados y SHALL mapearlos a códigos HTTP de forma estable y coincidente con la creación: `NotFound` → `404`, `InvalidName`/`InvalidKind`/`InvalidValue` → `400`, `WriteFailed` → `500`, `ReloadFailed`/`UnitRejected` → `502`, `Transport` → `503`. La respuesta SHALL usar la envoltura existente `{data}`/`{error:{code,message}}`.

#### Scenario: Error de escritura

- **WHEN** la reescritura o el borrado del fichero falla por causas del sistema de ficheros
- **THEN** la respuesta SHALL ser `WriteFailed` (`write_failed`, `500`) con la envoltura existente

#### Scenario: Error de recarga

- **WHEN** la operación completa pero `daemon-reload` falla
- **THEN** la respuesta SHALL ser `ReloadFailed`/`UnitRejected` (`502`) o `Transport` (`503`) según el tipo

### Requirement: Registro de edición y borrado

La aplicación SHALL registrar cada edición y cada borrado completados con éxito en la persistencia propia. Las operaciones fallidas NUNCA SHALL registrarse.

#### Scenario: Edición registrada

- **WHEN** una edición termina con éxito
- **THEN** SHALL escribirse un registro de operación de tipo edición para el nombre de fichero afectado

#### Scenario: Operación fallida no registrada

- **WHEN** una edición o un borrado falla antes de completarse
- **THEN** NO SHALL quedar registro de esa operación
