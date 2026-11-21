# Edición y borrado de quadlets

## Qué es

Cierra el ciclo de vida de las quadlets en podpot: además de listar/controlar (`bootstrap-core`) y crear (`creacion-quadlets`, issue #3), permite **consultar**, **editar** y **borrar** una quadlet existente desde el panel. Issue #5; change OpenSpec `edicion-borrado-quadlets`; plan `plans/PLAN-003-ARCHIVED.md`.

## Ficheros implicados

### Backend (`backend/`)
- `src/quadlet/mod.rs` — dominio: `QuadletError::{NotFound, ReadFailed, Registro}`, `Quadlet::from_file_name` (identidad `<stem>.<kind>`), `QuadletDraft::parse` (inverso del render), `describe_a` (inmutabilidad del nombre).
- `src/quadlet/store.rs` — `leer`, `reescribir` (tmp + `rename` atómico, permisos `0600`/preservados), `borrar`, `nombre_temporal`.
- `src/quadlet/service.rs` — orquestación `leer`/`editar`/`borrar` (validar → identidad → existencia → render → escritura → `daemon-reload` → registrar).
- `src/api/quadlets.rs` — handlers `leer` (GET), `editar` (PATCH), `borrar` (DELETE).
- `src/api/mod.rs` — ruta `/quadlets/{name}` = `get(...).patch(...).delete(...)`.
- `src/error.rs` — mapeo HTTP estable de los errores tipados.
- `migrations/0003_quadlet_operations.sql` — tabla de operaciones (tipo `edit`/`delete`, nombre de fichero, marca temporal, `CHECK`, índice).

### Frontend (`frontend/`)
- `src/api/quadlets.ts` — `obtenerQuadlet` (GET), `editarQuadlet` (PATCH), `borrarQuadlet` (DELETE).
- `src/quadlets/identidad.ts` — deducción inversa `nombre de unidad → <stem>.<kind>` (ver decisiones).
- `src/hooks/useUnits.ts` — `useDetalleDeQuadlet`, `useVerificarQuadlet`, `useEditarQuadlet`, `useBorrarQuadlet`.
- `src/components/FormularioDeCreacion/FormularioDeCreacion.tsx` — reutilizado con `valoresIniciales`/`identidadFija`.
- `src/components/TablaDeUnidades/TablaDeUnidades.tsx` — acciones editar/borrar (con `Popconfirm`).
- `src/pages/unidades.tsx` — estados de edición/borrado, verificación pre-`DELETE`, mensajes.

## Cómo funciona

1. **Consulta**: `GET /api/v1/quadlets/{name}` lee el fichero y lo parsea de vuelta al struct tipado del tipo (tolerante: solo claves modeladas y en la sección del tipo). `404 not_found` si no existe.
2. **Edición**: `PATCH /api/v1/quadlets/{name}` exige el struct tipado completo del tipo (`deny_unknown_fields`), valida y sanitiza valores (`\n`, `\r`, `\0`, controles → `invalid_value` 400 sin tocar el fichero), reescribe de forma **atómica** (temporal `0600` + `rename`) y dispara `daemon-reload` por D-Bus. La identidad es el **nombre de fichero**; editar nunca renombra (`invalid_name` 400 si cambia).
3. **Borrado**: `DELETE /api/v1/quadlets/{name}` elimina el fichero y dispara `daemon-reload`. `404` si no existe.
4. **Registro**: cada edición/borrado exitoso se anota en `quadlet_operations`; las operaciones fallidas no.
5. **UI**: al editar se lee la definición y se precarga el formulario (si la lectura falla, se muestra `error.message` y no se abre el formulario con valores inventados). Al borrar, se **verifica con un `GET`** antes del `DELETE`, se **confirma** (el `Popconfirm` muestra el **nombre de fichero**) y se refresca el listado.

## Configuración y variables

- `PODPOT_QUADLET_DIR` (default `$HOME/.config/containers/systemd`): directorio canónico único de quadlets. La escritura/borrado se limita al **primer nivel**.
- Sin variables nuevas.

## Cómo habilitar / desplegar

No cambia el despliegue respecto a las features anteriores: contenedor rootless con `--userns=keep-id` y montaje de `/run/user/$UID`. La migración `0003` se aplica sola al arrancar (embebida con `sqlx::migrate!`).

## Decisiones

- **Identidad por nombre de fichero `<stem>.<kind>`** y **sin renombrado**: evita colisiones, symlinks rotos y registros huérfanos.
- **`PATCH` con struct completo** (no fusión de campos) para un único camino de render/validación; no se expone `PUT`.
- **Permisos `0600`** en ficheros creados/reescritos (la reescritura preserva el modo del destino si existía): una quadlet puede llevar `Environment=…PASSWORD=…`.
- **`leer` no sigue symlinks** (`symlink_metadata`): coherente con el resto del módulo.
- **Carrera benigna** entre `existe` y `reescribir`: aceptada y documentada.
- **Errores**: `NotFound`→404, `InvalidName`/`InvalidKind`/`InvalidValue`→400, `WriteFailed`/`ReadFailed`/`Registro`→500, `ReloadFailed`/`UnitRejected`→502, `Transport`→503.

## Follow-ups

- **Issue #6**: exponer `file_name`/`kind` real en `GET /api/v1/units` para eliminar la deducción no inyectiva de identidad en el frontend (`web-volume.service` es ambiguo).
- Alinear el código de error `Persistencia`/`Registro` (casi idénticos) si se quiere reducir superficie.
- (Heredados) auth OIDC; SSE/`PropertiesChanged`; chunk de antd >500 kB.
