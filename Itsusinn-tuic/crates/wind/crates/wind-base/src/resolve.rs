use std::net::SocketAddr;

use wind_core::{resolve::Resolver, types::TargetAddr};

/// Resolve a `TargetAddr` to a single `SocketAddr` using the given resolver.
pub async fn resolve_target(target: &TargetAddr, resolver: &dyn Resolver) -> eyre::Result<SocketAddr> {
	match target {
		TargetAddr::IPv4(ip, port) => Ok(SocketAddr::from((*ip, *port))),
		TargetAddr::IPv6(ip, port) => Ok(SocketAddr::from((*ip, *port))),
		TargetAddr::Domain(domain, port) => Ok(SocketAddr::new(resolver.resolve(domain).await?, *port)),
	}
}

#[cfg(test)]
mod tests {
	use std::{future::Future, net::IpAddr, pin::Pin};

	use super::*;

	/// Resolver that returns a fixed IP for any host.
	struct FixedResolver(IpAddr);

	impl Resolver for FixedResolver {
		fn resolve<'a>(&'a self, _host: &'a str) -> Pin<Box<dyn Future<Output = eyre::Result<IpAddr>> + Send + 'a>> {
			let ip = self.0;
			Box::pin(async move { Ok(ip) })
		}

		fn resolve_all<'a>(&'a self, _host: &'a str) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<IpAddr>>> + Send + 'a>> {
			let ip = self.0;
			Box::pin(async move { Ok(vec![ip]) })
		}
	}

	/// Resolver that panics if used — proves IP-literal targets never hit DNS.
	struct PanicResolver;

	impl Resolver for PanicResolver {
		fn resolve<'a>(&'a self, _host: &'a str) -> Pin<Box<dyn Future<Output = eyre::Result<IpAddr>> + Send + 'a>> {
			Box::pin(async { panic!("resolver must not be called for IP-literal targets") })
		}

		fn resolve_all<'a>(&'a self, _host: &'a str) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<IpAddr>>> + Send + 'a>> {
			Box::pin(async { panic!("resolver must not be called for IP-literal targets") })
		}
	}

	#[tokio::test]
	async fn ip_literal_targets_bypass_the_resolver() {
		let v4 = resolve_target(&TargetAddr::IPv4("192.168.1.1".parse().unwrap(), 8080), &PanicResolver)
			.await
			.unwrap();
		assert_eq!(v4.to_string(), "192.168.1.1:8080");

		let v6 = resolve_target(&TargetAddr::IPv6("::1".parse().unwrap(), 443), &PanicResolver)
			.await
			.unwrap();
		assert!(v6.ip().is_ipv6());
		assert_eq!(v6.port(), 443);
	}

	#[tokio::test]
	async fn domain_targets_use_the_resolver_and_keep_the_port() {
		let resolver = FixedResolver("203.0.113.7".parse().unwrap());
		let s = resolve_target(&TargetAddr::Domain("example.com".into(), 443), &resolver)
			.await
			.unwrap();
		assert_eq!(s.to_string(), "203.0.113.7:443");
	}
}
