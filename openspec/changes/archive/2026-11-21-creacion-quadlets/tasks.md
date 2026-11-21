# Tasks — creacion-quadlets

TDD por bloques: primero las pruebas (rojo), luego la implementación mínima (verde), luego refactor. Cada tarea indica su criterio de verificación. Orden por dependencia: dominio → escritura → D-Bus → persistencia → API → UI → cierre.

## 1. Dominio: validación de nombre y tipo

- [x] 1.1 Tipos de dominio de la creación (kinds soportados `container`/`volume`/`network`/`pod` y sus fields). — Verificación: `cargo test` de las pruebas de parseo del struct tipado.
- [x] 1.2 Validación de charset del stem. — Verificación: test rojo→verde que devuelve `InvalidName` para stems inválidos.
- [x] 1.3 Validación de extensión permitida, con `.desktop` excluida. — Verificación: test que devuelve `InvalidKind` para `.desktop` y extensiones fuera del conjunto.
- [x] 1.4 Validación de unicidad del nombre de fichero `<stem>.<kind>` en el directorio configurado (`PODPOT_QUADLET_DIR`), sin escribir en disco al colisionar; stems iguales con tipos distintos NO son colisión. — Verificación: test que devuelve `Duplicate` y comprueba que el directorio destino queda intacto, más test de `web.container` + `web.volume` sin colisión.
- [x] 1.5 Sanitización de los valores de campo: rechazo de `\n`, `\r`, `\0` y caracteres de control con `InvalidValue { field }`. — Verificación: test de inyección (`image="nginx\nUser=root"`) que devuelve `InvalidValue` y no escribe fichero.

## 2. Plantilla tipada

- [x] 2.1 Render determinista por tipo desde el struct tipado. — Verificación: test de snapshot por tipo comparando el contenido generado.
- [x] 2.2 Rechazo de contenido libre de fichero. — Verificación: test que confirma que el contrato no acepta un cuerpo de fichero arbitrario.

## 3. Escritura sin pisar

- [x] 3.1 Escritura en el primer nivel de `PODPOT_QUADLET_DIR`. — Verificación: test con directorio temporal que comprueba la ruta y el contenido del fichero.
- [x] 3.2 Rechazo si existe un fichero regular con el nombre destino (no pisar). — Verificación: test que precrea el fichero y confirma error tipado y contenido original intacto.

## 4. Recarga D-Bus y nombres de unidad

- [x] 4.1 Operación de recarga (`daemon-reload`) en el trait `SystemdClient` con doble de pruebas. — Verificación: test con doble que verifica la llamada de recarga.
- [x] 4.2 Nombre de unidad generado: `.container`→`<stem>.service`; `volume|network|pod`→`<stem>-<ext>.service`. — Verificación: test de tabla por tipo.
- [x] 4.3 Orquestación: escribir → recargar → devolver nombre; mapear `ReloadFailed`/`UnitRejected`/`Transport`. — Verificación: tests rojo→verde con doble en fallo de recarga y de transporte.

## 5. Persistencia

- [x] 5.1 Migración `0002_*.sql` (prefijo entero) con la tabla de registro de creaciones. — Verificación: `cargo test` contra base vacía que comprueba que la tabla existe tras migrar.
- [x] 5.2 Registro de la creación por el pool, preservando WAL y `foreign_keys`. — Verificación: test que escribe un registro y comprueba `PRAGMA foreign_keys=1` y el modo WAL.
- [x] 5.3 No registrar creaciones fallidas. — Verificación: test que fuerza un fallo previo a completar y comprueba que no queda registro.

## 6. API

- [x] 6.1 `POST /api/v1/quadlets` con la envoltura `{data}`/`{error}`. — Verificación: test de integración de la ruta con respuesta correcta.
- [x] 6.2 Mapeo de errores tipados a HTTP: `Duplicate`→409, `InvalidName`/`InvalidKind`/`InvalidValue`→400, `WriteFailed`→500, `ReloadFailed`/`UnitRejected`→502, `Transport`→503. — Verificación: test de tabla de mapeo en la ruta.
- [x] 6.3 Preservar los fallbacks `404`/`405` con envoltura, incluida la nueva ruta. — Verificación: tests de `405` sobre `/api/v1/quadlets` y de `404` de ruta inexistente.

## 7. UI

- [x] 7.1 Formulario de creación (Antd) con selector de tipo y campos mínimos por tipo. — Verificación: test de componente (Vitest + Testing Library) que rellena y envía.
- [x] 7.2 Refrescar el listado tras crear con éxito. — Verificación: test que comprueba la recarga del listado tras el éxito de la API.
- [x] 7.3 Mostrar `error.message` al fallar. — Verificación: test que simula error de la API y comprueba el mensaje mostrado.

## 8. Cierre del cambio

- [x] 8.1 `just check` (fmt → clippy → test) y `just check-all` en verde. — Verificación: salida limpia de los comandos.
- [x] 8.2 `docs/<feature>.md` de la feature de creación de quadlets. — Verificación: documento existente y enlazado en el change.
- [x] 8.3 `just spec-validate creacion-quadlets` en verde. — Verificación: `openspec validate creacion-quadlets --strict` sin errores.
- [ ] 8.4 PR a `development` y archivar el change (`openspec archive creacion-quadlets`). — Verificación: change archivado y delta fundido en `openspec/specs/`.
