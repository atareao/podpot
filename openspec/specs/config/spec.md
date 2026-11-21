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

### Requirement: Variables de autenticación

La configuración SHALL reconocer las variables del proveedor OIDC `PODPOT_OIDC_ISSUER_URL`, `PODPOT_OIDC_CLIENT_ID`, `PODPOT_OIDC_CLIENT_SECRET` y `PODPOT_OIDC_REDIRECT_URL` (esta última con valor por defecto), y la variable del secreto de firma de sesión `PODPOT_SECRET`. La presencia de las tres primeras SHALL determinar el modo `oidc`; su ausencia total, el modo `disabled`. Una definición parcial SHALL ser un error de arranque.

#### Scenario: Las tres definidas activan OIDC

- **WHEN** `PODPOT_OIDC_ISSUER_URL`, `PODPOT_OIDC_CLIENT_ID` y `PODPOT_OIDC_CLIENT_SECRET` están definidas
- **THEN** el modo de autenticación SHALL ser `oidc` y SHALL exigirse `PODPOT_SECRET` para firmar la sesión

#### Scenario: Ninguna definida

- **WHEN** ninguna de las tres variables OIDC está definida
- **THEN** el modo de autenticación SHALL ser `disabled` y la aplicación SHALL arrancar sin exigir `PODPOT_SECRET`

#### Scenario: Definición parcial

- **WHEN** se define alguna de las tres variables OIDC pero no todas
- **THEN** el arranque SHALL fallar nombrando la variable que falta

#### Scenario: Redirect por defecto

- **WHEN** `PODPOT_OIDC_REDIRECT_URL` no está definida y el modo es `oidc`
- **THEN** SHALL usarse un valor por defecto coherente con el bind local

#### Scenario: Falta el secreto en modo OIDC

- **WHEN** el modo es `oidc` y `PODPOT_SECRET` no está definida
- **THEN** el arranque SHALL fallar nombrando `PODPOT_SECRET`
