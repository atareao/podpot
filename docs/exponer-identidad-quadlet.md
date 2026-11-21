# Identidad real de quadlet en el listado

## Qué es

Elimina la **deducción inversa** con la que el panel reconstruía el nombre de fichero de una quadlet a partir del nombre de unidad. El backend publica ahora, en `GET /api/v1/units`, la identidad **real** de cada quadlet (`file_name`, el basename del fichero) y su `kind` opcional; el frontend la consume tal cual y deja de adivinar.

Motivo: la regla inversa `nombre de unidad → <stem>.<kind>` **no es inyectiva**. Un contenedor `web-volume` y un volumen `web` generan ambos la unidad `web-volume.service`, y la deducción resolvía siempre a `web.volume` —el fichero equivocado—. Editar o borrar en ese caso colisionado apuntaba al fichero incorrecto.

Issue #6; change OpenSpec `exponer-identidad-quadlet`; spec `ui` (y `api-shell`/`systemd-dbus` en el backend).

## Ficheros implicados

### Backend (`backend/`)

- `src/systemd/mod.rs` — accesor de dominio con el **basename** de la procedencia (`UnitOrigin`/`UnitStatus`); el `Serialize` de `UnitOrigin` sigue omitiendo la ruta.
- `src/api/units.rs` — `UnitSummary` gana `file_name: String` y `kind: Option<QuadletKind>`, poblados desde `UnitStatus`. **Nunca** se publica la ruta completa.
- `src/systemd/fake.rs` — el doble ya construía las quadlets con origen; el listado las expone con su `file_name`.

### Frontend (`frontend/`)

- `src/types/api.ts` — `QuadletUnit` gana `file_name: string` y `kind: TipoDeQuadlet | null`, espejo de la respuesta.
- `src/components/TablaDeUnidades/TablaDeUnidades.tsx` — usa `unit.file_name` como identidad; el `Popconfirm` de borrado nombra ese fichero y `rowKey` es `file_name` (dos filas pueden compartir nombre de unidad).
- `src/pages/unidades.tsx` — editar/borrar operan sobre `unit.file_name`; el estado de edición guarda la `QuadletUnit` completa (ya no una identidad deducida).
- `src/hooks/useUnits.ts` — `useVerificarQuadlet` sigue existiendo (verificación de existencia pre-`DELETE`), pero su justificación pasa de «desambiguar la deducción» a «el listado es una foto: el fichero pudo desaparecer fuera del panel».
- **Retirado**: `src/quadlets/identidad.ts` y `src/quadlets/identidad.test.ts` (la deducción inversa ya no existe).

## Cómo funciona

1. El backend lee de systemd el `SourcePath` de cada unidad (D-Bus) y publica **solo su basename** (`web.container`) como `file_name`; la ruta completa no sale nunca.
2. `kind` se deriva de la extensión con `QuadletKind::from_extension`; es `null` cuando la extensión no es un tipo creable (el panel no lo necesita para operar).
3. El panel toma `file_name` como identidad y lo usa en `GET`/`PATCH`/`DELETE /api/v1/quadlets/{name}`.
4. Dos quadlets con el mismo nombre de unidad (`web-volume.service`) se distinguen por su `file_name` (`web-volume.container` frente a `web.volume`); la tabla las identifica por fichero (`rowKey="file_name"`).

## Configuración y variables

- Sin variables nuevas. El contrato de API es **aditivo** (`file_name`/`kind` se añaden al listado); no rompe consumidores previos.

## Cómo habilitar / desplegar

- Nada especial: el cambio va en el binario del backend y en el bundle del frontend. Sin migraciones ni configuración.

## Decisiones

- **Publicar el basename, no la ruta**: resuelve la identidad sin filtrar rutas del usuario (la intención de privacidad de `UnitOrigin` se preserva con `#[serde(skip)]`).
- **El dato derivado vive en el dominio**: el basename se calcula con un accesor de `UnitStatus`, no en la capa API, para que el doble de pruebas y el cliente real compartan la regla.
- **El frontend deja de deducir**: `identidadDeUnidad` se elimina en lugar de arrastrarla como fallback permanente (perpetuaría la ambigüedad).
- **La verificación `GET` pre-`DELETE` se mantiene**: ya no desambigua, pero sigue cubriendo la carrera entre pintar el listado y borrar.

## Follow-ups

- Auth OIDC; SSE/`PropertiesChanged` para el panel en vivo; chunk de antd >500 kB.
