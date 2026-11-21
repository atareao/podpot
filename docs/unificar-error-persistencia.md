# Unificar el error de persistencia

## Qué es

Colapsa las dos variantes casi idénticas con las que `QuadletError` representaba el mismo desenlace —un fallo al escribir en SQLite— en **una sola**: `QuadletError::Persistencia`. Antes convivían `Persistencia` (creación) y `Registro` (edición/borrado) y ambas traducían al mismo código HTTP `internal` (`500`); la segunda era superficie duplicada sin valor de contrato, porque la causa real ya viaja en el campo `causa` y no se publica.

Es el follow-up registrado en `docs/edicion-borrado-quadlets.md`. Issue #20; PR #21; change OpenSpec `unificar-error-persistencia`; capability `persistencia`, requisito «Fallo de persistencia tipado y unificado».

## Ficheros implicados

### Backend (`backend/`)

- `src/quadlet/mod.rs` — se elimina la variante `QuadletError::Registro`; queda `Persistencia { file_name, causa: Box<sqlx::Error> }` como única variante de fallo de persistencia. Su `Display` interno pasa de nombrar «la creación» a «la operación sobre».
- `src/quadlet/service.rs` — las construcciones de la variante pasan a `Persistencia`: `registrar_operacion` (`:209`, edición/borrado) y `registrar` (`:225`, creación).
- `src/error.rs` — el brazo `Registro` de `traducir_quadlet` se elimina; `Persistencia` → `internal`/`500` con mensaje genérico y `tracing::error!` de la causa.
- `src/api/quadlets.rs` — tests de fallo de persistencia que fijan el desenlace unificado: en creación (`:837`) y en edición/borrado (`:807`).

## Cómo funciona

1. Toda operación de escritura en la tabla de registro que falla —tanto `quadlet_creations` (creación) como `quadlet_operations` (edición/borrado)— produce un `QuadletError::Persistencia` con el `file_name` afectado y la causa de `sqlx` en caja.
2. `traducir_quadlet` mapea esa única variante a `internal` (`500`), con el mensaje genérico `INTERNAL_MESSAGE`; la causa de SQLite **solo** va al log (`tracing::error!`), nunca al cuerpo.
3. El **contrato HTTP no cambia**: envoltura `{error:{code,message}}`, `code = "internal"`, `status = 500`, sin campo `data`.
4. El hecho ocurrido **no se deshace** si falla el registro: en la creación el fichero ya está escrito, en la edición/borrado ya se reescribió o se borró. Es un fallo del propio podpot, no de la petición.
5. Los tests fuerzan el fallo **cerrando el pool** (`pool.close().await`), de forma determinista y sin depender de permisos del disco, y comprueban `500`, `code=internal`, ausencia de `data` y que el fichero sigue en su sitio.

## Configuración y variables

- Sin variables nuevas. Sin cambios en configuración ni en el despliegue.

## Cómo habilitar / desplegar

- Nada especial: el cambio va en el binario del backend. Sin migraciones, sin variables y sin tocar el frontend.

## Decisiones

- **Una sola variante tipada** (`Persistencia`) para creación, edición y borrado: dos variantes con la misma traducción HTTP no aportaban nada y obligaban a mantener dos brazos idénticos en `traducir_quadlet`.
- **No dar un código propio al fallo de persistencia**: se mantiene `internal` genérico para no alterar el contrato ni filtrar detalles internos. El detalle —el `Display` de SQLite— queda en el log, coherente con la regla de que un `5xx` no publica su causa.
- **El mensaje `Display` interno pasa de «creación» a «operación sobre»**: el mismo error se usa ahora en los tres caminos, así que el texto no puede nombrar solo uno. Es solo para el log; sin impacto en HTTP.

## Follow-ups

- Endurecer más la cobertura del fallo de persistencia si se quiere (el cierre del pool cubre el camino feliz del fallo, no variantes de error de SQLite).
- (Heredados) auth OIDC; SSE/`PropertiesChanged`; chunk de antd >500 kB.
