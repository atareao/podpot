# Spec Delta — persistencia

## Purpose

La persistencia propia de podpot (SQLite vía `sqlx`). Este delta documenta que el fallo al escribir en la base de datos durante el ciclo de vida de una quadlet se representa con una única variante tipada de dominio, mapeada a un código HTTP estable.

## ADDED Requirements

### Requirement: Fallo de persistencia tipado y unificado

Todo fallo al escribir en la base de datos durante el ciclo de vida de una quadlet (creación, edición o borrado) SHALL representarse con **una única** variante tipada de dominio, que agrupa el nombre de fichero afectado y la causa subyacente. NUNCA SHALL existir más de una variante para el mismo desenlace. Ese fallo SHALL mapearse al código `internal` (`500`) con la envoltura existente `{error:{code,message}}`, sin exponer la causa subyacente en el cuerpo.

#### Scenario: Fallo de persistencia en creación, edición o borrado

- **WHEN** la escritura en la base de datos falla durante una creación, una edición o un borrado
- **THEN** la respuesta SHALL ser `500` con el código `internal` y la envoltura existente
- **AND** la causa subyacente NO SHALL exponerse en el cuerpo de la respuesta

#### Scenario: Una sola variante para el mismo desenlace

- **WHEN** se inspecciona el tipo de error de dominio de las quadlets
- **THEN** SHALL existir exactamente una variante para el fallo de persistencia, compartida por creación, edición y borrado
