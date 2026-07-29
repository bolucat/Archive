use std::{fs::File, net::IpAddr, path::Path};

use memmap2::Mmap;

use crate::{builder::build_snapshot, snapshot::ArchivedGeoDataSnapshot};

pub mod builder;
mod error;
mod query;
pub mod snapshot;

pub use error::GeoDataError;

/// File magic identifying a wind-geodata cache.
const MAGIC: [u8; 8] = *b"WINDGEO\0";
/// Snapshot schema version. Bump on any change to `snapshot.rs` layout.
const FORMAT_VERSION: u32 = 1;
/// Header is 8 (magic) + 4 (version) + 4 (reserved) = 16 bytes. The 16-byte
/// size keeps the rkyv payload aligned to 16 relative to the (page-aligned)
/// mmap base.
const HEADER_LEN: usize = 16;

pub struct GeoData {
	mmap: Mmap,
}

impl GeoData {
	pub fn open(cache_path: &Path) -> Result<Self, GeoDataError> {
		let file = File::open(cache_path)?;
		let mmap = unsafe { Mmap::map(&file)? };
		Self::validate(&mmap)?;
		Ok(Self { mmap })
	}

	pub fn build_and_open(geosite_bytes: &[u8], geoip_bytes: &[u8], cache_path: &Path) -> Result<Self, GeoDataError> {
		let snapshot = build_snapshot(geosite_bytes, geoip_bytes)?;
		let payload =
			rkyv::api::high::to_bytes::<rkyv::rancor::Error>(&snapshot).map_err(|e| GeoDataError::Serialize(e.to_string()))?;

		let mut buf = Vec::with_capacity(HEADER_LEN + payload.len());
		buf.extend_from_slice(&MAGIC);
		buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
		buf.extend_from_slice(&[0u8; 4]);
		buf.extend_from_slice(&payload[..]);
		// Write atomically: a partial write (process killed mid-write) or a
		// concurrent builder must never leave a truncated cache that a later
		// `open()` would read. Write to a pid-scoped temp file, then rename it
		// into place (atomic on the same filesystem).
		let tmp_path = cache_path.with_extension(format!("tmp.{}", std::process::id()));
		std::fs::write(&tmp_path, &buf)?;
		if let Err(e) = std::fs::rename(&tmp_path, cache_path) {
			let _ = std::fs::remove_file(&tmp_path);
			return Err(e.into());
		}

		let file = File::open(cache_path)?;
		let mmap = unsafe { Mmap::map(&file)? };
		Self::validate(&mmap)?;
		Ok(Self { mmap })
	}

	/// Validate the header and fully type-check the rkyv archive. Run once at
	/// open time so `snapshot()` can use the cheap unchecked accessor
	/// afterwards.
	fn validate(bytes: &[u8]) -> Result<(), GeoDataError> {
		if bytes.len() < HEADER_LEN {
			return Err(GeoDataError::Truncated);
		}
		if bytes[..8] != MAGIC {
			return Err(GeoDataError::BadMagic);
		}
		let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
		if version != FORMAT_VERSION {
			return Err(GeoDataError::UnsupportedVersion(version));
		}
		let snapshot = rkyv::access::<ArchivedGeoDataSnapshot, rkyv::rancor::Error>(&bytes[HEADER_LEN..])
			.map_err(|e| GeoDataError::Validate(e.to_string()))?;
		// rkyv confirms the archive is structurally sound; additionally check the
		// application-level slice-offset invariants so a corrupt cache can't cause
		// an out-of-bounds panic at query time.
		snapshot.validate_offsets().map_err(GeoDataError::Validate)?;
		Ok(())
	}

	fn snapshot(&self) -> &ArchivedGeoDataSnapshot {
		// Safety: the archive was validated by `validate()` in `open`/`build_and_open`,
		// and the payload starts at a 16-byte boundary (page-aligned base + 16).
		unsafe { rkyv::access_unchecked(&self.mmap[HEADER_LEN..]) }
	}

	pub fn geoip_lookup(&self) -> impl Fn(&str, IpAddr) -> bool + '_ {
		|country, ip| self.snapshot().geoip.contains(country, ip)
	}

	pub fn geosite_lookup(&self) -> impl Fn(&str, &str) -> bool + '_ {
		|category, domain| self.snapshot().geosite.contains(category, domain)
	}
}

#[cfg(test)]
mod tests {
	use std::net::IpAddr;

	use geosite_rs::{Cidr, Domain, GeoIp, GeoIpList, GeoSite, GeoSiteList, encode_geoip, encode_geosite};

	use super::*;

	fn domain(r#type: i32, value: &str) -> Domain {
		Domain {
			r#type,
			value: value.to_string(),
			..Default::default()
		}
	}

	fn cidr_v4(octets: [u8; 4], prefix: u32) -> Cidr {
		Cidr {
			ip: octets.to_vec(),
			prefix,
		}
	}

	fn cidr_v6(addr: u128, prefix: u32) -> Cidr {
		Cidr {
			ip: addr.to_be_bytes().to_vec(),
			prefix,
		}
	}

	/// Synthetic geosite/geoip `.dat` bytes, exercising every match type and an
	/// overlapping-country case (CLOUDFLARE nested inside US).
	fn fixture() -> (Vec<u8>, Vec<u8>) {
		let geosite = GeoSiteList {
			entry: vec![
				GeoSite {
					country_code: "GOOGLE".to_string(),
					domain: vec![
						domain(2, "google.com"),  // Domain → suffix
						domain(3, "youtube.com"), // Full → exact
					],
				},
				GeoSite {
					country_code: "CATEGORY-ADS".to_string(),
					domain: vec![domain(0, "doubleclick")], // Plain → keyword
				},
			],
		};

		let geoip = GeoIpList {
			entry: vec![
				GeoIp {
					country_code: "US".to_string(),
					cidr: vec![cidr_v4([8, 8, 8, 0], 24), cidr_v4([104, 16, 0, 0], 12)],
					..Default::default()
				},
				GeoIp {
					// Overlaps US 104.16.0.0/12.
					country_code: "CLOUDFLARE".to_string(),
					cidr: vec![cidr_v4([104, 16, 0, 0], 13)],
					..Default::default()
				},
				GeoIp {
					country_code: "CN".to_string(),
					cidr: vec![cidr_v6(0x2400_3200_0000_0000_0000_0000_0000_0000, 32)],
					..Default::default()
				},
			],
		};

		(encode_geosite(geosite), encode_geoip(geoip))
	}

	fn open_fixture() -> (tempfile::TempPath, GeoData) {
		let (gs, gi) = fixture();
		let tmp = tempfile::NamedTempFile::new().unwrap().into_temp_path();
		let geo = GeoData::build_and_open(&gs, &gi, &tmp).unwrap();
		(tmp, geo)
	}

	#[test]
	fn geosite_suffix() {
		let (_tmp, geo) = open_fixture();
		let site = geo.geosite_lookup();
		assert!(site("google", "google.com")); // the value itself
		assert!(site("google", "mail.google.com")); // sub-label
		assert!(site("GOOGLE", "mail.google.com")); // category is case-insensitive
		assert!(site("google", "a.b.c.google.com"));
		assert!(!site("google", "notgoogle.com")); // not a label boundary
		assert!(!site("google", "google.com.evil.com"));
	}

	#[test]
	fn geosite_exact() {
		let (_tmp, geo) = open_fixture();
		let site = geo.geosite_lookup();
		assert!(site("google", "youtube.com"));
		assert!(site("google", "YouTube.COM")); // domain is case-insensitive
		assert!(!site("google", "www.youtube.com")); // exact: no subdomains
	}

	#[test]
	fn geosite_keyword() {
		let (_tmp, geo) = open_fixture();
		let site = geo.geosite_lookup();
		assert!(site("category-ads", "x.doubleclick.net"));
		assert!(site("category-ads", "doubleclick.com"));
		assert!(!site("category-ads", "example.com"));
	}

	#[test]
	fn geosite_miss_does_not_panic() {
		// Regression: a matched category with a domain that misses exact/suffix/keyword
		// used to walk past the end of the byte buffer and panic.
		let (_tmp, geo) = open_fixture();
		let site = geo.geosite_lookup();
		assert!(!site("google", "example.org"));
		assert!(!site("google", "com"));
		assert!(!site("google", "x"));
		assert!(!site("nonexistent-category", "google.com"));
	}

	#[test]
	fn geoip_v4() {
		let (_tmp, geo) = open_fixture();
		let ip = geo.geoip_lookup();
		assert!(ip("US", "8.8.8.8".parse::<IpAddr>().unwrap()));
		assert!(ip("us", "8.8.8.8".parse::<IpAddr>().unwrap())); // case-insensitive
		assert!(!ip("US", "9.9.9.9".parse::<IpAddr>().unwrap()));
		assert!(!ip("US", "8.8.9.1".parse::<IpAddr>().unwrap())); // outside /24, outside /12
	}

	#[test]
	fn geoip_overlapping_countries() {
		// Regression: 104.16.0.1 is in both US /12 and CLOUDFLARE /13. The old
		// first-match scan could report only one of them.
		let (_tmp, geo) = open_fixture();
		let ip = geo.geoip_lookup();
		let addr = "104.16.0.1".parse::<IpAddr>().unwrap();
		assert!(ip("US", addr));
		assert!(ip("CLOUDFLARE", addr));
		assert!(!ip("CN", addr));
	}

	#[test]
	fn geoip_v6() {
		let (_tmp, geo) = open_fixture();
		let ip = geo.geoip_lookup();
		assert!(ip("CN", "2400:3200::1".parse::<IpAddr>().unwrap()));
		assert!(!ip("CN", "2401:3200::1".parse::<IpAddr>().unwrap()));
		assert!(!ip("US", "2400:3200::1".parse::<IpAddr>().unwrap()));
	}

	#[test]
	fn open_roundtrips_via_cache() {
		let (tmp, _geo) = open_fixture();
		let reopened = GeoData::open(&tmp).unwrap();
		assert!(reopened.geoip_lookup()("US", "8.8.8.8".parse::<IpAddr>().unwrap()));
		assert!(reopened.geosite_lookup()("google", "mail.google.com"));
	}

	#[test]
	fn open_rejects_garbage() {
		let tmp = tempfile::NamedTempFile::new().unwrap();
		std::fs::write(tmp.path(), b"this is definitely not a geodata cache file").unwrap();
		assert!(matches!(
			GeoData::open(tmp.path()),
			Err(GeoDataError::BadMagic | GeoDataError::Validate(_) | GeoDataError::UnsupportedVersion(_))
		));
	}

	#[test]
	fn open_rejects_tiny_file() {
		let tmp = tempfile::NamedTempFile::new().unwrap();
		std::fs::write(tmp.path(), b"hi").unwrap();
		assert!(matches!(GeoData::open(tmp.path()), Err(GeoDataError::Truncated)));
	}

	#[test]
	fn open_rejects_out_of_bounds_offsets() {
		use crate::snapshot::{CategoryInfo, CountryInfo, GeoDataSnapshot, GeoIpIndex, GeoSiteIndex};

		// Structurally valid rkyv archive, but a category claims exact domains
		// far beyond the (empty) exact_domains vector. Must be rejected, not
		// allowed to panic later during a query.
		let snapshot = GeoDataSnapshot {
			geosite: GeoSiteIndex {
				categories: vec![CategoryInfo {
					name: "EVIL".to_string(),
					exact_start: 5,
					exact_len: 100,
					suffix_start: 0,
					suffix_len: 0,
					keyword_start: 0,
					keyword_len: 0,
				}],
				exact_domains: Vec::new(),
				suffix_domains: Vec::new(),
				keyword_domains: Vec::new(),
			},
			geoip: GeoIpIndex {
				countries: Vec::<CountryInfo>::new(),
				v4_ranges: Vec::new(),
				v6_ranges: Vec::new(),
			},
		};
		let payload = rkyv::api::high::to_bytes::<rkyv::rancor::Error>(&snapshot).unwrap();
		let mut buf = Vec::new();
		buf.extend_from_slice(&MAGIC);
		buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
		buf.extend_from_slice(&[0u8; 4]);
		buf.extend_from_slice(&payload[..]);

		let tmp = tempfile::NamedTempFile::new().unwrap();
		std::fs::write(tmp.path(), &buf).unwrap();
		assert!(matches!(GeoData::open(tmp.path()), Err(GeoDataError::Validate(_))));
	}
}
