//! TUIC (TCP/UDP over QUIC) — a single crate supporting both the quinn and
//! quiche (tokio-quiche) backends.
//!
//! The TUIC protocol logic is written once, generic over
//! [`wind_quic::QuicConnection`] (see [`server`] and [`client`]); the backend
//! modules ([`quinn`], [`quiche`]) are thin connection providers selected by
//! the `quinn` / `quiche` features.

pub mod proto;

// Backend-neutral congestion-control config types (defined in `wind-quic`),
// re-exported so config front-ends (e.g. the TUIC server) can build a
// [`CongestionTuning`] and per-algorithm tuning without depending on
// `wind-quic` directly. Available regardless of which backend feature is on.
pub use wind_quic::{Bbr2gcConfig, BbrBwLoReductionStrategy, CongestionTuning};

#[cfg(feature = "server")]
pub mod active;
#[cfg(feature = "server")]
pub use active::ActiveConnections;

#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "quinn")]
pub mod quinn;

#[cfg(feature = "quiche")]
pub mod quiche;

pub type Error = eyre::Report;
pub type Result<T> = eyre::Result<T>;
