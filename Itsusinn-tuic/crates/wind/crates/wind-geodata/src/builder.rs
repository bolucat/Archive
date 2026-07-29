use std::collections::BTreeMap;

use geosite_rs::{GeoIpList, GeoSiteList, decode_geoip, decode_geosite};

use crate::snapshot::*;

pub fn build_snapshot(geosite_bytes: &[u8], geoip_bytes: &[u8]) -> Result<GeoDataSnapshot, crate::error::GeoDataError> {
	let geosite_list = decode_geosite(geosite_bytes).map_err(|e| crate::error::GeoDataError::Decode(e.to_string()))?;
	let geoip_list = decode_geoip(geoip_bytes).map_err(|e| crate::error::GeoDataError::Decode(e.to_string()))?;

	let geosite = build_geosite(&geosite_list);
	let geoip = build_geoip(&geoip_list);

	Ok(GeoDataSnapshot { geosite, geoip })
}

fn build_geosite(list: &GeoSiteList) -> GeoSiteIndex {
	let mut categories: Vec<CategoryInfo> = Vec::new();
	let mut exact_domains: Vec<String> = Vec::new();
	let mut suffix_domains: Vec<String> = Vec::new();
	let mut keyword_domains: Vec<String> = Vec::new();

	for site in &list.entry {
		let mut exact: Vec<String> = Vec::new();
		let mut suffix: Vec<String> = Vec::new();
		let mut keyword: Vec<String> = Vec::new();

		for domain in &site.domain {
			let value = domain.value.to_ascii_lowercase();
			match domain.r#type {
				0 => keyword.push(value), // Plain → keyword (substring) match
				1 => {}                   // Regex → skip (not in flat arrays)
				2 => suffix.push(value),  // Domain → suffix match
				3 => exact.push(value),   // Full → exact match
				_ => {}
			}
		}

		exact.sort();
		exact.dedup();
		suffix.sort();
		suffix.dedup();
		keyword.sort();
		keyword.dedup();

		let exact_start = exact_domains.len() as u32;
		let exact_len = exact.len() as u32;
		let suffix_start = suffix_domains.len() as u32;
		let suffix_len = suffix.len() as u32;
		let keyword_start = keyword_domains.len() as u32;
		let keyword_len = keyword.len() as u32;

		exact_domains.extend(exact);
		suffix_domains.extend(suffix);
		keyword_domains.extend(keyword);

		categories.push(CategoryInfo {
			// Stored uppercase so lookups can be case-insensitive (geosite.dat tags are uppercase).
			name: site.country_code.to_ascii_uppercase(),
			exact_start,
			exact_len,
			suffix_start,
			suffix_len,
			keyword_start,
			keyword_len,
		});
	}

	// Sort categories by name for binary search.
	categories.sort_by(|a, b| a.name.cmp(&b.name));

	GeoSiteIndex {
		categories,
		exact_domains,
		suffix_domains,
		keyword_domains,
	}
}

fn build_geoip(list: &GeoIpList) -> GeoIpIndex {
	// Accumulate ranges per country (uppercased). A BTreeMap keeps names sorted and
	// merges duplicate country entries deterministically.
	let mut v4_by_country: BTreeMap<String, Vec<(u32, u32)>> = BTreeMap::new();
	let mut v6_by_country: BTreeMap<String, Vec<(u128, u128)>> = BTreeMap::new();

	for geoip in &list.entry {
		let name = geoip.country_code.to_ascii_uppercase();
		// Ensure the country shows up even if it only has one address family.
		v4_by_country.entry(name.clone()).or_default();
		v6_by_country.entry(name.clone()).or_default();

		for cidr in &geoip.cidr {
			match cidr.ip.len() {
				4 => {
					let addr = u32::from_be_bytes(cidr.ip[..4].try_into().unwrap());
					let prefix = cidr.prefix.min(32) as u8;
					v4_by_country.get_mut(&name).unwrap().push(v4_range(addr, prefix));
				}
				16 => {
					let addr = u128::from_be_bytes(cidr.ip[..16].try_into().unwrap());
					let prefix = cidr.prefix.min(128) as u8;
					v6_by_country.get_mut(&name).unwrap().push(v6_range(addr, prefix));
				}
				_ => {}
			}
		}
	}

	let mut countries: Vec<CountryInfo> = Vec::with_capacity(v4_by_country.len());
	let mut v4_ranges: Vec<RangeV4> = Vec::new();
	let mut v6_ranges: Vec<RangeV6> = Vec::new();

	// BTreeMap iteration is sorted by key, so `countries` ends up sorted by name.
	for (name, v4) in v4_by_country {
		let v6 = v6_by_country.remove(&name).unwrap_or_default();
		let v4 = merge_ranges_v4(v4);
		let v6 = merge_ranges_v6(v6);

		let v4_start = v4_ranges.len() as u32;
		let v4_len = v4.len() as u32;
		let v6_start = v6_ranges.len() as u32;
		let v6_len = v6.len() as u32;

		v4_ranges.extend(v4.into_iter().map(|(start, end)| RangeV4 { start, end }));
		v6_ranges.extend(v6.into_iter().map(|(start, end)| RangeV6 { start, end }));

		countries.push(CountryInfo {
			name,
			v4_start,
			v4_len,
			v6_start,
			v6_len,
		});
	}

	GeoIpIndex {
		countries,
		v4_ranges,
		v6_ranges,
	}
}

/// Inclusive `[start, end]` range covered by an IPv4 CIDR.
fn v4_range(addr: u32, prefix: u8) -> (u32, u32) {
	if prefix == 0 {
		return (0, u32::MAX);
	}
	let mask = u32::MAX << (32 - prefix as u32);
	let start = addr & mask;
	(start, start | !mask)
}

/// Inclusive `[start, end]` range covered by an IPv6 CIDR.
fn v6_range(addr: u128, prefix: u8) -> (u128, u128) {
	if prefix == 0 {
		return (0, u128::MAX);
	}
	let mask = u128::MAX << (128 - prefix as u32);
	let start = addr & mask;
	(start, start | !mask)
}

/// Sort and merge overlapping/adjacent ranges so the result is disjoint.
fn merge_ranges_v4(mut ranges: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
	ranges.sort_unstable();
	let mut merged: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
	for (start, end) in ranges {
		match merged.last_mut() {
			Some(last) if start <= last.1.saturating_add(1) => {
				if end > last.1 {
					last.1 = end;
				}
			}
			_ => merged.push((start, end)),
		}
	}
	merged
}

/// Sort and merge overlapping/adjacent ranges so the result is disjoint.
fn merge_ranges_v6(mut ranges: Vec<(u128, u128)>) -> Vec<(u128, u128)> {
	ranges.sort_unstable();
	let mut merged: Vec<(u128, u128)> = Vec::with_capacity(ranges.len());
	for (start, end) in ranges {
		match merged.last_mut() {
			Some(last) if start <= last.1.saturating_add(1) => {
				if end > last.1 {
					last.1 = end;
				}
			}
			_ => merged.push((start, end)),
		}
	}
	merged
}
