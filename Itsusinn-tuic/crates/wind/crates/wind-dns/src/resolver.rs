use std::{net::IpAddr, pin::Pin, sync::Arc};

use eyre::{Context, Result};
use hickory_resolver::{
	TokioResolver,
	config::{CLOUDFLARE, GOOGLE, LookupIpStrategy, NameServerConfig, QUAD9, ResolverConfig, ResolverOpts},
	net::runtime::TokioRuntimeProvider,
};
use wind_core::{
	StackPrefer,
	resolve::{Resolver, bail_if_no_addrs, filter_addrs_by_preference, pick_addr_by_preference},
};

use crate::config::{DnsConfig, DnsMode};

/// DNS resolver backed by [Hickory DNS](https://github.com/hickory-dns/hickory-dns).
pub struct HickoryResolver {
	inner: TokioResolver,
	prefer: StackPrefer,
}

impl HickoryResolver {
	pub fn new(inner: TokioResolver, prefer: StackPrefer) -> Self {
		Self { inner, prefer }
	}
}

impl Resolver for HickoryResolver {
	fn resolve<'a>(&'a self, host: &'a str) -> Pin<Box<dyn Future<Output = Result<IpAddr>> + Send + 'a>> {
		Box::pin(async move {
			if let Ok(ip) = host.parse::<IpAddr>() {
				return Ok(ip);
			}
			let lookup = self
				.inner
				.lookup_ip(host)
				.await
				.with_context(|| format!("hickory lookup_ip {host}"))?;
			let addrs: Vec<IpAddr> = lookup.iter().collect();
			if addrs.is_empty() {
				eyre::bail!("no DNS records for {host}");
			}
			pick_addr_by_preference(addrs, self.prefer)
				.ok_or_else(|| eyre::eyre!("no address matching {:?} for {host}", self.prefer))
		})
	}

	fn resolve_all<'a>(&'a self, host: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<IpAddr>>> + Send + 'a>> {
		Box::pin(async move {
			if let Ok(ip) = host.parse::<IpAddr>() {
				return Ok(vec![ip]);
			}
			let lookup = self
				.inner
				.lookup_ip(host)
				.await
				.with_context(|| format!("hickory lookup_ip {host}"))?;
			let addrs: Vec<IpAddr> = lookup.iter().collect();
			if addrs.is_empty() {
				eyre::bail!("no DNS records for {host}");
			}
			let addrs = filter_addrs_by_preference(addrs, self.prefer);
			bail_if_no_addrs(&addrs, host, self.prefer)?;
			Ok(addrs)
		})
	}
}

/// Build a [`HickoryResolver`] from the given configuration.
///
/// Returns `None` for `DnsMode::System`.
pub(crate) fn build(cfg: &DnsConfig) -> Result<Option<HickoryResolver>> {
	let rc: ResolverConfig = match cfg.mode {
		DnsMode::System => return Ok(None),
		DnsMode::Cloudflare => ResolverConfig::udp_and_tcp(&CLOUDFLARE),
		DnsMode::CloudflareTls => ResolverConfig::tls(&CLOUDFLARE),
		DnsMode::CloudflareHttps => ResolverConfig::https(&CLOUDFLARE),
		DnsMode::Google => ResolverConfig::udp_and_tcp(&GOOGLE),
		DnsMode::GoogleTls => ResolverConfig::tls(&GOOGLE),
		DnsMode::GoogleHttps => ResolverConfig::https(&GOOGLE),
		DnsMode::Quad9 => ResolverConfig::udp_and_tcp(&QUAD9),
		DnsMode::Quad9Tls => ResolverConfig::tls(&QUAD9),
		DnsMode::Quad9Https => ResolverConfig::https(&QUAD9),
		DnsMode::Custom => {
			if cfg.servers.is_empty() {
				eyre::bail!("[dns] mode = \"custom\" requires at least one entry in `servers`");
			}
			let mut name_servers = Vec::with_capacity(cfg.servers.len());
			for s in &cfg.servers {
				let ns = parse_server(s).with_context(|| format!("parsing DNS server spec {s:?}"))?;
				name_servers.push(ns);
			}
			ResolverConfig::from_parts(None, vec![], name_servers)
		}
	};

	let mut opts = ResolverOpts::default();
	if let Some(t) = cfg.timeout {
		opts.timeout = t;
	}
	if let Some(a) = cfg.attempts {
		opts.attempts = a;
	}
	opts.ip_strategy = match cfg.stack_prefer {
		StackPrefer::V4only => LookupIpStrategy::Ipv4Only,
		StackPrefer::V6only => LookupIpStrategy::Ipv6Only,
		StackPrefer::V4first => LookupIpStrategy::Ipv4thenIpv6,
		StackPrefer::V6first => LookupIpStrategy::Ipv6thenIpv4,
	};

	let resolver = TokioResolver::builder_with_config(rc, TokioRuntimeProvider::default())
		.with_options(opts)
		.build()?;
	Ok(Some(HickoryResolver::new(resolver, cfg.stack_prefer)))
}

/// Transport requested by a server spec.
///
/// A spec without `://` is kept distinct from an explicit `udp://` so that the
/// legacy plaintext form (UDP with a TCP fallback for truncated answers) is not
/// silently narrowed by this change.
enum ServerScheme<'a> {
	Default,
	Named(&'a str),
}

type MakeNameServer = fn(IpAddr, Option<String>) -> NameServerConfig;

/// Parse a DNS server URL or bare address into a [`NameServerConfig`].
fn parse_server(spec: &str) -> Result<NameServerConfig> {
	let (scheme, rest) = match spec.split_once("://") {
		Some((a, b)) => (ServerScheme::Named(a), b),
		None => (ServerScheme::Default, spec),
	};
	// Scheme names stay case-insensitive (`TLS://` keeps working).
	let folded;
	let scheme = match scheme {
		ServerScheme::Named(a) => {
			folded = a.to_ascii_lowercase();
			ServerScheme::Named(&folded)
		}
		ServerScheme::Default => ServerScheme::Default,
	};

	let (addr_part, sni) = match rest.rsplit_once('#') {
		Some((a, b)) => (a, Some(b.to_string())),
		None => (rest, None),
	};

	let (default_port, make_ns): (u16, MakeNameServer) = match scheme {
		// The transport in the spec is authoritative: an explicit `udp://` or
		// `tcp://` opens only that transport, so an operator can pin a
		// plaintext resolver exactly as written. Hickory's pool filters on the
		// configured protocols and tries UDP before TCP, so a `tcp://` spec
		// that also opened UDP previously never actually took effect.
		ServerScheme::Named("udp") => (53u16, |ip, _sni| NameServerConfig::udp(ip)),
		ServerScheme::Named("tcp") => (53u16, |ip, _sni| NameServerConfig::tcp(ip)),
		ServerScheme::Named("tls") => (853u16, |ip, sni| {
			NameServerConfig::tls(ip, Arc::from(sni.unwrap_or_else(|| ip.to_string()).as_str()))
		}),
		ServerScheme::Named("https") => (443u16, |ip, sni| {
			NameServerConfig::https(ip, Arc::from(sni.unwrap_or_else(|| ip.to_string()).as_str()), None)
		}),
		// Historical bare-address behaviour: UDP with a TCP fallback.
		ServerScheme::Default => (53u16, |ip, _sni| NameServerConfig::udp_and_tcp(ip)),
		ServerScheme::Named(other) => eyre::bail!("unknown DNS scheme: {other}"),
	};

	let (ip_str, port) = split_host_port(addr_part, default_port)?;
	let ip: IpAddr = ip_str
		.parse()
		.with_context(|| format!("invalid IP literal in DNS server: {ip_str}"))?;

	let mut ns = make_ns(ip, sni);
	// The constructors fill the protocol's default port. Honour an explicit
	// port from the user spec instead.
	for c in &mut ns.connections {
		c.port = port;
	}
	Ok(ns)
}

fn split_host_port(s: &str, default_port: u16) -> Result<(String, u16)> {
	// Bracketed IPv6: [addr] or [addr]:port
	if let Some(rest) = s.strip_prefix('[') {
		let end = rest.find(']').ok_or_else(|| eyre::eyre!("unclosed IPv6 bracket in {s}"))?;
		let host = &rest[..end];
		let tail = &rest[end + 1..];
		let port = if let Some(p) = tail.strip_prefix(':') {
			p.parse::<u16>().with_context(|| format!("invalid port in {s}"))?
		} else if tail.is_empty() {
			default_port
		} else {
			eyre::bail!("trailing garbage after ']' in {s}");
		};
		return Ok((host.to_string(), port));
	}

	// Bare IPv6 (at least two colons) — cannot carry a port without brackets.
	if s.matches(':').count() >= 2 {
		return Ok((s.to_string(), default_port));
	}

	// "host:port" or "host"
	match s.rsplit_once(':') {
		Some((h, p)) => {
			let port = p.parse::<u16>().with_context(|| format!("invalid port in {s}"))?;
			Ok((h.to_string(), port))
		}
		None => Ok((s.to_string(), default_port)),
	}
}

#[cfg(test)]
mod tests {
	use std::net::{Ipv4Addr, Ipv6Addr};

	use hickory_resolver::config::ProtocolConfig;
	use wind_core::resolve::filter_addrs_by_preference;

	use super::*;

	/// Extract the TLS/HTTPS SNI from a name server's connections, if any.
	fn sni(ns: &NameServerConfig) -> Option<&str> {
		ns.connections.iter().find_map(|c| match &c.protocol {
			ProtocolConfig::Tls { server_name } | ProtocolConfig::Https { server_name, .. } => Some(server_name.as_ref()),
			_ => None,
		})
	}

	/// The transports opened for a name server, as a comparable list.
	fn transports(ns: &NameServerConfig) -> Vec<String> {
		ns.connections
			.iter()
			.map(|c| match &c.protocol {
				ProtocolConfig::Udp => String::from("udp"),
				ProtocolConfig::Tcp => String::from("tcp"),
				ProtocolConfig::Tls { .. } => String::from("tls"),
				ProtocolConfig::Https { .. } => String::from("https"),
			})
			.collect()
	}

	#[test]
	fn parse_bare_ipv4() {
		let ns = parse_server("1.1.1.1").unwrap();
		assert_eq!(ns.ip, IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
		// A scheme-less spec keeps the legacy `udp_and_tcp` pair.
		assert_eq!(transports(&ns), ["udp", "tcp"]);
		assert!(ns.connections.iter().all(|c| c.port == 53));
		assert!(ns.connections.iter().any(|c| matches!(c.protocol, ProtocolConfig::Udp)));
	}

	#[test]
	fn explicit_udp_opens_only_udp() {
		// `udp://` must not silently open a TCP fallback: the pool would
		// otherwise be free to answer over TCP as well.
		let ns = parse_server("udp://1.1.1.1:5353").unwrap();
		assert_eq!(ns.ip, IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
		assert_eq!(transports(&ns), ["udp"]);
		assert_eq!(ns.connections[0].port, 5353);
	}

	#[test]
	fn explicit_tcp_opens_only_tcp() {
		// Regression: a `tcp://` spec used to produce `[udp, tcp]`, and
		// Hickory's pool sorts UDP first, so the request was actually sent
		// over UDP and the spec had no effect.
		let ns = parse_server("tcp://1.1.1.1:5353").unwrap();
		assert_eq!(transports(&ns), ["tcp"]);
		assert_eq!(ns.connections[0].port, 5353);
	}

	#[test]
	fn scheme_is_case_insensitive() {
		assert_eq!(transports(&parse_server("UDP://1.1.1.1").unwrap()), ["udp"]);
		assert_eq!(transports(&parse_server("TCP://1.1.1.1").unwrap()), ["tcp"]);
	}

	#[test]
	fn parse_dot_with_sni() {
		let ns = parse_server("tls://1.1.1.1#cloudflare-dns.com").unwrap();
		assert_eq!(ns.ip, IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
		assert_eq!(ns.connections.len(), 1);
		assert_eq!(ns.connections[0].port, 853);
		assert_eq!(sni(&ns), Some("cloudflare-dns.com"));
	}

	#[test]
	fn parse_doh_bracketed_ipv6() {
		let ns = parse_server("https://[2606:4700:4700::1111]:443#cloudflare-dns.com").unwrap();
		assert_eq!(ns.ip, IpAddr::V6("2606:4700:4700::1111".parse::<Ipv6Addr>().unwrap()));
		assert_eq!(ns.connections.len(), 1);
		assert_eq!(ns.connections[0].port, 443);
		assert!(matches!(ns.connections[0].protocol, ProtocolConfig::Https { .. }));
		assert_eq!(sni(&ns), Some("cloudflare-dns.com"));
	}

	#[test]
	fn parse_bare_ipv6() {
		let ns = parse_server("2606:4700:4700::1111").unwrap();
		assert!(ns.connections.iter().all(|c| c.port == 53));
	}

	#[test]
	fn parse_rejects_bad_scheme() {
		assert!(parse_server("ftp://1.1.1.1").is_err());
	}

	#[test]
	fn build_system_returns_none() {
		let cfg = DnsConfig {
			mode: DnsMode::System,
			..DnsConfig::default()
		};
		let result = build(&cfg).unwrap();
		assert!(result.is_none());
	}

	#[tokio::test]
	async fn resolve_all_errors_when_the_preference_filter_empties_the_list() {
		// Regression W53: `HickoryResolver::resolve_all` returned
		// `Ok(filter_addrs_by_preference(..))` as-is, so a host that only
		// resolved to the other family produced `Ok(vec![])` —
		// indistinguishable from a successful lookup with no addresses.
		let v4_only: Vec<IpAddr> = vec![IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))];
		let filtered = filter_addrs_by_preference(v4_only, StackPrefer::V6only);
		assert!(filtered.is_empty());
		let err = bail_if_no_addrs(&filtered, "example.com", StackPrefer::V6only).unwrap_err();
		assert!(
			err.to_string().contains("no address matching V6only"),
			"unexpected message: {err}"
		);
	}
}
