use std::{
	future::Future,
	net::{IpAddr, SocketAddr},
	pin::Pin,
};

use crate::{types::TargetAddr, utils::StackPrefer};

/// Trait for asynchronous DNS resolution.
///
/// Implementations can back this with system DNS, Hickory DNS, or any other
/// resolver. The trait is object-safe so it can be stored as
/// `Arc<dyn Resolver>`.
pub trait Resolver: Send + Sync + 'static {
	/// Resolve `host` to a single [`IpAddr`].
	///
	/// Literal IP addresses bypass the resolver and are returned directly.
	fn resolve<'a>(&'a self, host: &'a str) -> Pin<Box<dyn Future<Output = eyre::Result<IpAddr>> + Send + 'a>>;

	/// Resolve `host` to every available [`IpAddr`].
	///
	/// Literal IP addresses yield a single-entry vector.
	fn resolve_all<'a>(&'a self, host: &'a str) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<IpAddr>>> + Send + 'a>>;
}

/// System DNS resolver backed by [`tokio::net::lookup_host`].
///
/// When the hostname is a literal IP address it is returned immediately
/// without hitting the network.
pub struct SystemResolver {
	pub prefer: StackPrefer,
}

impl SystemResolver {
	pub fn new(prefer: StackPrefer) -> Self {
		Self { prefer }
	}
}

impl Resolver for SystemResolver {
	fn resolve<'a>(&'a self, host: &'a str) -> Pin<Box<dyn Future<Output = eyre::Result<IpAddr>> + Send + 'a>> {
		Box::pin(async move {
			if let Ok(ip) = host.parse::<IpAddr>() {
				return Ok(ip);
			}
			let addrs: Vec<IpAddr> = tokio::net::lookup_host(format!("{host}:0")).await?.map(|s| s.ip()).collect();
			if addrs.is_empty() {
				eyre::bail!("no DNS records for {host}");
			}
			pick_addr_by_preference(addrs, self.prefer)
				.ok_or_else(|| eyre::eyre!("no address matching {:?} for {host}", self.prefer))
		})
	}

	fn resolve_all<'a>(&'a self, host: &'a str) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<IpAddr>>> + Send + 'a>> {
		Box::pin(async move {
			if let Ok(ip) = host.parse::<IpAddr>() {
				return Ok(vec![ip]);
			}
			let addrs: Vec<IpAddr> = tokio::net::lookup_host(format!("{host}:0")).await?.map(|s| s.ip()).collect();
			if addrs.is_empty() {
				eyre::bail!("no DNS records for {host}");
			}
			Ok(filter_addrs_by_preference(addrs, self.prefer))
		})
	}
}

/// Pick the best address from a resolved list according to [`StackPrefer`].
pub fn pick_addr_by_preference(addrs: Vec<IpAddr>, prefer: StackPrefer) -> Option<IpAddr> {
	let v4 = || addrs.iter().copied().filter(|a| a.is_ipv4());
	let v6 = || addrs.iter().copied().filter(|a| a.is_ipv6());

	match prefer {
		StackPrefer::V4only => v4().next(),
		StackPrefer::V6only => v6().next(),
		StackPrefer::V4first => v4().next().or_else(|| v6().next()),
		StackPrefer::V6first => v6().next().or_else(|| v4().next()),
	}
}

/// Filter addresses according to [`StackPrefer`], preserving order within the
/// preferred family.
pub fn filter_addrs_by_preference(addrs: Vec<IpAddr>, prefer: StackPrefer) -> Vec<IpAddr> {
	match prefer {
		StackPrefer::V4only => addrs.into_iter().filter(|a| a.is_ipv4()).collect(),
		StackPrefer::V6only => addrs.into_iter().filter(|a| a.is_ipv6()).collect(),
		StackPrefer::V4first => {
			let (mut v4, v6): (Vec<_>, Vec<_>) = addrs.into_iter().partition(|a| a.is_ipv4());
			v4.extend(v6);
			v4
		}
		StackPrefer::V6first => {
			let (v4, mut v6): (Vec<_>, Vec<_>) = addrs.into_iter().partition(|a| a.is_ipv4());
			v6.extend(v4);
			v6
		}
	}
}

/// Resolve a `TargetAddr` to a single `SocketAddr` using the given resolver.
///
/// The resolver's built-in preference is used — call
/// [`resolve_target_with_preference`] if a per-outbound override is needed.
pub async fn resolve_target(target: &TargetAddr, resolver: &dyn Resolver) -> eyre::Result<SocketAddr> {
	match target {
		TargetAddr::IPv4(ip, port) => Ok(SocketAddr::from((*ip, *port))),
		TargetAddr::IPv6(ip, port) => Ok(SocketAddr::from((*ip, *port))),
		TargetAddr::Domain(domain, port) => Ok(SocketAddr::new(resolver.resolve(domain).await?, *port)),
	}
}

/// Resolve a `TargetAddr` to a single `SocketAddr`, optionally overriding
/// the resolver's built-in IP stack preference.
///
/// When `prefer` is `Some`, domain names are resolved via
/// [`Resolver::resolve_all`] and the best address is picked with
/// [`pick_addr_by_preference`].  This lets individual outbounds select a
/// different IP family than the global resolver default (mirroring mihomo's
/// per-outbound `ip-version`).
///
/// When `prefer` is `None`, the behaviour is identical to
/// [`resolve_target`].
pub async fn resolve_target_with_preference(
	target: &TargetAddr,
	resolver: &dyn Resolver,
	prefer: Option<StackPrefer>,
) -> eyre::Result<SocketAddr> {
	let Some(prefer) = prefer else {
		return resolve_target(target, resolver).await;
	};
	match target {
		TargetAddr::IPv4(ip, port) => Ok(SocketAddr::from((*ip, *port))),
		TargetAddr::IPv6(ip, port) => Ok(SocketAddr::from((*ip, *port))),
		TargetAddr::Domain(domain, port) => {
			let addrs = resolver.resolve_all(domain).await?;
			let picked = pick_addr_by_preference(addrs, prefer)
				.ok_or_else(|| eyre::eyre!("no address matching {:?} for {domain}", prefer))?;
			Ok(SocketAddr::new(picked, *port))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn ips(list: &[&str]) -> Vec<IpAddr> {
		list.iter().map(|s| s.parse().unwrap()).collect()
	}

	#[test]
	fn pick_only_modes_require_matching_family() {
		let mixed = ips(&["192.168.1.1", "2001:db8::1"]);
		assert!(pick_addr_by_preference(mixed.clone(), StackPrefer::V4only).unwrap().is_ipv4());
		assert!(pick_addr_by_preference(mixed, StackPrefer::V6only).unwrap().is_ipv6());

		assert!(pick_addr_by_preference(ips(&["2001:db8::1"]), StackPrefer::V4only).is_none());
		assert!(pick_addr_by_preference(ips(&["192.168.1.1"]), StackPrefer::V6only).is_none());
	}

	#[test]
	fn pick_first_modes_fall_back_to_other_family() {
		assert!(
			pick_addr_by_preference(ips(&["2001:db8::1", "192.168.1.1"]), StackPrefer::V4first)
				.unwrap()
				.is_ipv4()
		);
		// V4first with no IPv4 falls back to IPv6.
		assert!(
			pick_addr_by_preference(ips(&["2001:db8::1"]), StackPrefer::V4first)
				.unwrap()
				.is_ipv6()
		);

		assert!(
			pick_addr_by_preference(ips(&["192.168.1.1", "2001:db8::1"]), StackPrefer::V6first)
				.unwrap()
				.is_ipv6()
		);
		// V6first with no IPv6 falls back to IPv4.
		assert!(
			pick_addr_by_preference(ips(&["192.168.1.1"]), StackPrefer::V6first)
				.unwrap()
				.is_ipv4()
		);
	}

	#[test]
	fn pick_empty_list_is_none() {
		assert!(pick_addr_by_preference(vec![], StackPrefer::V4first).is_none());
	}

	#[test]
	fn filter_only_modes_keep_a_single_family() {
		let addrs = ips(&["192.168.1.1", "2001:db8::1", "10.0.0.1"]);
		let v4 = filter_addrs_by_preference(addrs.clone(), StackPrefer::V4only);
		assert_eq!(v4, ips(&["192.168.1.1", "10.0.0.1"]));
		let v6 = filter_addrs_by_preference(addrs, StackPrefer::V6only);
		assert_eq!(v6, ips(&["2001:db8::1"]));
	}

	#[test]
	fn filter_first_modes_group_preferred_family_first_preserving_order() {
		let addrs = ips(&["2001:db8::1", "192.168.1.1", "::1", "10.0.0.1"]);
		assert_eq!(
			filter_addrs_by_preference(addrs.clone(), StackPrefer::V4first),
			ips(&["192.168.1.1", "10.0.0.1", "2001:db8::1", "::1"]),
		);
		assert_eq!(
			filter_addrs_by_preference(addrs, StackPrefer::V6first),
			ips(&["2001:db8::1", "::1", "192.168.1.1", "10.0.0.1"]),
		);
	}
}
