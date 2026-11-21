# Tasks — edicion-borrado-quadlets

TDD por bloques: primero las pruebas (rojo), luego la implementación mínima (verde), luego refactor. Cada tarea indica su criterio de verificación. Orden por dependencia: dominio → escritura → D-Bus → persistencia → API → UI → cierre.

## 1. Dominio: identidad, validación y render reutilizado

- [x] 1.1 Identidad por nombre de fichero `<stem>.<kind>` y `NotFound` si no existe en `PODPOT_QUADLET_DIR`. — Verificación: test rojo→verde que devuelve `NotFound` para un nombre inexistente.
- [x] 1.2 Reutilización del struct tipado por `kind` para renderizar el contenido en edición; rechazo de contenido libre. — Verificación: test que confirma que el contrato no acepta cuerpo de fichero arbitrario.
- [x] 1.3 Sanitización de valores idéntica a la creación (`\n`, `\r`, `\0` y controles → `InvalidValue { field }`) sin tocar el fichero. — Verificación: test de inyección que devuelve `InvalidValue` y comprueba que el fichero queda intacto.
- [x] 1.4 Inmutabilidad del nombre: un intento de renombrar produce `InvalidName`. — Verificación: test que confirma `InvalidName` y descarta la operación.

## 2. Escritura atómica de la edición

- [x] 2.1 Reescritura atómica (fichero temporal + `rename`) en el primer nivel de `PODPOT_QUADLET_DIR`. — Verificación: test con directorio temporal que comprueba contenido final y ausencia de temporales.
- [x] 2.2 Fallo de escritura mapeado a `WriteFailed`. — Verificación: test con directorio de solo lectura o ruta no escribible que devuelve `WriteFailed` sin dejar el original corrupto.

## 3. Borrado

- [x] 3.1 Borrado del fichero en el primer nivel de `PODPOT_QUADLET_DIR`. — Verificación: test con directorio temporal que comprueba que el fichero desaparece.
- [x] 3.2 Borrado de inexistente → `NotFound` sin efectos. — Verificación: test que devuelve `NotFound` y comprueba que el directorio queda intacto.

## 4. Recarga D-Bus

- [x] 4.1 Recarga (`daemon-reload`) tras editar y tras borrar, reutilizando el trait `SystemdClient` y su doble de pruebas. — Verificación: test con doble que verifica la llamada de recarga en ambas operaciones.
- [x] 4.2 Orquestación y mapeo `ReloadFailed`/`UnitRejected`/`Transport`. — Verificación: tests rojo→verde con doble en fallo de recarga y de transporte.

## 5. Persistencia

- [x] 5.1 Migración `0003_*.sql` (prefijo entero) con la tabla de operaciones (tipo, nombre de fichero, marca temporal). — Verificación: `cargo test` contra base vacía que comprueba que la tabla existe tras migrar.
- [x] 5.2 Registro de edición y de borrado por el pool, preservando WAL y `foreign_keys`. — Verificación: test que escribe registros y comprueba `PRAGMA foreign_keys=1` y el modo WAL.
- [x] 5.3 No registrar operaciones fallidas. — Verificación: test que fuerza un fallo previo a completar y comprueba que no queda registro.

## 6. API

- [x] 6.1 `PATCH /api/v1/quadlets/{name}` con la envoltura `{data}`/`{error}`. — Verificación: test de integración de la ruta con respuesta correcta.
- [x] 6.2 `DELETE /api/v1/quadlets/{name}` con la envoltura `{data}`/`{error}`. — Verificación: test de integración de la ruta con respuesta correcta.
- [x] 6.3 Mapeo de errores tipados a HTTP: `NotFound`→404, `InvalidName`/`InvalidKind`/`InvalidValue`→400, `WriteFailed`→500, `ReloadFailed`/`UnitRejected`→502, `Transport`→503. — Verificación: test de tabla de mapeo en ambas rutas.
- [x] 6.4 Preservar los fallbacks `404`/`405` con envoltura, incluidas las nuevas rutas. — Verificación: tests de `405` sobre `/api/v1/quadlets/{name}` y de `404` de ruta inexistente.
- [x] 6.5 `GET /api/v1/quadlets/{name}` que devuelve tipo y campos de una quadlet existente; `NotFound`→404 si no existe. — Verificación: test de integración de la ruta (correcto e inexistente).

## 7. UI

- [x] 7.1 Acción de editar reutilizando el formulario de creación precargado con el tipo y los campos de la quadlet. — Verificación: test de componente con `@testing-library/react` que guarda y refresca el listado.
- [x] 7.2 Acción de borrar con confirmación (Ant Design) y refresco tras el éxito. — Verificación: test de componente que confirma el borrado y comprueba el refresco.
- [x] 7.3 Mostrar `error.message` de la API en fallo de edición y de borrado. — Verificación: test de componente con respuesta de error simulada.
- [x] 7.4 Precarga real de los campos al editar mediante `GET /api/v1/quadlets/{name}`, con manejo del error de consulta. — Verificación: test de componente que precarga los campos leídos y otro que muestra el `error.message` si la consulta falla.

## 8. Cierre

- [x] 8.1 `just check-all` en verde y `clippy -D warnings` limpio. — Verificación: salida EXIT 0.
- [x] 8.2 Reviewers (`rust-reviewer`, `react-reviewer`) sin bloqueantes. — Verificación: informes de review.
- [x] 8.3 `openspec validate --all` y `docs/edicion-borrado-quadlets.md` redactado. — Verificación: validación 8/8 y doc existente.
