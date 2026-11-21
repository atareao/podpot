# config Specification

## Purpose
La configuración de podpot leída del entorno, con sus reglas de arranque y valores por defecto.

## Requirements

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

### Requirement: Servicio del panel

La aplicación SHALL servir el panel estático desde el directorio indicado por `PODPOT_STATIC_DIR` cuando esté definido y exista, con reserva a `index.html` para rutas que no son de la API (SPA). Si no está definido, la aplicación SHALL funcionar solo como API.

#### Scenario: Panel en la raíz

- **WHEN** `PODPOT_STATIC_DIR` apunta al build del panel y se pide `/`
- **THEN** SHALL responder con el `index.html` del panel

#### Scenario: Ruta SPA desconocida

- **WHEN** se pide una ruta que no es de la API y no corresponde a un fichero
- **THEN** SHALL responder el `index.html` del panel (reserva SPA), no el `404` de la envoltura

#### Scenario: Sin panel configurado

- **WHEN** `PODPOT_STATIC_DIR` no está definido
- **THEN** la aplicación SHALL servir solo `/api/v1/*` y el arranque SHALL seguir siendo correcto
