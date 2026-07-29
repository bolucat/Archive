use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Serialize, Deserialize)]
pub struct GeoDataSnapshot {
	pub geosite: GeoSiteIndex,
	pub geoip: GeoIpIndex,
}

#[derive(Archive, Serialize, Deserialize)]
pub struct GeoSiteIndex {
	/// Sorted by `name` (uppercase) for binary search.
	pub categories: Vec<CategoryInfo>,
	pub exact_domains: Vec<String>,
	pub suffix_domains: Vec<String>,
	pub keyword_domains: Vec<String>,
}

#[derive(Archive, Serialize, Deserialize)]
pub struct CategoryInfo {
	/// Category tag, stored uppercase (matching geosite.dat convention).
	pub name: String,
	pub exact_start: u32,
	pub exact_len: u32,
	pub suffix_start: u32,
	pub suffix_len: u32,
	pub keyword_start: u32,
	pub keyword_len: u32,
}

#[derive(Archive, Serialize, Deserialize)]
pub struct GeoIpIndex {
	/// Sorted by `name` (uppercase) for binary search.
	pub countries: Vec<CountryInfo>,
	/// Per-country ranges, grouped contiguously, sorted by `start` within each
	/// group and merged so they are disjoint and non-overlapping.
	pub v4_ranges: Vec<RangeV4>,
	pub v6_ranges: Vec<RangeV6>,
}

#[derive(Archive, Serialize, Deserialize)]
pub struct CountryInfo {
	/// Country/category code, stored uppercase (matching geoip.dat convention).
	pub name: String,
	pub v4_start: u32,
	pub v4_len: u32,
	pub v6_start: u32,
	pub v6_len: u32,
}

/// Inclusive `[start, end]` IPv4 range.
#[derive(Archive, Serialize, Deserialize)]
pub struct RangeV4 {
	pub start: u32,
	pub end: u32,
}

/// Inclusive `[start, end]` IPv6 range.
#[derive(Archive, Serialize, Deserialize)]
pub struct RangeV6 {
	pub start: u128,
	pub end: u128,
}
