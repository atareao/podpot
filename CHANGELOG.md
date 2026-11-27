# Changelog
## [0.2.1] - 2026-11-27

### Miscellaneous Tasks

- Remove accidental /home/lorenzo artifact from release
## [0.3.0] - 2026-11-27

### Documentation

- *(branch-protection)* Rulesets con bypass admin + PAT, y archivar PLAN-024 (#55)
- *(openspec)* Propuesta antd-bundle-budget y PLAN-025
- *(plan)* Archivar PLAN-026
- *(openspec)* Sincronizar spec de autenticación y cerrar PLAN-026
- *(openspec)* Archivar antd-bundle-budget y cerrar PLAN-025
- *(openspec)* Archivar logout-epoch

### Features

- *(auth)* Logout epoch (#59)
- *(ui)* Agrupar quadlets por stack

### Miscellaneous Tasks

- Release v0.3.0

### Performance

- *(frontend)* Reducir huella de antd por debajo de 500 kB raw

### Refactor

- *(ui)* Restaurar componentes de Ant Design

### Testing

- *(frontend)* Tipar y ampliar presupuesto por chunk de antd
## [0.2.0] - 2026-11-23

### Bug Fixes

- *(systemd)* No listar unidades transitorias como quadlets
- *(frontend)* Alinear confirmación de borrado con la identidad publicada
- *(frontend)* Alinear toast de borrado con la identidad publicada
- *(auth)* Cerrar sesión aunque la cookie no verifique

### Documentation

- Inicializa OpenSpec y proposal de bootstrap-core
- Completa el paquete SDD de bootstrap-core (specs, design, tasks) y PLAN-001
- Corrige la ruta de migraciones en el layout (backend/migrations)
- Documenta la feature bootstrap-core
- Archiva el change bootstrap-core y el PLAN-001
- *(openspec)* Change y plan de creación de quadlets
- FEATURE-002 creación de quadlets
- *(openspec)* Archivar change creacion-quadlets y fusionar deltas
- *(plans)* Archivar PLAN-004 (exponer identidad de quadlet)
- *(plan)* Archivar PLAN-009 (visor registros scroll determinista)
- Documentar la unificación de errores de persistencia (#20)
- *(openspec)* Archivar followups-identidad-popconfirm
- *(openspec)* Archivar toast-identidad-borrado
- *(openspec)* Archivar toasts-identidad
- *(openspec)* Archivar titulo-edicion-identidad
- *(openspec)* Archivar logout-cookie-invalida
- *(openspec)* Archivar refresh-sesion-deslizante
- *(openspec)* Archivar dedupe-jwks-rotacion
- *(openspec)* Archivar discovery-caido-en-refresco-jwks
- *(openspec)* Archivar refresh-tokens-proveedor (#45) (#47)
- *(openspec)* Proteger development + plan PLAN-023 (#49)
- *(openspec)* Archivar PLAN-023 (branch protection) (#50)

### Features

- *(ui)* Listado y control de quadlets con React Query y Antd
- Shell HTTP, configuración, persistencia y capa D-Bus del backend
- *(api)* Listado y control de quadlets sobre la capa de systemd
- *(panel)* Sirve el panel estático (PODPOT_STATIC_DIR) y aplica los menores del review
- *(quadlets)* Crear quadlets (.container/.volume/.network/.pod)
- *(quadlets)* Edición, borrado y consulta de quadlets (API + UI)
- *(units)* Exponer file_name/kind real de cada quadlet en el listado
- *(sse)* Panel en vivo con SSE alimentado por señales D-Bus de systemd
- *(auth)* Autenticación OIDC opcional con sesión en cookie y gate
- *(registros)* Canal SSE de logs del contenedor por la API de Podman
- *(progreso-de-trabajos)* Progreso de acciones por job en el panel
- *(frontend)* Alinear avisos con la identidad publicada
- *(frontend)* Usar identidad publicada en el título de edición
- *(auth)* Renovar la sesión de forma deslizante
- *(auth)* Deduplicar el refresco forzado del JWKS
- *(auth)* Refrescar el ID token del proveedor con refresh_token (#46)

### Miscellaneous Tasks

- Bootstrap repositorio (AGENTS.md, estructura, gitignore)
- *(ui)* Scaffold de Vite + React 19 + Antd + TanStack Query en frontend/
- *(deploy)* Añade imagen multi-stage y compose rootless con keep-id
- Añade .justfile
- Checks de PR y release a ghcr.io con GitHub Actions
- *(openspec)* Archivar el change unificar-error-persistencia (#20)
- Trigger release workflow (#52)
- Trigger release workflow (#53)
- *(release)* Usar PAT de admin (RELEASE_TOKEN) para saltar los rulesets (#54)
- Release v0.2.0

### Performance

- *(oidc)* Cachear el JWKS para no pedirlo en cada callback
- *(ui)* Dividir el bundle del panel en chunks por proveedor

### Refactor

- *(ui)* Deriva «en curso» de la mutación, a11y por unidad y ActiveState completo
- *(errores)* Unificar las variantes Persistencia/Registro
- *(ui)* Identidad file_name en TablaDeUnidades

### Testing

- *(ui)* Pruebas de listado y control de quadlets (rojo)
- *(ui)* Nombres accesibles por unidad, estados intermedios y mocks aislados
- Arranque del proceso con SIGTERM y pruebas contra el bus de systemd real
- *(ui)* Usa nombres de unidad systemd en los fixtures
- *(visor-registros)* Autodesplazamiento determinista y contrato del scroll
- *(auth)* Cubrir el descubrimiento caído al refrescar el JWKS
