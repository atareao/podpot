# systemd-dbus Specification

## Purpose
El cliente D-Bus de podpot contra el manager de systemd del usuario: descubre las unidades quadlet, lee su estado y las controla.

## Requirements

### Requirement: Descubrimiento de quadlets

La aplicación SHALL obtener, por D-Bus contra `org.freedesktop.systemd1`, el conjunto de unidades generadas por quadlets (`UnitFileState` `generated`) de la instancia de usuario, con su nombre.

#### Scenario: Hay quadlets instaladas

- **WHEN** existen unidades quadlet en el manager de usuario
- **THEN** el listado SHALL incluir cada una por su nombre de unidad

#### Scenario: No hay quadlets

- **WHEN** no existe ninguna unidad quadlet
- **THEN** el listado SHALL quedar vacío sin error

### Requirement: Lectura de estado

La aplicación SHALL leer `ActiveState` y `SubState` de cada unidad para informar de su estado.

#### Scenario: Unidad activa

- **WHEN** una unidad quadlet está en ejecución
- **THEN** su estado reportado SHALL corresponder a activa/en ejecución

### Requirement: Control de unidades

La aplicación SHALL poder arrancar, parar y reiniciar una unidad, respetando que las operaciones D-Bus de systemd son **asíncronas** (devuelven un *job*): SHALL esperar el resultado antes de dar la acción por concluida.

#### Scenario: Arrancar una unidad inactiva

- **WHEN** se pide arrancar una unidad inactiva
- **THEN** la operación SHALL completarse y la unidad SHALL quedar activa

#### Scenario: Unidad inexistente

- **WHEN** se pide una acción sobre una unidad que no existe
- **THEN** SHALL devolver un error tipado que nombra la unidad, sin filtrar detalles internos

### Requirement: Nombres de unidad escapados

La aplicación SHALL escapar los nombres de unidad al construir las rutas y argumentos de D-Bus.

#### Scenario: Nombre con caracteres especiales

- **WHEN** el nombre de unidad contiene `.` o `-`
- **THEN** la ruta D-Bus SHALL usar su forma escapada (`_2e`, `_2d`) y la llamada SHALL funcionar

### Requirement: Solo la instancia de usuario

La aplicación SHALL operar exclusivamente contra el manager de systemd del usuario; NUNCA sobre el manager de sistema (root).

#### Scenario: Ámbito de usuario

- **WHEN** la aplicación se conecta al bus
- **THEN** SHALL usar el bus de usuario del host, no el bus del sistema
