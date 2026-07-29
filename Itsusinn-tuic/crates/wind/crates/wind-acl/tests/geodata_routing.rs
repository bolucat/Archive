//! End-to-end check that `GEOIP` / `GEOSITE` rules actually match once a
//! geodata database is wired into the engine. Before the geodata was wired in,
//! these rules compiled but never matched (fail-open); this test guards against
//! that regression.

use std::sync::Arc;

use geosite_rs::{Cidr, Domain, GeoIp, GeoIpList, GeoSite, GeoSiteList, encode_geoip, encode_geosite};
use wind_acl::AclEngine;
use wind_core::{RouteAction, Router, types::TargetAddr};
use wind_geodata::GeoData;

fn build_geodata() -> (tempfile::TempPath, Arc<GeoData>) {
	let geosite = GeoSiteList {
		entry: vec![GeoSite {
			country_code: "GOOGLE".to_string(),
			// type 2 = Domain → suffix match (also matches subdomains).
			domain: vec![Domain {
				r#type: 2,
				value: "google.com".to_string(),
				..Default::default()
			}],
		}],
	};
	let geoip = GeoIpList {
		entry: vec![GeoIp {
			country_code: "CN".to_string(),
			cidr: vec![Cidr {
				ip: [1, 2, 3, 0].to_vec(),
				prefix: 24,
			}],
			..Default::default()
		}],
	};
	let tmp = tempfile::NamedTempFile::new().unwrap().into_temp_path();
	let geo = GeoData::build_and_open(&encode_geosite(geosite), &encode_geoip(geoip), &tmp).unwrap();
	(tmp, Arc::new(geo))
}

#[tokio::test]
async fn geoip_rule_matches_when_geodata_is_wired() {
	let (_tmp, geo) = build_geodata();
	let engine = AclEngine::builder("direct")
		.apernet_acl_str("reject(geoip:cn)")
		.unwrap()
		.geodata(geo)
		.build()
		.unwrap();

	// 1.2.3.4 is in the CN block → rejected.
	let cn = TargetAddr::IPv4("1.2.3.4".parse().unwrap(), 443);
	assert!(
		matches!(engine.route(&cn, true).await.unwrap(), RouteAction::Reject(_)),
		"CN IP should be rejected by geoip:cn"
	);

	// 9.9.9.9 is outside CN → falls through to the default outbound.
	let other = TargetAddr::IPv4("9.9.9.9".parse().unwrap(), 443);
	assert!(
		matches!(engine.route(&other, true).await.unwrap(), RouteAction::Forward(o) if o == "direct"),
		"non-CN IP should reach the default outbound"
	);
}

#[tokio::test]
async fn geosite_rule_matches_when_geodata_is_wired() {
	let (_tmp, geo) = build_geodata();
	let engine = AclEngine::builder("direct")
		.apernet_acl_str("reject(geosite:google)")
		.unwrap()
		.geodata(geo)
		.build()
		.unwrap();

	let g = TargetAddr::Domain("mail.google.com".to_string(), 443);
	assert!(
		matches!(engine.route(&g, true).await.unwrap(), RouteAction::Reject(_)),
		"google.com subdomain should be rejected by geosite:google"
	);

	let other = TargetAddr::Domain("example.com".to_string(), 443);
	assert!(
		matches!(engine.route(&other, true).await.unwrap(), RouteAction::Forward(o) if o == "direct"),
		"non-google domain should reach the default outbound"
	);
}

#[tokio::test]
async fn geoip_rule_never_matches_without_geodata() {
	// Without geodata the rule compiles but cannot match: the CN IP falls
	// through to the default outbound (the fail-open the build-time warning
	// flags).
	let engine = AclEngine::builder("direct")
		.apernet_acl_str("reject(geoip:cn)")
		.unwrap()
		.build()
		.unwrap();

	let cn = TargetAddr::IPv4("1.2.3.4".parse().unwrap(), 443);
	assert!(
		matches!(engine.route(&cn, true).await.unwrap(), RouteAction::Forward(o) if o == "direct"),
		"without geodata the geoip rule must not match"
	);
}
