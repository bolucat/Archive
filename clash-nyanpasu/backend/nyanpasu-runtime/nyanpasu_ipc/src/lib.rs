#[macro_use]
extern crate derive_builder;

pub mod api;
#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;
pub mod types;
pub mod utils;

pub const SERVICE_PLACEHOLDER: &str = "nyanpasu_ipc";
