# Spec Delta — api-shell

## Purpose

El servidor HTTP de podpot: arranque, envoltura JSON única de las respuestas, ruta de salud, manejo de errores y apagado ordenado. Este delta añade el requirement del listado de unidades, hoy ausente de la spec, y fija que el listado publica la identidad real (nombre de fichero) de cada quadlet sin publicar su ruta.

## ADDED Requirements

### Requirement: Ruta de listado de unidades

El servidor SHALL exponer `GET /api/v1/units` devolviendo las quadlets del usuario con su estado. La respuesta SHALL usar la envoltura existente `{data}`/`{error}`. Por cada quadlet SHALL publicar, además de su nombre de unidad y sus estados, su **nombre de fichero** (`file_name`), que es la identidad con la que operan `PATCH`/`DELETE /api/v1/quadlets/{name}`. NUNCA SHALL publicar la ruta del sistema de ficheros de la que procede la unidad.

#### Scenario: Listado con la identidad real de cada quadlet

- **WHEN** se llama a `GET /api/v1/units` y hay quadlets instaladas
- **THEN** SHALL responder `200` con `{data}` conteniendo, por cada quadlet, al menos su `name`, `active_state`, `sub_state`, `unit_file_state` y su `file_name` (el basename del fichero de la quadlet, p. ej. `web.container`)

#### Scenario: La ruta no se publica

- **WHEN** se inspecciona cualquier unidad del listado
- **THEN** SHALL contener el nombre de fichero pero NO la ruta del sistema de ficheros de la que systemd la generó

#### Scenario: Sin quadlets

- **WHEN** el usuario no tiene quadlets instaladas
- **THEN** SHALL responder `200` con `{data}` como array vacío
