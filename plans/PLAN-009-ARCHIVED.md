# PLAN-009 — autodesplazamiento determinista del visor de registros

- **Issue:** #18 — 🐛 Test flaky: autodesplazamiento del visor de registros (VisorDeRegistros) en CI
- **Rama:** `chore/visor-registros-scroll-determinista` (desde `development`)
- **Change OpenSpec:** `openspec/changes/autodesplazamiento-visor-registros/`
- **Estado:** PLANIFICACIÓN (pendiente de aprobación de la spec)

## Objetivo

El test «arrastra el scroll al final…» de `VisorDeRegistros` falla de forma intermitente en CI porque afirma justo después de que aparezca la línea, pero el autodesplazamiento lo dispara un efecto que corre tras el volcado por lote del hook (buffer ~150 ms). Además, ese comportamiento **no está especificado**. El trabajo: (1) **especificar** el autodesplazamiento condicional en la capability `ui`; (2) **endurecer** el test para que espere al efecto de forma determinista y cubra el umbral.

## Mapa

- Spec: `openspec/changes/autodesplazamiento-visor-registros/specs/ui/spec.md` (requires nuevo: autodesplazamiento condicional con umbral de 24 px).
- Test: `frontend/src/components/VisorDeRegistros/VisorDeRegistros.test.tsx` (espera determinista del scroll; caso de umbral).
- Componente (referencia, sin cambio de comportamiento esperado): `frontend/src/components/VisorDeRegistros/VisorDeRegistros.tsx` (`UMBRAL_DE_FONDO_PX`, `efecto [lineas]`, `pegadoAlFondo`).

## Tareas (TDD)

1. Añadir el requirement a la delta de `ui` y validar el change (`openspec validate --strict`).
2. Test: hacer determinista la aserción del autodesplazamiento (esperar al efecto, no al timing del runner) y añadir el caso del umbral (a ≤24 px desplaza; a >24 px no).
3. Refactor: reutilizar el helper de doblado de `scrollIntoView` sin duplicar; sin cambios de comportamiento del componente salvo que el test demuestre lo contrario.
4. Verificación: `pnpm run test` repetido (p. ej. 20 pasadas del fichero) sin flake; typecheck/lint/build; CI verde.
5. Cierre: `docs/autodesplazamiento-visor-registros.md`, archivar el change, plan → `-ARCHIVED`, PR y cerrar el issue.

## Criterios de aceptación

- `cd frontend && pnpm run test` estable en N pasadas consecutivas (0 flake); `typecheck`/`lint`/`build` verdes.
- `openspec validate autodesplazamiento-visor-registros --strict` y `openspec validate --all` en verde.
- CI del PR en verde.
- Comportamiento del visor sin cambios observables (solo se especifica y se estabiliza su prueba).

## Gitflow

Rama `chore/*` → PR a `development`; conventional commits + gitmoji; al cerrar, renombrar este plan a `plans/PLAN-009-ARCHIVED.md`.
