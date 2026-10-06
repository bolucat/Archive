use std::collections::BTreeMap;

use geosite_rs::{GeoIpList, GeoSiteList, decode_geoip, decode_geosite};

use crate::{error::GeoDataError, snapshot::*};

pub fn build_snapshot(geosite_bytes: &[u8], geoip_bytes: &[u8]) -> Result<GeoDataSnapshot, GeoDataError> {
	let geosite_list = decode_geosite(geosite_bytes).map_err(|e| GeoDataError::Decode(e.to_string()))?;
	let geoip_list = decode_geoip(geoip_bytes).map_err(|e| GeoDataError::Decode(e.to_string()))?;

	let geosite = build_geosite(&geosite_list)?;
	let geoip = build_geoip(&geoip_list)?;

	Ok(GeoDataSnapshot { geosite, geoip })
}

/// Convert a slice offset or length into the `u32` the snapshot layout stores.
///
/// `CategoryInfo`/`CountryInfo` describe their slice as `[start, start + len)`
/// with both parts typed `u32`, so a dataset with more than `u32::MAX` entries
/// in one of the backing vectors would wrap around: the recorded slice would
/// point at the wrong entries, and `validate_offsets` would then reject a cache
/// that this very builder had just written. Refuse to build such a snapshot
/// instead of truncating silently.
fn checked_index(value: usize, what: &str) -> Result<u32, GeoDataError> {
	u32::try_from(value).map_err(|_| {
		GeoDataError::IndexOverflow(format!(
			"{what} holds {value} entries; the snapshot index stores offsets and lengths as u32 (max {})",
			u32::MAX
		))
	})
}

/// Domains of one geosite category, split by the match type they compile to.
#[derive(Default)]
struct SiteDomains {
	exact: Vec<String>,
	suffix: Vec<String>,
	keyword: Vec<String>,
}

fn build_geosite(list: &GeoSiteList) -> Result<GeoSiteIndex, GeoDataError> {
	// Accumulate domains per category (uppercased). A BTreeMap keeps the
	// categories sorted for the binary search and merges entries that share a
	// tag, so every domain of a duplicated tag stays reachable.
	let mut by_name: BTreeMap<String, SiteDomains> = BTreeMap::new();

	for site in &list.entry {
		let entry = by_name.entry(site.country_code.to_ascii_uppercase()).or_default();

		for domain in &site.domain {
			let value = domain.value.to_ascii_lowercase();
			match domain.r#type {
				0 => entry.keyword.push(value), // Plain → keyword (substring) match
				1 => {}                         // Regex → skip (not in flat arrays)
				2 => entry.suffix.push(value),  // Domain → suffix match
				3 => entry.exact.push(value),   // Full → exact match
				_ => {}
			}
		}
	}

	let mut categories: Vec<CategoryInfo> = Vec::with_capacity(by_name.len());
	let mut exact_domains: Vec<String> = Vec::new();
	let mut suffix_domains: Vec<String> = Vec::new();
	let mut keyword_domains: Vec<String> = Vec::new();

	// BTreeMap iteration is sorted by key, so `categories` ends up sorted by
	// name for the binary search.
	for (name, domains) in by_name {
		let SiteDomains {
			mut exact,
			mut suffix,
			mut keyword,
		} = domains;
		exact.sort();
		exact.dedup();
		suffix.sort();
		suffix.dedup();
		keyword.sort();
		keyword.dedup();

		let exact_start = checked_index(exact_domains.len(), "geosite exact domains")?;
		let exact_len = checked_index(exact.len(), "geosite exact domains")?;
		let suffix_start = checked_index(suffix_domains.len(), "geosite suffix domains")?;
		let suffix_len = checked_index(suffix.len(), "geosite suffix domains")?;
		let keyword_start = checked_index(keyword_domains.len(), "geosite keyword domains")?;
		let keyword_len = checked_index(keyword.len(), "geosite keyword domains")?;

		exact_domains.extend(exact);
		suffix_domains.extend(suffix);
		keyword_domains.extend(keyword);

		categories.push(CategoryInfo {
			// Stored uppercase so lookups can be case-insensitive (geosite.dat tags are uppercase).
			name,
			exact_start,
			exact_len,
			suffix_start,
			suffix_len,
			keyword_start,
			keyword_len,
		});
	}

	Ok(GeoSiteIndex {
		categories,
		exact_domains,
		suffix_domains,
		keyword_domains,
	})
}

fn build_geoip(list: &GeoIpList) -> Result<GeoIpIndex, GeoDataError> {
	// Accumulate ranges per country (uppercased). A BTreeMap keeps names sorted
	// and merges duplicate country entries deterministically.
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

	// BTreeMap iteration is sorted by key, so `countries` ends up sorted by
	// name.
	for (name, v4) in v4_by_country {
		let v6 = v6_by_country.remove(&name).unwrap_or_default();
		let v4 = merge_ranges_v4(v4);
		let v6 = merge_ranges_v6(v6);

		let v4_start = checked_index(v4_ranges.len(), "geoip v4 ranges")?;
		let v4_len = checked_index(v4.len(), "geoip v4 ranges")?;
		let v6_start = checked_index(v6_ranges.len(), "geoip v6 ranges")?;
		let v6_len = checked_index(v6.len(), "geoip v6 ranges")?;

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

	Ok(GeoIpIndex {
		countries,
		v4_ranges,
		v6_ranges,
	})
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

#[cfg(test)]
mod tests {
	use geosite_rs::{Cidr, Domain, GeoIp, GeoIpList, GeoSite, GeoSiteList, encode_geoip, encode_geosite};

	use super::*;

	fn domain(r#type: i32, value: &str) -> Domain {
		Domain {
			r#type,
			value: value.to_string(),
			..Default::default()
		}
	}

	/// The `[start, start + len)` slice a `CategoryInfo`/`CountryInfo` records,
	/// as strings/ranges. Asserts the slice really is inside its backing vector
	/// so a truncated or wrapped index surfaces here instead of silently
	/// returning different entries.
	fn strings(v: &[String], start: u32, len: u32) -> Vec<&str> {
		let start = start as usize;
		let end = start + len as usize;
		assert!(
			end <= v.len(),
			"slice [{start}, {end}) escapes a backing vector of len {}",
			v.len()
		);
		v[start..end].iter().map(String::as_str).collect()
	}

	fn v4_ranges(v: &[RangeV4], start: u32, len: u32) -> Vec<(u32, u32)> {
		let start = start as usize;
		let end = start + len as usize;
		assert!(
			end <= v.len(),
			"slice [{start}, {end}) escapes a backing vector of len {}",
			v.len()
		);
		v[start..end].iter().map(|r| (r.start, r.end)).collect()
	}

	#[test]
	fn offsets_that_do_not_fit_the_u32_index_are_rejected() {
		// Boundary: the largest representable index is still accepted.
		assert_eq!(checked_index(0, "geosite exact domains").unwrap(), 0);
		assert_eq!(checked_index(u32::MAX as usize, "geosite exact domains").unwrap(), u32::MAX);

		// `u32::MAX + 1` entries no longer fit; the old `len() as u32` wrapped
		// around to 0 instead of failing. Skipped on 32-bit hosts, where such a
		// count cannot be represented at all.
		let Ok(beyond) = usize::try_from(u64::from(u32::MAX) + 1) else {
			return;
		};
		let err = checked_index(beyond, "geosite exact domains").unwrap_err();
		let GeoDataError::IndexOverflow(message) = err else {
			panic!("an oversized index must be reported as GeoDataError::IndexOverflow, got {err:?}");
		};
		assert!(message.contains("geosite exact domains"), "{message}");
		assert!(message.contains(&beyond.to_string()), "{message}");
	}

	#[test]
	fn recorded_offsets_describe_the_stored_slices() {
		// The offsets are the only link between a category/country and its
		// domains/ranges, so pin that they index the vectors they claim to.
		let geosite = GeoSiteList {
			entry: vec![
				GeoSite {
					country_code: "GOOGLE".to_string(),
					domain: vec![domain(2, "google.com"), domain(3, "youtube.com")],
				},
				GeoSite {
					country_code: "ADS".to_string(),
					domain: vec![domain(0, "doubleclick"), domain(2, "ads.example")],
				},
			],
		};
		let geoip = GeoIpList {
			entry: vec![
				GeoIp {
					country_code: "US".to_string(),
					cidr: vec![Cidr {
						ip: vec![8, 8, 8, 0],
						prefix: 24,
					}],
					..Default::default()
				},
				GeoIp {
					country_code: "CN".to_string(),
					cidr: vec![Cidr {
						ip: 0x2400_3200u128.to_be_bytes().to_vec(),
						prefix: 32,
					}],
					..Default::default()
				},
			],
		};

		let snapshot = build_snapshot(&encode_geosite(geosite), &encode_geoip(geoip)).unwrap();

		let names: Vec<&str> = snapshot.geosite.categories.iter().map(|c| c.name.as_str()).collect();
		assert_eq!(names, ["ADS", "GOOGLE"], "categories must stay sorted by name");
		for category in &snapshot.geosite.categories {
			let exact = strings(&snapshot.geosite.exact_domains, category.exact_start, category.exact_len);
			let suffix = strings(&snapshot.geosite.suffix_domains, category.suffix_start, category.suffix_len);
			let keyword = strings(
				&snapshot.geosite.keyword_domains,
				category.keyword_start,
				category.keyword_len,
			);
			match category.name.as_str() {
				"ADS" => {
					assert!(exact.is_empty(), "{exact:?}");
					assert_eq!(suffix, ["ads.example"]);
					assert_eq!(keyword, ["doubleclick"]);
				}
				"GOOGLE" => {
					assert_eq!(exact, ["youtube.com"]);
					assert_eq!(suffix, ["google.com"]);
					assert!(keyword.is_empty(), "{keyword:?}");
				}
				other => panic!("unexpected category {other}"),
			}
		}

		let names: Vec<&str> = snapshot.geoip.countries.iter().map(|c| c.name.as_str()).collect();
		assert_eq!(names, ["CN", "US"], "countries must stay sorted by name");
		for country in &snapshot.geoip.countries {
			let v4 = v4_ranges(&snapshot.geoip.v4_ranges, country.v4_start, country.v4_len);
			match country.name.as_str() {
				"US" => {
					let network = u32::from(std::net::Ipv4Addr::new(8, 8, 8, 0));
					assert_eq!(v4, [(network, network | 0xff)]);
					assert_eq!(country.v6_len, 0);
				}
				"CN" => {
					assert!(v4.is_empty(), "{v4:?}");
					assert_eq!(country.v6_len, 1);
					// `v6_start`/`v6_len` are checked by the v6 counterpart
					// querying the same range, so only the index is verified
					// here.
					let start = country.v6_start as usize;
					let end = start + country.v6_len as usize;
					assert!(end <= snapshot.geoip.v6_ranges.len());
				}
				other => panic!("unexpected country {other}"),
			}
		}
	}
}
