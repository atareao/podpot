# Tasks

## 1. Especificación

- [x] 1.1 Cuatro artefactos OpenSpec (`proposal.md`, `specs/ui/spec.md`, `design.md`, `tasks.md`) escritos y `openspec validate --change iconos-en-acciones` en verde

## 2. RED — tests que capturan el síntoma

- [x] 2.1 Añadir en `TablaDeUnidades.test.tsx` tests que comprueben que no queda texto visible de las seis acciones y que cada control conserva su nombre accesible; verificar que `npx vitest run src/components/TablaDeUnidades/TablaDeUnidades.test.tsx` falla solo por esos tests nuevos
- [x] 2.2 Revisar `TablaDeUnidades.filtros.test.tsx` y los tests de `pages/unidades` para que sigan localizando los controles por nombre accesible; verificar que los tests heredados permanecen en verde

## 3. GREEN — implementación mínima

- [x] 3.1 Crear el módulo de mapeo acción → (icono, etiqueta) con imports individuales de `@ant-design/icons`; verificar que no aparece `texto` de acción en el JSX de los botones
- [x] 3.2 Sustituir los seis `Button` por botones con `icon`, `aria-label` y `Tooltip`; verificar que los tests de `TablaDeUnidades` pasan
- [x] 3.3 Comprobar tipos con `npx tsc --noEmit` sin errores

## 4. REFACTOR y documentación

- [x] 4.1 Extraer constantes compartidas y revisar accesibilidad de los seis controles; verificar `npx eslint .` sin errores
- [x] 4.2 Crear `docs/iconos-en-acciones.md` con qué es, ficheros implicados, decisiones y follow-ups; verificar que el documento describe el cambio y cómo verificarlo
- [x] 4.3 Ejecutar `just check-all` y verificar rc=0 (incluye el test de presupuesto de bundle)

## 5. Revisión, cierre y despliegue

- [x] 5.1 Auditoría de `@react-reviewer` con resultado APPROVE y sin hallazgos abiertos (APPROVE en reauditoría)
- [x] 5.2 PR a `development`, merge y cierre del issue #67; verificar estado del PR y del issue con `gh`
- [x] 5.3 Archivar el change OpenSpec y renombrar `plans/PLAN-029.md` a `plans/PLAN-029-ARCHIVED.md`; verificar `openspec validate --specs` en verde
- [ ] 5.4 Reconstruir la imagen con `just build`, reiniciar `podpot.service` y verificar `GET /api/v1/health` 200 y los stacks correctos en `/api/v1/units` (pendiente: just build + reinicio de podpot.service)
