mod config;
mod resolver;

pub use config::{DnsConfig, DnsMode};
pub use resolver::HickoryResolver;

/// Build a [`HickoryResolver`] from the given configuration.
///
/// Returns `None` when `mode = "system"`, signalling that callers should fall
/// back to the OS resolver (e.g. [`wind_core::SystemResolver`]).
pub fn build(cfg: &DnsConfig) -> eyre::Result<Option<HickoryResolver>> {
	resolver::build(cfg)
}
