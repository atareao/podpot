# api-shell Specification

## Purpose
El servidor HTTP de podpot: arranque, envoltura JSON única de las respuestas, ruta de salud, manejo de errores y apagado ordenado.

## Requirements

### Requirement: Envoltura JSON única

Todas las rutas de negocio bajo `/api/v1/` SHALL responder con una envoltura JSON uniforme (un campo con los datos y, cuando proceda, un campo de error), también en los errores.

#### Scenario: Respuesta de negocio correcta

- **WHEN** una ruta de negocio devuelve datos
- **THEN** la respuesta SHALL ser `200` con la envoltura JSON y los datos dentro

#### Scenario: Recurso desconocido

- **WHEN** se pide una ruta inexistente
- **THEN** la respuesta SHALL ser `404` con la misma envoltura y un error descriptivo

#### Scenario: Método no permitido

- **WHEN** se usa un método no permitido sobre una ruta existente
- **THEN** la respuesta SHALL ser `405` con la misma envoltura

#### Scenario: Fallbacks preservados en la ruta de creación

- **WHEN** se usa un método distinto de `POST` sobre `/api/v1/quadlets`
- **THEN** la respuesta SHALL ser `405` con la misma envoltura, preservando el fallback existente

### Requirement: Ruta de salud

El servidor SHALL exponer `GET /api/v1/health` sin autenticación, con el estado del proceso.

#### Scenario: Salud disponible

- **WHEN** el proceso está vivo y se llama a `GET /api/v1/health`
- **THEN** SHALL responder `200` con un estado que incluya, al menos, el modo de escucha

### Requirement: Apagado ordenado

El servidor SHALL dejar de aceptar conexiones nuevas y cerrar los recursos ante una señal de terminación.

#### Scenario: Terminación limpia

- **WHEN** el proceso recibe `SIGTERM`
- **THEN** SHALL terminar el servidor y liberar el pool y el cliente D-Bus antes de salir

### Requirement: Ruta de creación de quadlets

El servidor SHALL exponer `POST /api/v1/quadlets` para crear una quadlet de un tipo soportado. La respuesta SHALL usar la envoltura existente `{data}`/`{error}`.

#### Scenario: Creación correcta

- **WHEN** se envía una petición válida de creación de quadlet
- **THEN** SHALL responder `201` (o `200`) con `{data}` conteniendo, al menos, el nombre de unidad generado

#### Scenario: Error de creación envuelto

- **WHEN** la creación falla con un error tipado
- **THEN** la respuesta SHALL usar `{error:{code,message}}` con el estado HTTP correspondiente al tipo de error
