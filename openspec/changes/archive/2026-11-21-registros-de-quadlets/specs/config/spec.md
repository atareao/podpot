# Spec Delta — config

## Purpose

La configuración de podpot leída del entorno. Este delta añade la variable del socket de Podman para la lectura de logs.

## ADDED Requirements

### Requirement: Socket de Podman

La configuración SHALL reconocer `PODPOT_PODMAN_SOCKET`, que indica la ruta del socket de la API de Podman para leer los logs de los contenedores. Cuando no esté definida SHALL usar por defecto el socket rootless del usuario (`$XDG_RUNTIME_DIR/podman/podman.sock`).

#### Scenario: Valor por defecto

- **WHEN** `PODPOT_PODMAN_SOCKET` no está definida
- **THEN** la ruta del socket SHALL ser `$XDG_RUNTIME_DIR/podman/podman.sock`

#### Scenario: Valor explícito

- **WHEN** `PODPOT_PODMAN_SOCKET` apunta a un socket
- **THEN** la aplicación SHALL usarlo para la API de Podman
