//! podpot: panel web para gestionar las **quadlets de Podman** de la instancia
//! **rootless** de systemd (`systemctl --user`).
//!
//! El crate es una biblioteca además de un binario por un motivo concreto: el
//! binario (`src/main.rs`) es sólo la composición —configuración, registro,
//! base, cliente de systemd, escucha— y toda la lógica vive aquí, donde las
//! pruebas pueden alcanzarla sin levantar un proceso.
//!
//! Los módulos públicos fijan el contrato de la aplicación:
//!
//! * [`config`] — la configuración leída del entorno y sus reglas de arranque.
//! * [`error`] — el error tipado de la aplicación y su traducción a HTTP.
//! * [`api`] — el servidor HTTP: envoltura JSON, rutas y manejadores.
//! * [`auth`] — la autenticación OIDC: sesión, flujo de entrada y *gate*.
//! * [`fanout`] — el fan-out: convierte avisos de cambio en instantáneas publicadas.
//! * [`quadlet`] — la creación de quadlets nuevas: dominio, escritura y registro.
//! * [`state`] — el estado compartido que se inyecta en los manejadores.
//! * [`server`] — la escucha y el apagado ordenado.

#![forbid(unsafe_code)]

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod fanout;
pub mod quadlet;
pub mod secret;
pub mod server;
pub mod state;
pub mod systemd;

#[cfg(test)]
mod test_support;
