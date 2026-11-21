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

### Requirement: Nombre de fichero derivado de la procedencia

El estado de una unidad SHALL exponer el **nombre de fichero** de la quadlet derivado de su procedencia (`SourcePath`): el basename de la ruta, que es el nombre de fichero `<stem>.<kind>` (p. ej. `web.container`). El dato SHALL ser un accesor del dominio, testeable con el doble, y SHALL devolver «sin nombre» cuando la procedencia es desconocida (unidad transitoria, que no es quadlet).

#### Scenario: Quadlet con fichero

- **WHEN** una unidad proviene de un fichero de quadlet (p. ej. `SourcePath` termina en `/web.container`)
- **THEN** su nombre de fichero SHALL ser el basename de esa ruta (`web.container`)

#### Scenario: Procedencia desconocida

- **WHEN** la procedencia de una unidad es desconocida (unidad transitoria, sin fichero)
- **THEN** el nombre de fichero SHALL ser «sin nombre» (ausente), y la unidad NO SHALL considerarse quadlet

### Requirement: Vigilante de cambios de unidades

La capa de systemd SHALL exponer un puerto de **notificaciones de cambio** de unidades que avise cuando el conjunto o el estado de las unidades pueda haber cambiado, sin transportar datos de unidad. La implementación real SHALL alimentarse de las señales que systemd emite por D-Bus (cambios de propiedades de las unidades y alta/baja de unidades), y SHALL habilitar la emisión de esas señales antes de escucharlas. Las notificaciones SHALL coalescerse para no avisar en ráfaga. El doble de pruebas SHALL poder emitir notificaciones sin bus real.

#### Scenario: Señal de cambio reenviada como notificación

- **WHEN** systemd emite por D-Bus una señal de cambio de una unidad
- **THEN** el vigilante SHALL emitir una notificación de cambio a sus suscriptores

#### Scenario: Alta o baja de unidad

- **WHEN** aparece o desaparece una unidad (alta/baja)
- **THEN** el vigilante SHALL emitir una notificación de cambio

#### Scenario: Notificaciones coalescidas

- **WHEN** llegan varias señales en un intervalo corto
- **THEN** el vigilante SHALL coalescerlas en una sola notificación

#### Scenario: El doble emite notificaciones sin bus

- **WHEN** una prueba usa el doble de systemd y este emite una notificación
- **THEN** los suscriptores del puerto SHALL recibirla, sin necesidad de un bus de sesión real

#### Scenario: El bus cierra el stream de señales

- **WHEN** el bus de systemd cierra el stream de señales (el bus se cae o la conexión se corta)
- **THEN** el vigilante SHALL registrarlo (un aviso en el log) y SHALL reintentar la suscripción con una espera creciente (backoff), de modo que la pérdida del canal no quede invisible ni sea permanente
