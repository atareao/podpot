# Spec Delta — config

## Purpose

La configuración de podpot leída del entorno, con sus reglas de arranque y valores por defecto. Este delta añade el directorio de escritura de quadlets.

## ADDED Requirements

### Requirement: Directorio de quadlets

La configuración SHALL incluir la clave `PODPOT_QUADLET_DIR`, que indica el directorio canónico donde se escriben las quadlets nuevas. Cuando no esté definida, SHALL usar por defecto `$HOME/.config/containers/systemd`. El valor SHALL validarse como un directorio.

#### Scenario: Valor por defecto

- **WHEN** `PODPOT_QUADLET_DIR` no está definida
- **THEN** el directorio de quadlets SHALL ser `$HOME/.config/containers/systemd`

#### Scenario: Valor no es un directorio

- **WHEN** `PODPOT_QUADLET_DIR` apunta a una ruta que no es un directorio
- **THEN** el arranque SHALL fallar nombrando `PODPOT_QUADLET_DIR`

#### Scenario: Directorio válido

- **WHEN** `PODPOT_QUADLET_DIR` apunta a un directorio existente
- **THEN** la aplicación SHALL usarlo como destino de escritura de las quadlets nuevas
