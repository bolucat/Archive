//! Shared utilities for Nyanpasu applications and services.

#[cfg(feature = "core_manager")]
#[macro_use]
extern crate derive_builder;

#[cfg(feature = "core_manager")]
pub mod core;

pub mod io;

pub mod runtime;

pub use nyanpasu_log_preview as log_preview;

#[cfg(feature = "reqwest")]
pub mod reqwest_ext;

#[cfg(feature = "dirs")]
pub mod dirs;

#[cfg(feature = "os")]
pub mod os;

#[cfg(feature = "network")]
pub mod network;

#[cfg(feature = "process")]
pub mod process;
