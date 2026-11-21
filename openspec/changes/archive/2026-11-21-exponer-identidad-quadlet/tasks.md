# Tasks — exponer-identidad-quadlet

TDD por bloques: primero las pruebas (rojo), luego la implementación mínima (verde), luego refactor. Cada tarea indica su criterio de verificación. Orden por dependencia: dominio → API → frontend → limpieza → cierre.

## 1. Dominio: nombre de fichero derivado de la procedencia

- [x] 1.1 Accesor de dominio que devuelve el basename de `UnitOrigin` (`web.container`) y «sin nombre» si la procedencia es desconocida. — Verificación: test rojo→verde con origen con ruta y con origen desconocido.
- [x] 1.2 Exponer el nombre de fichero desde `UnitStatus` sin publicar la ruta (el `Serialize` de `UnitOrigin` sigue omitido). — Verificación: test que confirma que la serialización de `UnitStatus` NO incluye la ruta pero el accesor sí da el basename.

## 2. API: identidad en el listado

- [x] 2.1 `UnitSummary` gana `file_name` (basename) y `kind` opcional (`null` si la extensión no es un tipo conocido), poblados desde `UnitStatus`. — Verificación: test de integración de `GET /api/v1/units` que comprueba `file_name` y `kind` de cada quadlet.
- [x] 2.2 La ruta del sistema de ficheros NO se publica en el listado. — Verificación: test que inspecciona el objeto de una unidad y confirma que no hay campo de ruta y que `file_name` es un basename.
- [x] 2.3 El doble de pruebas sigue construyendo quadlets con origen y el listado las expone con su `file_name`. — Verificación: test con `FakeSystemd` que comprueba el `file_name` de las quadlets de muestra.

## 3. Frontend: tipos y consumo de la identidad publicada

- [x] 3.1 `QuadletUnit` gana `file_name: string` y `kind: TipoDeQuadlet | null`, espejo de la respuesta. — Verificación: `npx tsc --noEmit` limpio y test de tipo/cliente.
- [x] 3.2 `TablaDeUnidades` y `pages/unidades.tsx` usan `unit.file_name` como identidad en editar/borrar, en lugar de `identidadDeUnidad`. — Verificación: test de componente que edita y borra sobre el `file_name` publicado.
- [x] 3.3 Caso colisionado: dos quadlets con el mismo nombre de unidad se distinguen por su `file_name`. — Verificación: test de componente que opera sobre el `file_name` correcto de cada una.

## 4. Limpieza

- [x] 4.1 Retirar `frontend/src/quadlets/identidad.ts` y su test (ya no hay deducción inversa). — Verificación: `rg identidadDeUnidad` sin resultados en `frontend/src` y `npx vitest run` en verde.
- [x] 4.2 Documentar en `docs/` la nueva identidad publicada y el cambio de contrato. — Verificación: doc redactado.

## 5. Cierre

- [x] 5.1 `just check-all` en verde y `clippy -D warnings` limpio. — Verificación: salida EXIT 0.
- [x] 5.2 Reviewers (`rust-reviewer`, `react-reviewer`) sin bloqueantes. — Verificación: informes de review.
- [x] 5.3 `openspec validate --all` en verde y `docs/exponer-identidad-quadlet.md` redactado. — Verificación: validación OK y doc existente.
