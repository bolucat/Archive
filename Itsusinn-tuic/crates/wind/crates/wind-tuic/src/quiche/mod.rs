//! quiche (tokio-quiche) backend.
//!
//! Thin connection-provider wrapper around [`wind_quic::quiche`]: it constructs
//! the listener/connection and runs the shared TUIC protocol core
//! ([`crate::server`] / [`crate::client`] / [`crate::proto`]). Public types
//! mirror the former `wind-tuiche` crate so consumers only change their import
//! path.

pub mod utils;

pub use utils::{CongestionControl, ConnectionOpts, UdpRelayMode};
/// Hot-swappable certificate store for live cert rotation (re-exported from
/// `wind-quic`, where the boring-based hot-reload hook lives).
pub use wind_quic::quiche::CertStore;

#[cfg(feature = "server")]
pub mod inbound;

#[cfg(feature = "client")]
pub mod outbound;

#[cfg(feature = "server")]
pub use inbound::{TuicheInbound, TuicheInboundBuilder};
#[cfg(feature = "client")]
pub use outbound::{TuicheOutbound, TuicheOutboundBuilder};
