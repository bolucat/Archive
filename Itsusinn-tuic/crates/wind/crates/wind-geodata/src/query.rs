use std::cmp::Ordering;

use rkyv::{Archived, vec::ArchivedVec};

use crate::snapshot::*;

impl ArchivedGeoDataSnapshot {
	/// Verify every category/country slice `[start, start + len)` lies within
	/// its backing vector, and that the name lists are sorted for the binary
	/// searches. rkyv's structural check does not cover these application-level
	/// invariants, so a corrupt or bit-flipped cache that still passes it could
	/// otherwise drive an out-of-bounds index panic on every query (DoS).
	pub fn validate_offsets(&self) -> Result<(), String> {
		let gs = &self.geosite;
		let (ex, sf, kw) = (gs.exact_domains.len(), gs.suffix_domains.len(), gs.keyword_domains.len());
		let mut prev: Option<&str> = None;
		for c in gs.categories.iter() {
			check_slice(c.exact_start.to_native(), c.exact_len.to_native(), ex, "geosite exact")?;
			check_slice(c.suffix_start.to_native(), c.suffix_len.to_native(), sf, "geosite suffix")?;
			check_slice(c.keyword_start.to_native(), c.keyword_len.to_native(), kw, "geosite keyword")?;
			let name = c.name.as_str();
			if let Some(p) = prev
				&& p > name
			{
				return Err(format!("geosite categories not sorted: {p:?} before {name:?}"));
			}
			prev = Some(name);
		}

		let gi = &self.geoip;
		let (v4, v6) = (gi.v4_ranges.len(), gi.v6_ranges.len());
		let mut prev: Option<&str> = None;
		for c in gi.countries.iter() {
			check_slice(c.v4_start.to_native(), c.v4_len.to_native(), v4, "geoip v4")?;
			check_slice(c.v6_start.to_native(), c.v6_len.to_native(), v6, "geoip v6")?;
			let name = c.name.as_str();
			if let Some(p) = prev
				&& p > name
			{
				return Err(format!("geoip countries not sorted: {p:?} before {name:?}"));
			}
			prev = Some(name);
		}
		Ok(())
	}
}

fn check_slice(start: u32, len: u32, total: usize, what: &str) -> Result<(), String> {
	let end = (start as usize)
		.checked_add(len as usize)
		.ok_or_else(|| format!("{what} slice offset overflow"))?;
	if end > total {
		return Err(format!("{what} slice [{start}, {end}) out of bounds (backing len {total})"));
	}
	Ok(())
}

impl ArchivedGeoSiteIndex {
	pub fn contains(&self, category: &str, domain: &str) -> bool {
		let Some(idx) = binary_search_cat(&self.categories, category) else {
			return false;
		};
		let cat = &self.categories[idx];

		// Domains are stored lowercase; normalise the query once and reuse it.
		let domain = domain.to_ascii_lowercase();

		let exact_start = cat.exact_start.to_native() as usize;
		let exact_len = cat.exact_len.to_native() as usize;
		if binary_search_str(&self.exact_domains, exact_start, exact_len, &domain).is_some() {
			return true;
		}

		let suffix_start = cat.suffix_start.to_native() as usize;
		let suffix_len = cat.suffix_len.to_native() as usize;
		if suffix_match(&self.suffix_domains, suffix_start, suffix_len, &domain) {
			return true;
		}

		let keyword_start = cat.keyword_start.to_native() as usize;
		let keyword_len = cat.keyword_len.to_native() as usize;
		if keyword_match(&self.keyword_domains, keyword_start, keyword_len, &domain) {
			return true;
		}

		false
	}
}

/// Case-insensitive binary search over the uppercase-stored category names.
fn binary_search_cat(cats: &ArchivedVec<ArchivedCategoryInfo>, name: &str) -> Option<usize> {
	let upper = name.to_ascii_uppercase();
	let mut lo = 0usize;
	let mut hi = cats.len();
	while lo < hi {
		let mid = lo + (hi - lo) / 2;
		match cats[mid].name.as_str().cmp(upper.as_str()) {
			Ordering::Less => lo = mid + 1,
			Ordering::Greater => hi = mid,
			Ordering::Equal => return Some(mid),
		}
	}
	None
}

fn binary_search_str(v: &ArchivedVec<Archived<String>>, start: usize, len: usize, needle: &str) -> Option<usize> {
	if len == 0 {
		return None;
	}
	let mut lo = start;
	let mut hi = start + len;
	while lo < hi {
		let mid = lo + (hi - lo) / 2;
		match v[mid].as_str().cmp(needle) {
			Ordering::Less => lo = mid + 1,
			Ordering::Greater => hi = mid,
			Ordering::Equal => return Some(mid),
		}
	}
	None
}

/// Matches `domain` against a v2ray "Domain" (suffix) list: the domain itself,
/// or any parent reached by stripping leading labels. `domain` must already be
/// lowercase.
fn suffix_match(v: &ArchivedVec<Archived<String>>, start: usize, len: usize, domain: &str) -> bool {
	if len == 0 {
		return false;
	}
	// The full domain is a candidate ("google.com" matches the entry "google.com").
	if binary_search_str_suffix(v, start, len, domain) {
		return true;
	}
	// Each label boundary yields a parent candidate ("mail.google.com" →
	// "google.com" → "com"). '.' is ASCII, so `i + 1` is always a valid UTF-8
	// boundary.
	for (i, &b) in domain.as_bytes().iter().enumerate() {
		if b == b'.' && binary_search_str_suffix(v, start, len, &domain[i + 1..]) {
			return true;
		}
	}
	false
}

fn binary_search_str_suffix(v: &ArchivedVec<Archived<String>>, start: usize, len: usize, needle: &str) -> bool {
	if len == 0 {
		return false;
	}
	let mut lo = start;
	let mut hi = start + len;
	while lo < hi {
		let mid = lo + (hi - lo) / 2;
		match v[mid].as_str().cmp(needle) {
			Ordering::Less => lo = mid + 1,
			Ordering::Greater => hi = mid,
			Ordering::Equal => return true,
		}
	}
	false
}

/// Matches `domain` (already lowercase) against a v2ray "Plain" keyword list.
fn keyword_match(v: &ArchivedVec<Archived<String>>, start: usize, len: usize, domain: &str) -> bool {
	if len == 0 {
		return false;
	}
	let end = start + len;
	(start..end).any(|i| domain.contains(v[i].as_str()))
}

impl ArchivedGeoIpIndex {
	pub fn contains(&self, country: &str, ip: std::net::IpAddr) -> bool {
		let Some(idx) = binary_search_country(&self.countries, country) else {
			return false;
		};
		let c = &self.countries[idx];
		match ip {
			std::net::IpAddr::V4(v4) => {
				let addr = u32::from(v4);
				let start = c.v4_start.to_native() as usize;
				let len = c.v4_len.to_native() as usize;
				range_contains_v4(&self.v4_ranges, start, len, addr)
			}
			std::net::IpAddr::V6(v6) => {
				let addr = u128::from(v6);
				let start = c.v6_start.to_native() as usize;
				let len = c.v6_len.to_native() as usize;
				range_contains_v6(&self.v6_ranges, start, len, addr)
			}
		}
	}
}

/// Case-insensitive binary search over the uppercase-stored country names.
fn binary_search_country(cs: &ArchivedVec<ArchivedCountryInfo>, name: &str) -> Option<usize> {
	let upper = name.to_ascii_uppercase();
	let mut lo = 0usize;
	let mut hi = cs.len();
	while lo < hi {
		let mid = lo + (hi - lo) / 2;
		match cs[mid].name.as_str().cmp(upper.as_str()) {
			Ordering::Less => lo = mid + 1,
			Ordering::Greater => hi = mid,
			Ordering::Equal => return Some(mid),
		}
	}
	None
}

/// `ranges[start..start + len]` is disjoint and sorted by `start`, so at most
/// one range can contain `addr`: the last one whose `start <= addr`.
fn range_contains_v4(ranges: &ArchivedVec<ArchivedRangeV4>, start: usize, len: usize, addr: u32) -> bool {
	if len == 0 {
		return false;
	}
	let mut lo = start;
	let mut hi = start + len;
	while lo < hi {
		let mid = lo + (hi - lo) / 2;
		if ranges[mid].start.to_native() <= addr {
			lo = mid + 1;
		} else {
			hi = mid;
		}
	}
	if lo > start {
		// `ranges[lo - 1].start <= addr` by construction; check the upper bound.
		addr <= ranges[lo - 1].end.to_native()
	} else {
		false
	}
}

fn range_contains_v6(ranges: &ArchivedVec<ArchivedRangeV6>, start: usize, len: usize, addr: u128) -> bool {
	if len == 0 {
		return false;
	}
	let mut lo = start;
	let mut hi = start + len;
	while lo < hi {
		let mid = lo + (hi - lo) / 2;
		if ranges[mid].start.to_native() <= addr {
			lo = mid + 1;
		} else {
			hi = mid;
		}
	}
	if lo > start {
		addr <= ranges[lo - 1].end.to_native()
	} else {
		false
	}
}
