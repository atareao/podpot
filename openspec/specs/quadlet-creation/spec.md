# quadlet-creation Specification

## Purpose
La creación de quadlets nuevas desde podpot: validación de nombre y tipo, render de la definición desde una plantilla tipada, escritura en el directorio canónico, recarga de systemd, nombres de unidad generados, errores tipados con su mapeo HTTP y registro de cada creación.

## Requirements

### Requirement: Unicidad del nombre de fichero en el directorio configurado

La aplicación SHALL exigir que el nombre de fichero `<stem>.<kind>` sea único en el directorio de quadlets configurado (`PODPOT_QUADLET_DIR`) antes de escribir nada. Una colisión solo se detecta por **nombre de fichero exacto** y SHALL producir un error tipado `Duplicate` (`duplicate`, `409`) **sin escribir en disco**. Stems iguales con **tipos distintos** (p.ej. `web.container` y `web.volume`) NO SHALL considerarse colisión, porque generan unidades distintas (`web.service` frente a `web-volume.service`).

#### Scenario: Nombre de fichero ya en uso

- **WHEN** se pide crear una quadlet `<stem>.<kind>` y ya existe en el directorio configurado un fichero con ese mismo nombre
- **THEN** SHALL devolver un error tipado `Duplicate` y NUNCA escribir ningún fichero

#### Scenario: Mismo stem con tipo distinto

- **WHEN** existe `web.container` en el directorio configurado y se pide crear `web.volume`
- **THEN** NUNCA SHALL considerarse colisión, porque generan unidades distintas (`web.service` frente a `web-volume.service`), y SHALL continuar

#### Scenario: Nombre libre

- **WHEN** se pide crear una quadlet cuyo nombre de fichero `<stem>.<kind>` no existe en el directorio configurado
- **THEN** SHALL continuar con la validación de tipo y la escritura

### Requirement: Validación de nombre y tipo

La aplicación SHALL validar el charset del stem y SHALL exigir que la extensión esté entre las extensiones de quadlet permitidas para la creación (`container`, `volume`, `network`, `pod`). `.desktop` SHALL quedar excluida explícitamente. Un nombre inválido SHALL producir `InvalidName` y un tipo no permitido SHALL producir `InvalidKind`.

#### Scenario: Caracteres no válidos en el stem

- **WHEN** el stem contiene caracteres fuera del charset permitido
- **THEN** SHALL devolver un error tipado `InvalidName` y NUNCA escribir en disco

#### Scenario: Extensión no permitida

- **WHEN** la extensión no está entre las extensiones de quadlet permitidas
- **THEN** SHALL devolver un error tipado `InvalidKind`

#### Scenario: `.desktop` excluida

- **WHEN** se intenta crear una quadlet con extensión `.desktop`
- **THEN** SHALL devolver un error tipado `InvalidKind`, sin escribir en disco

### Requirement: Render desde plantilla tipada

La aplicación SHALL renderizar el contenido del fichero a partir de un struct tipado según el tipo de quadlet. La aplicación NUNCA SHALL aceptar contenido libre de fichero enviado por el cliente.

#### Scenario: Contenido derivado del tipo

- **WHEN** se crea una quadlet de un tipo soportado con sus campos
- **THEN** el fichero escrito SHALL derivarse de la plantilla tipada de ese tipo, no de texto libre

#### Scenario: Contenido libre rechazado

- **WHEN** la petición incluye un cuerpo de fichero arbitrario en lugar de los campos tipados
- **THEN** la aplicación NUNCA SHALL escribirlo tal cual; SHALL construirlo desde la plantilla del tipo

### Requirement: Sanitización de los valores de los campos

La aplicación SHALL rechazar los valores de los campos de texto que contengan `\n`, `\r`, `\0` o caracteres de control. Un valor inválido SHALL producir un error tipado `InvalidValue { field }` (`invalid_value`, `400`) **sin escribir en disco**. Esta validación de valores es independiente del rechazo de un campo `content` libre.

#### Scenario: Inyección por salto de línea

- **WHEN** un campo de texto (p.ej. `image`) contiene `"nginx\nUser=root"`
- **THEN** SHALL devolver un error tipado `InvalidValue { field: "image" }` (`invalid_value`, `400`) y NUNCA escribir el fichero

#### Scenario: Caracteres de control

- **WHEN** un valor de campo contiene `\r`, `\0` o un carácter de control
- **THEN** SHALL devolver `InvalidValue { field }` (`invalid_value`, `400`), sin escribir en disco

### Requirement: Escritura sin pisar en el directorio configurado

La aplicación SHALL escribir el fichero únicamente en el primer nivel del directorio configurado (`PODPOT_QUADLET_DIR`). Si ya existe un fichero regular con el nombre destino, la creación SHALL rechazarse sin sobrescribirlo.

#### Scenario: Fichero existente

- **WHEN** ya existe un fichero regular con el nombre destino en el directorio configurado
- **THEN** SHALL rechazar la creación con error tipado y NUNCA sobrescribir el fichero

#### Scenario: Escritura correcta

- **WHEN** el nombre es válido, el tipo está permitido y no existe el fichero destino
- **THEN** SHALL escribir el fichero en el primer nivel del directorio configurado

### Requirement: Recarga de systemd y nombre de unidad generado

Tras escribir el fichero, la aplicación SHALL pedir a systemd una recarga (`daemon-reload`) por D-Bus y SHALL devolver el nombre de unidad generado. Para `.container` el nombre de unidad SHALL ser `<stem>.service`; para `volume`, `network` y `pod` SHALL ser `<stem>-<ext>.service`.

#### Scenario: Recarga tras crear

- **WHEN** el fichero se ha escrito correctamente
- **THEN** SHALL invocar `daemon-reload` por D-Bus antes de dar la creación por concluida

#### Scenario: Nombre de unidad de contenedor

- **WHEN** se crea una quadlet `<stem>.container`
- **THEN** el nombre de unidad devuelto SHALL ser `<stem>.service`

#### Scenario: Nombre de unidad de recurso

- **WHEN** se crea una quadlet `<stem>.volume`, `<stem>.network` o `<stem>.pod`
- **THEN** el nombre de unidad devuelto SHALL ser `<stem>-<ext>.service`

### Requirement: Errores tipados y mapeo HTTP

La creación SHALL usar errores tipados y SHALL mapearlos a códigos HTTP de forma estable: `Duplicate` → `409`, `InvalidName`/`InvalidKind`/`InvalidValue` → `400`, `WriteFailed` → `500`, `ReloadFailed`/`UnitRejected` → `502`, `Transport` → `503`.

#### Scenario: Nombre duplicado

- **WHEN** falla la unicidad del nombre
- **THEN** la respuesta SHALL ser `409` con un error tipado `Duplicate`

#### Scenario: Nombre o tipo inválidos

- **WHEN** falla la validación de nombre o de tipo
- **THEN** la respuesta SHALL ser `400` con `InvalidName` o `InvalidKind` respectivamente

#### Scenario: Valor de campo inválido

- **WHEN** un valor de campo contiene `\n`, `\r`, `\0` o un carácter de control
- **THEN** la respuesta SHALL ser `400` con un error tipado `InvalidValue { field }`

#### Scenario: Fallo de escritura

- **WHEN** la escritura del fichero falla por un error de E/S
- **THEN** la respuesta SHALL ser `500` con un error tipado `WriteFailed`

#### Scenario: Fallo de recarga o unidad rechazada

- **WHEN** falla `daemon-reload` o systemd rechaza la unidad resultante
- **THEN** la respuesta SHALL ser `502` con `ReloadFailed` o `UnitRejected`

#### Scenario: Fallo de transporte

- **WHEN** la comunicación por D-Bus no está disponible
- **THEN** la respuesta SHALL ser `503` con un error tipado `Transport`

### Requirement: Registro de la creación

La aplicación SHALL registrar cada creación de quadlet en la persistencia propia.

#### Scenario: Creación registrada

- **WHEN** una quadlet se crea correctamente
- **THEN** SHALL quedar registrada en la persistencia con, al menos, su nombre de unidad y de fichero y el momento de la creación

#### Scenario: Creación fallida no registrada

- **WHEN** la creación falla antes de completarse
- **THEN** NUNCA SHALL quedar un registro de una creación que no ocurrió
