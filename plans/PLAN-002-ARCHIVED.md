# PLAN-002 — Creación de quadlets

- **Issue:** #3 — https://github.com/atareao/podpot/issues/3
- **Change OpenSpec:** `creacion-quadlets` (`openspec/changes/creacion-quadlets/`)
- **Rama:** `feature/creacion-quadlets` (desde `development`)
- **Estado:** borrador — **pendiente de aprobación del usuario** (SDD Phase 1)

## Objetivo

Que podpot pueda **crear** quadlets nuevas (`.container`, `.volume`, `.network`, `.pod`) desde la API y el panel, además de descubrirlas y accionarlas. Es la segunda función del proyecto y estrena la escritura en SQLite.

## Contexto verificado

- `qctl` (`/data/rust/qctl/src/main.rs`) **descubre** quadlets (`collect_quadlets`, primer nivel, nombre por `basename`) y solo crea **symlinks** hacia `~/.config/containers/systemd`; **no escribe contenido** de quadlets. La escritura es trabajo nuevo de podpot.
- podpot hoy: solo lectura (`GET /api/v1/health`, `GET /api/v1/units`) + una acción (`POST /api/v1/units/{name}/{action}`); sin capa de escritura a disco ni a DB. El pool SQLite aún no lo usa ninguna ruta.
- Envoltura de respuesta: `api/response.rs` (`{data}` / `{error:{code,message}}`).
- systemd D-Bus: `systemd/dbus.rs::DbusSystemdClient` (trait `SystemdClient`).

## Decisiones de diseño (a validar en la aprobación)

1. **Dir de escritura = dir canónica**: se escribe en un directorio configurable (`PODPOT_QUADLET_DIR`, por defecto `$HOME/.config/containers/systemd`, que es el que lee el generador de quadlets de systemd de usuario). **Sin symlinks** (divergencia consciente de `qctl`, que separa fuente y target).
2. **v1 = solo crear**: editar/borrar quedan fuera.
3. **Plantilla determinista por tipo**: el backend renderiza el fichero desde un struct tipado; nunca se acepta contenido libre de fichero.
4. **Sin pisar**: si ya existe un fichero regular con ese nombre, se rechaza (error tipado). Paridad con `link_or_replace`.
5. **Nombres de unidad** según `generated_unit`: `.container`→`<stem>.service`; `volume|network|pod`→`<stem>-<ext>.service`.
6. **Tras escribir**: `daemon-reload` vía D-Bus; se devuelve el nombre de unidad resultante.
7. **Persistencia**: nueva migración `0002_*.sql` (prefijo entero) con tabla de registro de quadlets creadas por podpot. Primer uso real del pool.

## Alcance

- API `POST /api/v1/quadlets` con envoltura `{data}`/`{error}`.
- Validación previa (nombre único entre dirs fuente; charset del stem; extensión permitida; `.desktop` excluido) sin tocar disco si falla.
- Escritura del fichero en el primer nivel del dir configurado + `daemon-reload`.
- Mapeo de errores: `Duplicate`→409, `InvalidName`/`InvalidKind`→400, `WriteFailed`→500, `ReloadFailed`→502 (`UnitRejected`) / 503 (`Transport`).
- Persistencia de la creación.
- UI (Antd): formulario de creación + refresco del listado.

## No alcance

- Unidades de sistema/root (solo rootless `--user`).
- Editar o borrar quadlets existentes.
- OIDC, SSE/`PropertiesChanged`, chunk de antd >500 kB (follow-ups ya registrados en `HANDOFF.md`).

## Tareas (TDD — Red/Green/Refactor)

- [ ] 1.1 Abrir el change `creacion-quadlets` y aprobar proposal/specs/design/tasks (`openspec validate creacion-quadlets --strict`). Verificación: validate sin errores.
- [ ] 1.2 Rama `feature/creacion-quadlets` desde `development` (`just gf-feature creacion-quadlets`).
- [ ] 2.1 **RED** backend: tests de dominio (validación de nombre/charset/extensión, colisión, render por tipo, nombres de unidad). Verificación: `cargo test` en rojo por lo nuevo, legacy verde.
- [ ] 2.2 **GREEN** backend: tipos `QuadletKind`/`QuadletDraft`/`Quadlet` + errores tipados + render determinista. Verificación: `cargo test` verde.
- [ ] 2.3 **RED/GREEN** backend: escritura al dir configurado (sin pisar) + `daemon-reload` vía D-Bus; test con doble `FakeSystemd` y dir temporal.
- [ ] 2.4 Persistencia: migración `0002_*.sql` (prefijo entero) + registro de creación; test de repositorio.
- [ ] 2.5 Config: `PODPOT_QUADLET_DIR` (default `$HOME/.config/containers/systemd`) + validación; test de config.
- [ ] 2.6 API: `POST /api/v1/quadlets` + mapeo de errores a HTTP; tests de integración (éxito, colisión, nombre inválido, dir no escribible).
- [ ] 3.1 **RED/GREEN** frontend: hook de API + formulario Antd; tests Vitest/RTL (éxito, error de validación, error de colisión).
- [ ] 3.2 Refresco del listado tras crear.
- [ ] 4.1 **REFACTOR**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `npx tsc --noEmit`, `npm run lint`. Verificación: sin warnings ni errores.
- [ ] 5.1 Verificación final: `just check-all` → EXIT 0; `openspec validate --all` OK; smoke real contra el bus.
- [ ] 6.1 `docs/FEATURE-00N-creacion-quadlets.md` (convención `docs/FEATURE-NNN-<slug>.md`).
- [ ] 7.1 PR a `development` (gitmoji + conventional), merge; archivar change (`just spec-archive creacion-quadlets`) y plan (`just plan-archive 2`).

## Verificación global

- `just check-all` EXIT 0 (lib + integración + ui).
- `openspec validate creacion-quadlets --strict` sin errores.
- Prueba manual: crear una `.container` desde el panel contra el bus real y ver la unidad generada.
