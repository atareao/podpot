# Change Proposal: Agrupación por Prefijo

## Descripción
Implementar agrupación de unidades systemd (quadlets) por **stack** en la vista "Unidades en vivo", donde el stack se calcula a partir del **prefijo del nombre de archivo** (antes del primer `-`) con **override opcional** vía `groups.toml`.

## Contexto
Actualmente la vista "Unidades en vivo" muestra todas las unidades en una lista plana sin agrupación lógica. Los usuarios gestionan múltiples stacks (ej. `atareao-*`, `centinela-*`) y necesitan verlas agrupadas para operaciones en lote y mejor visibilidad.

## Objetivo
- Agrupar visualmente las unidades por **stack** en el frontend (Ant Design `Collapse`/`Table` por grupo).
- Calcular el stack de forma determinística: **prefijo antes del primer `-`** en `file_name` (o `unit_name` si no hay `file_name`).
- Permitir **override explícito** mediante archivo de configuración `groups.toml` (clave `file_name` o `unit_name` → `group`).
- Mantener compatibilidad: unidades sin `-` usan el nombre completo como stack; sin `file_name` ni `unit_name` usan `unit_name` sin sufijo `.service`.

## Alcance
**Incluye:**
- Backend: nuevo campo `stack` en `LiveUnitDto` + endpoint `GET /api/v1/live-units` lo incluye.
- Backend: lógica de cálculo de stack (prefijo + override `groups.toml`).
- Backend: carga y parseo de `groups.toml` (hot-reload opcional, MVP: reload al iniciar).
- Frontend: vista "Unidades en vivo" agrupada por `stack` (Collapse por grupo, Table dentro).
- Frontend: badge/etiqueta del stack en cada fila.
- Tests: unitarios (cálculo stack), integración (endpoint + override), E2E (vista agrupada).

**Excluye:**
- Edición de `groups.toml` desde la UI (futuro: PR separado).
- Agrupación en otras vistas (histórico, logs).
- Persistencia de grupos en BD (solo archivo TOML local).

## Reglas de Negocio — Cálculo del Stack

| Entrada | Regla | Ejemplo |
|---------|-------|---------|
| `file_name` existe y contiene `-` | Prefijo antes del primer `-` | `atareao-mariadb.container` → `atareao` |
| `file_name` existe, **sin** `-` | `file_name` completo (sin extensión) | `standalone.container` → `standalone` |
| `file_name` no existe, `unit_name` existe y contiene `-` | Prefijo antes del primer `-` en `unit_name` (sin `.service`) | `centinela-app.service` → `centinela` |
| `file_name` no existe, `unit_name` existe, **sin** `-` | `unit_name` sin `.service` | `singleton.service` → `singleton` |
| Override en `groups.toml` (`file_name` o `unit_name` como clave) | **Prioridad absoluta** sobre regla base | `groups.toml: "atareao-mariadb.container" = "db-cluster"` → `db-cluster` |

**Formato `groups.toml`:**
```toml
# Clave = file_name (preferido) o unit_name
"atareao-mariadb.container" = "db-cluster"
"centinela-app.service" = "monitoring"
"standalone.container" = "infra"
```

## Criterios de Aceptación
1. **CA-01**: `GET /api/v1/live-units` devuelve campo `stack` en cada unidad calculado según reglas.
2. **CA-02**: Override en `groups.toml` gana sobre regla de prefijo.
3. **CA-03**: Frontend muestra `Collapse` por stack, cada uno con `Table` de sus unidades.
4. **CA-04**: Unidades sin `-` en nombre aparecen en grupo con su nombre completo.
5. **CA-05**: Tests unitarios cubren todas las combinaciones de la tabla de reglas.
6. **CA-06**: Test de integración valida override con `groups.toml` temporal.
7. **CA-07**: Test E2E (Playwright) verifica render agrupado en UI.

## Referencias
- **Issue**: #65 — "Agrupar unidades en vivo por stack (prefijo + override)"
- **Plan**: PLAN-028 — Implementación completa (backend, frontend, tests, docs)

## Impacto
- **Bajo riesgo**: cambio puramente presentacional + campo derivado en DTO.
- **Sin migración BD**: `stack` es campo computado, no persistido.
- **Configuración opcional**: `groups.toml` no es obligatorio; si no existe, solo regla de prefijo.