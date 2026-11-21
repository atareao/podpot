# Design — creacion-quadlets

## Context

podpot gestiona quadlets de Podman de la instancia **rootless** de systemd. El cambio `bootstrap-core` dejó el panel capaz de **ver y controlar** las quadlets existentes; este cambio añade **crearlas**. Una quadlet **es** una unidad systemd: el estado autoritativo vive en systemd y el fichero de definición vive en el directorio de quadlets del usuario.

`qctl` ilustra la parte de nombres (stem/basename, extensiones reconocidas, colisiones) pero **no** la de escritura: solo crea symlinks hacia el directorio de systemd. Escribir el contenido del fichero es el trabajo nuevo de este cambio.

## Goals / Non-goals

- **Goals**: crear una quadlet de un tipo soportado de forma segura (sin pisar, sin aceptar contenido libre), dejarla visible para systemd y registrarla.
- **Non-goals**: unidades de sistema/root; editar o borrar quadlets; OIDC; SSE/`PropertiesChanged`.

## Decisions

### Directorio de escritura canónico, sin symlinks

La creación escribe el fichero directamente en un **único directorio canónico configurable** (`PODPOT_QUADLET_DIR`, por defecto `$HOME/.config/containers/systemd`), sin factoría de symlinks. Es una **divergencia consciente** de `qctl`, que sí separa origen y enlace. Alternativa considerada y **rechazada**: replicar el patrón fuente+symlink de `qctl`, porque añade estado duplicado (fichero fuente + enlace) y más puntos de fallo (enlace roto, dos sitios que reconciliar) para un v1 que solo crea.

### v1: solo crear

La operación de v1 es **únicamente crear**. No se editan ni se borran quadlets existentes (paridad de alcance con lo ya entregado, que controla pero no modifica definiciones).

### Plantilla determinista por tipo, no contenido libre

El cuerpo del fichero se **renderiza** desde un struct tipado según el tipo (`container`, `volume`, `network`, `pod`). Queda **PROHIBIDO** aceptar contenido libre de fichero desde el cliente: se elimina así la vía de inyección y se mantiene un contrato estable y testeable. Además, los **valores** de los campos de texto SHALL sanitizarse: se rechaza cualquier valor con `\n`, `\r`, `\0` o caracteres de control con un error tipado `InvalidValue { field }` (`invalid_value`, `400`), sin escribir en disco.

### No pisar ficheros existentes

Si ya existe un **fichero regular** con el nombre destino, la creación se **rechaza** sin escribir. Se mantiene la paridad con el comportamiento de `link_or_replace` de `qctl` en su parte de no sobrescribir silenciosamente.

### Recarga de systemd tras escribir

Tras escribir el fichero, se pide a systemd una **recarga (`daemon-reload`)** vía D-Bus y se devuelve el **nombre de unidad generado**. La recarga es la que hace visible la quadlet recién creada.

### Persistencia del registro en nueva migración

Cada creación se persiste en una tabla propia mediante una **nueva migración con prefijo numérico entero** (`0002_*.sql`, no forma Flyway `V001__x.sql`). Es el **primer uso real del pool** de SQLite; se preservan WAL y `foreign_keys` en las opciones de conexión.

## Risks / Trade-offs

- **Colisión de nombres**: el mismo nombre de fichero puede existir ya en el directorio configurado. Mitigación: validar unicidad en la dir canónica configurada (`PODPOT_QUADLET_DIR`), por **nombre de fichero exacto** `<stem>.<kind>`, y **rechazar con error tipado antes de escribir**. Stems iguales con tipos distintos no son colisión porque generan unidades distintas.
- **Sobrescritura accidental**: escribir sobre una definición existente rompería el servicio. Mitigación: rechazo de fichero regular preexistente (no pisar).
- **Estado de systemd no recargado**: la quadlet existe en disco pero systemd no la ve. Mitigación: `daemon-reload` obligatorio tras escribir; si falla, se reporta error tipado distinto del de escritura.
- **Dependencia de la instancia de usuario**: nada de esto debe tocar el manager de sistema. Mitigación: se usa exclusivamente el bus de usuario ya aislado por `systemd-dbus`.

## Migration Plan

1. Añadir la validación de nombre/tipo, la sanitización de valores y la plantilla tipada (dominio, sin efectos).
2. Añadir la escritura sin pisar en `PODPOT_QUADLET_DIR`.
3. Añadir la recarga vía D-Bus y el cálculo del nombre de unidad.
4. Exponer `POST /api/v1/quadlets` con el mapeo de errores a HTTP.
5. Añadir la migración `0002_*.sql` y el registro de cada creación.
6. Añadir el formulario de creación y el refresco del listado.

Sin migración de datos: la migración es aditiva (tabla nueva).

## Open Questions

- ¿Debe la creación soportar más tipos en v2 (p. ej. `kube`, `build`, `image`)? Por ahora v1 cubre `container`, `volume`, `network` y `pod`.
- ¿Conviene ofrecer también el patrón origen+symlink de `qctl` como opción futura? Diferido; no lo necesita v1.
