# Spec Delta — config

## Purpose

La configuración de podpot leída del entorno, con sus reglas de arranque y valores por defecto.

## ADDED Requirements

### Requirement: Variables del entorno

La configuración SHALL leerse del entorno. Los nombres sin prefijo son `BIND_ADDRESS`, `DATABASE_URL` y `RUST_LOG`; el resto SHALL llevar prefijo `PODPOT_`.

#### Scenario: Valor por defecto del bind

- **WHEN** `BIND_ADDRESS` no está definida
- **THEN** la dirección de escucha SHALL ser `127.0.0.1:8080`

#### Scenario: Bind vacío no cae al defecto

- **WHEN** `BIND_ADDRESS` está definida pero vacía
- **THEN** el arranque SHALL fallar nombrando la variable, sin recurrir al valor por defecto

#### Scenario: Valor malformado

- **WHEN** `BIND_ADDRESS` no es una dirección `host:puerto` válida
- **THEN** el arranque SHALL fallar nombrando la variable

### Requirement: Requisitos de arranque

Las variables obligatorias SHALL exigirse al arrancar; su ausencia SHALL ser un error que nombra la variable, no un modo degradado.

#### Scenario: Falta la base de datos

- **WHEN** `DATABASE_URL` no está definida
- **THEN** el arranque SHALL fallar nombrando `DATABASE_URL`
