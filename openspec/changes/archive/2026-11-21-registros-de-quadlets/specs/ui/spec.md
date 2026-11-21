# Spec Delta — ui

## Purpose

El panel web (React + Ant Design). Este delta añade el visor de logs de una quadlet.

## ADDED Requirements

### Requirement: Visor de logs de una quadlet

El panel SHALL permitir abrir un visor de logs para una unidad desde el listado. Al abrirlo SHALL suscribirse al canal SSE de logs de esa unidad y mostrar las líneas recibidas, distinguiendo su origen cuando proceda. El visor SHALL mantenerse al día con las líneas nuevas mientras esté abierto. Al cerrarlo SHALL darse de baja del canal. Si el canal no está disponible, el visor SHALL comunicar el error sin romper el resto del panel.

#### Scenario: Abrir el visor

- **WHEN** el usuario abre los logs de una unidad
- **THEN** SHALL mostrarse las líneas recibidas por el canal de esa unidad

#### Scenario: Líneas nuevas en vivo

- **WHEN** llega una línea nueva mientras el visor está abierto
- **THEN** SHALL añadirse a lo mostrado

#### Scenario: Cerrar el visor

- **WHEN** el usuario cierra el visor
- **THEN** SHALL darse de baja del canal

#### Scenario: Canal no disponible

- **WHEN** el canal de logs no puede establecerse o falla
- **THEN** el visor SHALL mostrar el error sin afectar al resto del panel
