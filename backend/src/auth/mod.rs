//! Autenticación OIDC opcional: sesión en cookie, flujo de entrada y *gate*.
//!
//! El módulo se compone de cuatro piezas y ninguna conoce HTTP salvo la última:
//!
//! * [`session`] — la sesión: el JWT que viaja en la cookie y sus atributos.
//! * [`state`] — el estado de autenticación: la caché de descubrimiento y el
//!   registro de `state`/`nonce` de un solo uso.
//! * [`oidc`] — el flujo del proveedor: descubrimiento, URL de autorización,
//!   canje del código y verificación del ID token.
//! * [`middleware`] — el *gate* de la superficie de negocio.
//!
//! Todo él existe **sólo** cuando el modo es `Oidc`: en `Disabled` no se
//! construye ningún [`state::AuthState`] y el *gate* no se monta, de modo que el
//! modo se lee en el tipo y no hay que consultarlo en cada petición.

pub mod middleware;
pub mod oidc;
pub mod session;
pub mod state;

pub use state::AuthState;
