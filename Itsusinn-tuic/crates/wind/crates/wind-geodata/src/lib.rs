use std::{
	fs::File,
	net::IpAddr,
	path::{Path, PathBuf},
	sync::atomic::{AtomicU64, Ordering},
};

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

/// Distinguishes the temp files of concurrent builders living in the same
/// process (see `temp_cache_path`).
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Temp path used to stage a cache write before the atomic rename.
///
/// It must sit next to `cache_path` (the rename is only atomic within one
/// filesystem) and it must be unique per call: a pid alone is not enough,
/// because two builders in the same process — a second task, or parallel
/// tests using one cache path — would otherwise write, rename, and delete one
/// shared temp file out from under each other.
fn temp_cache_path(cache_path: &Path) -> PathBuf {
	let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
	let mut tmp_name = cache_path.file_name().unwrap_or_default().to_os_string();
	tmp_name.push(format!(".tmp.{}.{}", std::process::id(), seq));
	cache_path.with_file_name(tmp_name)
}

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
		// `open()` would read. Write to a per-call temp file, then rename it
		// into place (atomic on the same filesystem).
		let tmp_path = temp_cache_path(cache_path);
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
		// rkyv confirms the archive is structurally sound; additionally check
		// the application-level slice-offset invariants so a corrupt cache
		// can't cause an out-of-bounds panic at query time.
		snapshot.validate_offsets().map_err(GeoDataError::Validate)?;
		Ok(())
	}

	fn snapshot(&self) -> &ArchivedGeoDataSnapshot {
		// Safety: the archive was validated by `validate()` in
		// `open`/`build_and_open`, and the payload starts at a 16-byte
		// boundary (page-aligned base + 16).
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
	use std::{net::IpAddr, sync::atomic::AtomicUsize};

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
		// Regression: a matched category with a domain that misses
		// exact/suffix/keyword used to walk past the end of the byte buffer
		// and panic.
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
	fn duplicate_geosite_categories_are_merged() {
		// Regression: entries sharing a tag used to produce one `CategoryInfo`
		// per entry, so the by-name binary search could only ever reach one of
		// them (the other entry's domains were unreachable). Tags are compared
		// after uppercasing, so differently-cased spellings must merge too.
		let geosite = GeoSiteList {
			entry: vec![
				GeoSite {
					country_code: "google".to_string(),
					domain: vec![domain(3, "first.example")], // Full → exact
				},
				GeoSite {
					country_code: "GOOGLE".to_string(),
					domain: vec![domain(2, "second.example")], // Domain → suffix
				},
			],
		};
		let geoip = GeoIpList { entry: Vec::new() };
		let gs = encode_geosite(geosite);
		let gi = encode_geoip(geoip);

		let snapshot = crate::builder::build_snapshot(&gs, &gi).unwrap();
		assert_eq!(
			snapshot.geosite.categories.len(),
			1,
			"a repeated geosite tag must produce a single merged category"
		);
		assert_eq!(snapshot.geosite.categories[0].name, "GOOGLE");

		let tmp = tempfile::NamedTempFile::new().unwrap().into_temp_path();
		let geo = GeoData::build_and_open(&gs, &gi, &tmp).unwrap();
		let site = geo.geosite_lookup();
		assert!(site("google", "first.example"));
		assert!(site("google", "second.example"));
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

	/// Serialise `snapshot` behind a valid header into a temporary cache file.
	fn write_cache(snapshot: &crate::snapshot::GeoDataSnapshot) -> tempfile::NamedTempFile {
		let payload = rkyv::api::high::to_bytes::<rkyv::rancor::Error>(snapshot).unwrap();
		let mut buf = Vec::new();
		buf.extend_from_slice(&MAGIC);
		buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
		buf.extend_from_slice(&[0u8; 4]);
		buf.extend_from_slice(&payload[..]);

		let tmp = tempfile::NamedTempFile::new().unwrap();
		std::fs::write(tmp.path(), &buf).unwrap();
		tmp
	}

	/// A snapshot with no geosite data and one country holding `v4_ranges`.
	fn geoip_snapshot(v4_ranges: Vec<crate::snapshot::RangeV4>) -> crate::snapshot::GeoDataSnapshot {
		use crate::snapshot::{CountryInfo, GeoDataSnapshot, GeoIpIndex, GeoSiteIndex};

		GeoDataSnapshot {
			geosite: GeoSiteIndex {
				categories: Vec::new(),
				exact_domains: Vec::new(),
				suffix_domains: Vec::new(),
				keyword_domains: Vec::new(),
			},
			geoip: GeoIpIndex {
				countries: vec![CountryInfo {
					name: "EVIL".to_string(),
					v4_start: 0,
					v4_len: v4_ranges.len() as u32,
					v6_start: 0,
					v6_len: 0,
				}],
				v4_ranges,
				v6_ranges: Vec::new(),
			},
		}
	}

	#[test]
	fn open_rejects_unsorted_domain_slice() {
		use crate::snapshot::{CategoryInfo, CountryInfo, GeoDataSnapshot, GeoIpIndex, GeoSiteIndex};

		// In bounds and structurally sound, but the category's exact-domain
		// slice is out of order. Every query binary-searches that slice, so a
		// cache like this would silently miss entries instead of being
		// rejected.
		let snapshot = GeoDataSnapshot {
			geosite: GeoSiteIndex {
				categories: vec![CategoryInfo {
					name: "EVIL".to_string(),
					exact_start: 0,
					exact_len: 2,
					suffix_start: 0,
					suffix_len: 0,
					keyword_start: 0,
					keyword_len: 0,
				}],
				exact_domains: vec!["b.example".to_string(), "a.example".to_string()],
				suffix_domains: Vec::new(),
				keyword_domains: Vec::new(),
			},
			geoip: GeoIpIndex {
				countries: Vec::<CountryInfo>::new(),
				v4_ranges: Vec::new(),
				v6_ranges: Vec::new(),
			},
		};

		let tmp = write_cache(&snapshot);
		assert!(matches!(
			GeoData::open(tmp.path()),
			Err(GeoDataError::Validate(msg)) if msg.contains("geosite exact domains not sorted")
		));
	}

	#[test]
	fn open_rejects_overlapping_or_inverted_ranges() {
		use crate::snapshot::RangeV4;

		// `range_contains_v4` finds the last range whose start <= addr and
		// assumes at most one range can contain the address. Overlapping,
		// descending, or inverted ranges break that assumption silently.
		let cases: [(&str, Vec<RangeV4>); 3] = [
			(
				"overlapping",
				vec![RangeV4 { start: 10, end: 20 }, RangeV4 { start: 15, end: 25 }],
			),
			(
				"descending",
				vec![RangeV4 { start: 30, end: 40 }, RangeV4 { start: 10, end: 20 }],
			),
			("inverted", vec![RangeV4 { start: 20, end: 10 }]),
		];

		for (what, ranges) in cases {
			let tmp = write_cache(&geoip_snapshot(ranges));
			assert!(
				matches!(GeoData::open(tmp.path()), Err(GeoDataError::Validate(_))),
				"{what} ranges must be rejected"
			);
		}
	}

	#[test]
	fn open_accepts_ordered_slices() {
		use crate::snapshot::{CategoryInfo, CountryInfo, GeoDataSnapshot, GeoIpIndex, GeoSiteIndex, RangeV4};

		// Positive control: the invariants the ordering checks demand (sorted
		// domains, ranges sorted by start and disjoint) must still open and
		// answer queries.
		let snapshot = GeoDataSnapshot {
			geosite: GeoSiteIndex {
				categories: vec![CategoryInfo {
					name: "GOOGLE".to_string(),
					exact_start: 0,
					exact_len: 2,
					suffix_start: 0,
					suffix_len: 1,
					keyword_start: 0,
					keyword_len: 1,
				}],
				exact_domains: vec!["a.example".to_string(), "b.example".to_string()],
				suffix_domains: vec!["c.example".to_string()],
				keyword_domains: vec!["d".to_string()],
			},
			geoip: GeoIpIndex {
				countries: vec![CountryInfo {
					name: "US".to_string(),
					v4_start: 0,
					v4_len: 2,
					v6_start: 0,
					v6_len: 0,
				}],
				v4_ranges: vec![RangeV4 { start: 1, end: 2 }, RangeV4 { start: 5, end: 9 }],
				v6_ranges: Vec::new(),
			},
		};

		let tmp = write_cache(&snapshot);
		let geo = GeoData::open(tmp.path()).unwrap();
		let site = geo.geosite_lookup();
		assert!(site("google", "a.example"));
		assert!(site("google", "sub.c.example"));
		assert!(site("google", "xxdxx"));
		assert!(!site("google", "z.example"));
		let ip = geo.geoip_lookup();
		assert!(ip("US", "0.0.0.5".parse::<IpAddr>().unwrap()));
		assert!(!ip("US", "0.0.0.3".parse::<IpAddr>().unwrap()));
	}

	#[test]
	fn temp_cache_paths_are_unique_per_call() {
		// Regression: the temp path was `<cache>.tmp.<pid>`, i.e. identical
		// for every builder inside one process. Two builders sharing a cache
		// path then staged their bytes to the same file.
		let dir = tempfile::tempdir().unwrap();
		let cache = dir.path().join("geodata.rkyv");

		let first = temp_cache_path(&cache);
		let second = temp_cache_path(&cache);

		assert_ne!(first, second, "two builders in one process must not share a temp file");
		// The rename into place only stays atomic while the temp file is on
		// the cache's own filesystem.
		assert_eq!(first.parent(), cache.parent());
		assert_eq!(second.parent(), cache.parent());
	}

	#[test]
	fn concurrent_builds_to_one_cache_path_all_succeed() {
		// End to end counterpart of `temp_cache_paths_are_unique_per_call`:
		// two real builders racing on one cache path must both keep a usable
		// handle, and the cache left behind must still be complete.
		let (gs, gi) = fixture();
		let dir = tempfile::tempdir().unwrap();
		let cache = dir.path().join("geodata.rkyv");
		let barrier = std::sync::Barrier::new(2);
		let built = AtomicUsize::new(0);
		let built = &built;

		std::thread::scope(|scope| {
			for _ in 0..2 {
				let gs = &gs;
				let gi = &gi;
				let cache = &cache;
				let barrier = &barrier;
				scope.spawn(move || {
					barrier.wait();
					let geo = GeoData::build_and_open(gs, gi, cache).expect("concurrent build must not lose its temp file");
					assert!(geo.geosite_lookup()("google", "mail.google.com"));
					assert!(geo.geoip_lookup()("US", "8.8.8.8".parse::<IpAddr>().unwrap()));
					built.fetch_add(1, Ordering::SeqCst);
				});
			}
		});

		assert_eq!(built.load(Ordering::SeqCst), 2);
		let reopened = GeoData::open(&cache).unwrap();
		assert!(reopened.geosite_lookup()("google", "youtube.com"));
		assert!(reopened.geoip_lookup()("CN", "2400:3200::1".parse::<IpAddr>().unwrap()));
	}
}
