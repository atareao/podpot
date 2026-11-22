# PLAN-014 — Follow-ups de identidad en la tabla de unidades

- **Issue:** #29 (https://github.com/atareao/podpot/issues/29)
- **Change OpenSpec:** `followups-identidad-popconfirm`
- **Rama:** `feature/followups-identidad-popconfirm` desde `development`
- **Alcance:** solo frontend. Sin cambios de backend, API ni base de datos.

## Objetivo

Cerrar tres deudas de identidad detectadas al archivar `identidad-file-name-tabla`:

1. El `Popconfirm` de borrado usa `unidad.name` y `unidad.file_name` en crudo, mientras la fila y los `aria-label` usan `identidadDe(unidad)`.
2. El test de identidad usa un cast `null as unknown as string` para simular un `file_name` ausente, aunque el contrato real es `file_name: string` no nulo.
3. El test de orden de la identidad primaria usa `textContent.indexOf(...)`, frágil ante cambios de formato y poco expresivo.

## Contexto y decisiones

- `QuadletUnit.file_name` es `string` en `frontend/src/types/api.ts`; la API lo publica siempre. No se cambia el tipo ni se añade lógica de backend.
- `identidadDe(unidad)` devuelve `unidad.file_name || unidad.name`; se mantiene como única fuente de la identidad mostrada y accesible.
- El `Popconfirm` debe usar `identidadDe(unidad)` tanto en el título como en la descripción, sin deducir ni componer nombres.
- El fallback a `name` sigue siendo defensivo para valores vacíos. Se probará con `file_name: ''`, no con un cast que rompe el contrato.
- El test de orden se endurece comparando posiciones DOM reales (`compareDocumentPosition` o `within`), no índices de cadena.

## Archivos implicados

- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx`
- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.test.tsx`
- Documentación: `docs/followups-identidad-popconfirm.md` al cerrar.

## Tareas (SDD + TDD)

### 1. Baseline

- [ ] Ejecutar `pnpm --dir frontend vitest run src/components/TablaDeUnidades/TablaDeUnidades.test.tsx` y confirmar verde.
- [ ] Confirmar que no hay cambios locales ni rama previa con el mismo nombre.

### 2. RED — tests primero

- [ ] Añadir test que abra el `Popconfirm` de borrado y verifique que el título y la descripción usan `identidadDe(unidad)` (incluido un caso con `file_name` distinto de `name`).
- [ ] Sustituir el test con `null as unknown as string` por un caso con `file_name: ''` que verifique el fallback a `name` y la ausencia del literal `null`.
- [ ] Endurecer el test de orden de la celda «Unidad» para comprobar que el nodo del `file_name` precede al nodo del nombre de unidad mediante comparación DOM.
- [ ] Ejecutar el test y confirmar que falla por las razones esperadas (nuevo contrato del `Popconfirm` y orden DOM).

### 3. GREEN — implementación mínima

- [ ] En `TablaDeUnidades.tsx`, usar `identidadDe(unidad)` en el título y la descripción del `Popconfirm`.
- [ ] Ajustar comentarios solo si aclaran el contrato; sin refactors colaterales.
- [ ] Ejecutar el test focalizado y confirmar verde.

### 4. REFACTOR y verificación

- [ ] Revisar que no quedan casts que oculten el contrato ni usos de `unidad.file_name` en crudo en la UI.
- [ ] Ejecutar `pnpm --dir frontend vitest run`.
- [ ] Ejecutar `pnpm --dir frontend tsc --noEmit` (o el script equivalente del repo).
- [ ] Ejecutar `pnpm --dir frontend lint` (o el script equivalente).
- [ ] Ejecutar `just check-all` y confirmar verde.

### 5. Cierre

- [ ] Crear `docs/followups-identidad-popconfirm.md` con qué es, archivos, funcionamiento, decisiones y follow-ups.
- [ ] Actualizar `HANDOFF.md`.
- [ ] Commit gitmoji en `feature/followups-identidad-popconfirm`, push y PR a `development`.
- [ ] Marcar tareas de este plan y renombrarlo a `plans/PLAN-014-ARCHIVED.md` tras el merge.

## Criterios de aceptación

- [ ] El `Popconfirm` identifica la quadlet con la misma identidad que la fila y los `aria-label`.
- [ ] No queda ningún cast `null as unknown as string` en el test de identidad.
- [ ] El test de orden no depende de `indexOf` sobre `textContent`.
- [ ] Tests, typecheck, lint y `just check-all` en verde.
- [ ] Sin cambios de backend ni de contrato de API.
