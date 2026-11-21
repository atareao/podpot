# Design — edicion-borrado-quadlets

## Contexto

La creación de quadlets (`quadlet-creation`) fija el contrato tipado: el cliente envía campos, el servidor renderiza el contenido desde un struct y escribe sin pisar. La edición y el borrado deben reutilizar ese contrato y no abrir una vía de contenido libre.

## Decisiones

### Identidad: el nombre de fichero es la clave

La quadlet se identifica por el nombre de fichero completo `<stem>.<kind>` (p. ej. `web.container`), enviado como segmento `{name}` de la URL (codificado). No se acepta el nombre de unidad (`web.service`) como identificador, para no depender del mapeo de nombres ni de ambigüedades entre tipos.

### Sin renombrado

Editar NUNCA renombra. El stem y la extensión son la identidad; un intento de cambio de nombre se rechaza con `InvalidName` (`400`). Evita colisiones, symlinks rotos y registros huérfanos. El renombrado queda fuera de alcance.

### `PATCH` como actualización completa del struct tipado

Aunque el verbo sea `PATCH`, la semántica es la de reemplazar el conjunto de campos del tipo: se exige el struct tipado completo del `kind` (mismo contrato que la creación, con `deny_unknown_fields`), se renderiza y se reescribe el fichero de forma atómica (fichero temporal + `rename`). No hay fusión campo a campo, para mantener un único camino de render y validación. `PUT` no se expone para evitar dos verbos con idéntico comportamiento.

### Validación antes de tocar el disco

Se reutiliza la sanitización de valores de la creación: `\n`, `\r`, `\0` y controles producen `InvalidValue { field }` (`400`) y el fichero original queda intacto. La existencia de la quadlet se comprueba antes de renderizar (`NotFound` → `404`).

### Borrado con recarga

`DELETE` elimina el fichero del primer nivel de `PODPOT_QUADLET_DIR` y pide `daemon-reload` por D-Bus. Si el fichero no existe → `NotFound` (`404`) sin efectos. El borrado no intenta parar la unidad previamente: eso es responsabilidad del usuario vía las acciones de `units`; se documenta como follow-up.

### Recarga por D-Bus

Tanto la edición como el borrado terminan con `daemon-reload` a través del trait `SystemdClient` existente (mismo mecanismo que la creación). Fallos de recarga → `ReloadFailed`/`UnitRejected` (`502`) o `Transport` (`503`).

### Persistencia

Nueva migración `0003_*.sql` (prefijo numérico entero) que introduce una tabla de operaciones con el tipo de operación (`edit`/`delete`), nombre de fichero y marca temporal. Se preservan WAL y `foreign_keys` en las opciones del pool. Solo se registran operaciones completadas con éxito.

## Alternativas descartadas

- **`PUT` además de `PATCH`**: dos verbos, un solo comportamiento; ruido en el contrato.
- **Fusión parcial de campos**: multiplica los caminos de render/validación sin aportar valor en quadlets pequeñas.
- **Contenido libre del cliente**: rompe la garantía de plantilla tipada y reintroduce la superficie de inyección que la creación ya cerró.
- **Renombrado de quadlets**: colisiones y unidades huérfanas; fuera de alcance.

## Impacto en specs

- Nueva capacidad `quadlet-edition`.
- Deltas en `api-shell` (rutas), `ui` (acciones), `persistencia` (registro).
