# podpot

Panel web para gestionar **quadlets de Podman** de la instancia rootless de systemd.

- **Estado de cada contenedor** y control (start / stop / restart / …).
- **Creación** de quadlets nuevas.

Backend en **Rust + Axum** (`backend/`), frontend en **React + Ant Design** (`frontend/`).
Habla con systemd por **D-Bus** (`org.freedesktop.systemd1`); nunca por shell-out.

> Proyecto en construcción. Arquitectura y convenciones en [AGENTS.md](AGENTS.md).
Trigger release workflow
