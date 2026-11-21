# Spec Delta — ui

## Purpose

El panel web (React + Ant Design). Este delta especifica el autodesplazamiento condicional del visor de registros (un comportamiento heredado que hasta ahora no estaba escrito), para fijar su contrato y estabilizar su prueba.

## ADDED Requirements

### Requirement: Autodesplazamiento condicional del visor de registros

El visor de registros SHALL autodesplazarse al fondo al llegar líneas nuevas cuando el usuario esté **pegado al fondo** (a una distancia de 24 px o menos respecto del final del contenido). Si el usuario ha subido a leer (distancia mayor de 24 px), el visor NO SHALL desplazarle el scroll al llegar líneas nuevas. Al (re)abrir el visor SHALL partirse de la consideración de «pegado al fondo».

#### Scenario: Llegan líneas estando el usuario pegado al fondo

- **WHEN** llega una línea nueva y el usuario está a 24 px o menos del fondo
- **THEN** el visor SHALL desplazarse al fondo

#### Scenario: El usuario subió a leer

- **WHEN** llega una línea nueva y el usuario está a más de 24 px del fondo
- **THEN** el visor NO SHALL desplazarse

#### Scenario: Umbral delimitado por la distancia al fondo

- **WHEN** la distancia del usuario al fondo está en el límite de 24 px
- **THEN** el visor SHALL considerarse «pegado al fondo» y SHALL desplazarse

#### Scenario: Reapertura del visor

- **WHEN** se vuelve a abrir el visor de una unidad
- **THEN** volverá a considerarse «pegado al fondo» hasta que el usuario se desplace
