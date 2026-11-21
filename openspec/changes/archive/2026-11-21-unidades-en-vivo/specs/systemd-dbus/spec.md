# Spec Delta — systemd-dbus

## Purpose

La capa que traduce systemd a dominio. Este delta añade el puerto de notificaciones de cambio de unidades, alimentado por las señales D-Bus de systemd.

## ADDED Requirements

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
