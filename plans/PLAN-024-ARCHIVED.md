# Crear rama main y configurar branch protection + ruleset para release.yml — Implementation Plan (ARCHIVADO)

> Estado: **completado** (2026-11-23). Issue: #51.

## Objetivo

Crear la rama `main` desde `development`, proteger ambas ramas y permitir que el
workflow `release.yml` empuje a `main` y resincronice `development`.

## Resultado real (desviación respecto al plan original)

El plan original asumía que la integración de GitHub Actions (app_id 15368)
podría ser bypass actor del ruleset. **No es posible en repos personales**: la
API responde `422 Actor GitHub Actions integration must be part of the ruleset
source or owner organization`. Una deploy key como bypass tampoco funciona en
rulesets de rama.

Solución final: rulesets con bypass del **rol admin** (`RepositoryRole` id 5) y
el workflow usa un **PAT de admin** (`secrets.RELEASE_TOKEN`, guardado en crypta
como `github_podpot_release_token`) en `actions/checkout`.

## Tareas

### Tarea 1: Ruleset de protección en `development`
- [x] Ruleset `development-protection` (id 27879478): PR + checks strict +
      deletion + non_fast_forward; bypass `RepositoryRole` admin.
- [x] Retirada la branch protection clásica previa (PLAN-023), sustituida por el
      ruleset.

### Tarea 2: Crear rama `main` desde `development`
- [x] `main` creada desde `development` y empujada a origin.

### Tarea 3: Proteger `main`
- [x] Ruleset `main-protection` (id 27879853) con reglas idénticas a `development`.

### Tarea 4: Bypass para `release.yml`
- [x] `release.yml` usa `token: ${{ secrets.RELEASE_TOKEN }}` en el checkout del
      job `prepare`.
- [x] Secreto `RELEASE_TOKEN` configurado (PAT de admin).

### Tarea 5: Documentación y cierre
- [x] `docs/branch-protection.md` actualizado (rulesets + bypass por PAT).
- [x] Release `v0.2.0` publicada de extremo a extremo (tag, imagen ghcr,
      GitHub Release, resync de `development`).
