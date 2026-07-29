pub mod tls;
pub mod utils;

pub use utils::{CongestionControl, UdpRelayMode};

#[cfg(feature = "server")]
pub mod inbound;

#[cfg(feature = "client")]
pub mod outbound;

pub type Error = eyre::Report;
pub type Result<T> = eyre::Result<T>;
