# Spec Delta — ui

## Purpose

Listado de unidades del panel. Este delta añade la agrupación por stack y los filtros secundarios, conservando íntegramente el requisito base «Listado de quadlets» (identidad `file_name`, estado y nombres accesibles de las acciones).

## ADDED Requirements

### Requirement: Listado agrupado por stack

Además de la identidad y el estado definidos en «Listado de quadlets», la tabla de unidades SHALL mostrar un grupo por cada `stack` distinto, con una cabecera que identifique el stack y, dentro, una fila por unidad con su estado y acciones. Los grupos SHALL ordenarse por nombre de stack de forma case-insensitive; las unidades dentro de un grupo SHALL ordenarse de forma determinista (principales antes que auxiliares, luego por nombre). La vista sin filtros SHALL mostrar todas las unidades. La cabecera del grupo SHALL ser informativa y SHALL NO sustituir la identidad `file_name` ni las acciones de cada fila.

#### Scenario: Unidades del mismo stack juntas

- **GIVEN** tres unidades con `stack == "foo"` y una con `stack == "bar"`
- **WHEN** se renderiza el listado
- **THEN** SHALL aparecer dos grupos, `bar` antes que `foo`, y las tres unidades de `foo` dentro del mismo grupo

#### Scenario: Estado visible por unidad

- **GIVEN** un grupo con una unidad activa y otra parada
- **WHEN** se renderiza el grupo
- **THEN** cada fila SHALL mostrar su propio estado y sus acciones sin que la cabecera del grupo sustituya esa información

#### Scenario: Actualización en vivo sin reordenar

- **GIVEN** el listado agrupado visible
- **WHEN** llega un cambio de estado de una unidad
- **THEN** la unidad SHALL permanecer en su grupo y posición determinista, actualizándose solo su estado

### Requirement: Filtros secundarios por tipo y estado

El listado SHALL ofrecer filtros por tipo de unidad (`kind`) y por estado (`active_state`), combinables entre sí y con la agrupación por stack.

#### Scenario: Filtro por tipo

- **WHEN** el usuario selecciona el tipo `container`
- **THEN** SHALL mostrarse solo las unidades de ese tipo, agrupadas por sus stacks

#### Scenario: Filtros combinados

- **WHEN** el usuario selecciona tipo `volume` y estado `inactive`
- **THEN** SHALL mostrarse solo las unidades que cumplan ambas condiciones

#### Scenario: Sin resultados

- **WHEN** ninguna unidad cumple los filtros
- **THEN** SHALL mostrarse un estado vacío explícito, sin errores ni grupos vacíos
