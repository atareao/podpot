# PLAN-015 — Toast de borrado con la identidad publicada

- **Issue:** #30 (https://github.com/atareao/podpot/issues/30)
- **Change OpenSpec:** `toast-identidad-borrado`
- **Rama:** `feature/toast-identidad-borrado` desde `development`
- **Alcance:** solo frontend. Sin cambios de backend, API ni base de datos.

## Objetivo

Alinear el aviso de éxito del borrado con la identidad publicada por la API: hoy usa `unidad.name`; debe usar `identidadDe(unidad)`, que devuelve el `file_name` publicado.

## Contexto y decisiones

- `identidadDe` ya existía en `TablaDeUnidades.tsx`; se mueve a `frontend/src/identidad.ts` para reutilizarla sin exportar una función no-componente desde un archivo de componente (warning `react-refresh/only-export-components`).
- El módulo nuevo solo importa el tipo `QuadletUnit`; no introduce ciclos.
- El toast de borrado usa `identidadDe(unidad)`; el fallback defensivo a `name` sigue aplicando solo si `file_name` es cadena vacía.
- No se cambia el tipo `QuadletUnit.file_name`: es `string` no nulo.
- No se tocan los toasts de crear/editar/acción; quedan como follow-up separado.

## Archivos implicados

- `frontend/src/identidad.ts` (nuevo)
- `frontend/src/components/TablaDeUnidades/TablaDeUnidades.tsx`
- `frontend/src/pages/unidades.tsx`
- `frontend/src/pages/unidades.test.tsx`

## Tareas (SDD + TDD)

### 1. Baseline

- [x] 1.1 Ejecutar el test de `unidades.test.tsx` y confirmar verde.
- [x] 1.2 Confirmar rama limpia y crear `feature/toast-identidad-borrado` desde `development`.

### 2. RED

- [x] 2.1 Actualizar las expectativas del toast para exigir `file_name`.
- [x] 2.2 Añadir cobertura del fallback a `name` con `file_name: ''`.
- [x] 2.3 Ejecutar tests y confirmar los fallos esperados.

### 3. GREEN

- [x] 3.1 Crear `frontend/src/identidad.ts` con `identidadDe`.
- [x] 3.2 Usar `identidadDe` en el toast de borrado.
- [x] 3.3 Ejecutar tests focalizados y confirmar verde.

### 4. REFACTOR y verificación

- [x] 4.1 Eliminar el warning de `react-refresh` moviendo la utilidad a `identidad.ts`.
- [x] 4.2 Ejecutar `pnpm --dir frontend vitest run`.
- [x] 4.3 Ejecutar typecheck y lint del frontend.
- [x] 4.4 Ejecutar `just check-all`.

### 5. Cierre

- [x] 5.1 Crear `docs/toast-identidad-borrado.md`.
- [ ] 5.2 Actualizar `HANDOFF.md`.
- [ ] 5.3 Commit gitmoji, push y PR a `development`.
- [ ] 5.4 Archivar el change y el plan tras el merge.

## Criterios de aceptación

- [x] El toast de borrado identifica la quadlet con el mismo `file_name` que la fila y el `Popconfirm`.
- [x] El fallback a `name` sigue funcionando si `file_name` está vacío.
- [x] Tests, typecheck, lint y `just check-all` en verde.
- [x] Sin cambios de backend ni de contrato de API.
