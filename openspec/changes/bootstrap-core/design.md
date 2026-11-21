# Design — bootstrap-core

Issue de entrada: **#1**.

## Context

podpot es nuevo. El objetivo es un panel web para gestionar las **quadlets de Podman** de la instancia rootless de systemd (`systemctl --user`): ver el estado y controlar cada contenedor, y (en un cambio posterior) crear quadlets nuevas. Una quadlet es una unidad systemd, así que el estado autoritativo vive en **systemd**, no en Podman: la API de Podman no puede accionar una quadlet.

## Goals / Non-goals

- **Goals**: esqueleto caminante con estado y control de quadlets, de extremo a extremo (backend + D-Bus + API + UI + contenedor).
- **Non-goals**: creación de quadlets nuevas; autenticación OIDC (queda el bind a loopback); SSE/`PropertiesChanged`.

## Decisions

### D-Bus directo (`zbus`), no shell-out

Se habla con `org.freedesktop.systemd1` por D-Bus en lugar de invocar `systemctl`. Da estado **tipado** (sin parsear texto), permite `PropertiesChanged` más adelante (SSE) y no carga el binario `systemctl` en la imagen. Contrapartida asumida: nombres de unidad **escapados** y el modelo de **jobs** asíncronos. Se sacrifica reutilizar la lógica shell-out de `qctl`; lo que sí se hereda de allí es la convención de nombres de unidad y el descubrimiento.

### Modelo de jobs

`StartUnit`/`StopUnit`/`RestartUnit` devuelven un *job* (`/org/freedesktop/systemd1/job/<id>`). La capa esperará el fin del job (o consultará el estado tras él) antes de dar por concluida la acción; nunca asumirá éxito inmediato.

### Modelo de datos

La primera entrega **no** introduce entidades de negocio: el estado de las unidades vive en systemd y las quadlets en disco. La persistencia (pool + migraciones + WAL) se monta ahora como infraestructura, con una primera migración mínima (ajustes/registro de acciones) para dejar el camino probado. El esquema detallado se fija en la fase de implementación.

### Imagen y despliegue

Contenedor rootless. Hallazgo del *spike* (2026-11-21): el bus de usuario solo responde con `--userns=keep-id` (`/run/user/<uid>/bus`); sin el uid coincidente, `Did not receive a reply`. Por eso la imagen **bakea** sus dependencias y corre como usuario no-root (con keep-id no hay root para `apk`). Se montan bus de usuario, `~/.config/containers/systemd` y socket de Podman. Requiere `loginctl enable-linger` en el host.

## Risks / Trade-offs

- **Portabilidad**: la variabilidad de systemd/Podman entre hosts. Mitigación: contrato D-Bus detrás de un trait para poder inyectar un doble en las pruebas.
- **Fragilidad del contenedor**: los montajes son la parte delicada; documentados en `AGENTS.md` y verificados por el smoke test del despliegue.
