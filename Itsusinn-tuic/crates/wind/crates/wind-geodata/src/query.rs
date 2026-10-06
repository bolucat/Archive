use std::{cmp::Ordering, ops::Range};

use rkyv::{Archived, vec::ArchivedVec};

use crate::snapshot::*;

impl ArchivedGeoDataSnapshot {
	/// Verify every category/country slice `[start, start + len)` lies within
	/// its backing vector, that the name lists are sorted for the binary
	/// searches, and that each domain/range slice itself carries the order the
	/// queries rely on. rkyv's structural check does not cover these
	/// application-level invariants, so a corrupt or bit-flipped cache that
	/// still passes it could otherwise drive an out-of-bounds index panic on
	/// every query (DoS) or, for an unordered slice, silently answer with wrong
	/// match results.
	pub fn validate_offsets(&self) -> Result<(), String> {
		let gs = &self.geosite;
		let (ex, sf, kw) = (gs.exact_domains.len(), gs.suffix_domains.len(), gs.keyword_domains.len());
		let mut prev: Option<&str> = None;
		for c in gs.categories.iter() {
			let exact = check_slice(c.exact_start.to_native(), c.exact_len.to_native(), ex, "geosite exact")?;
			check_ascending(&gs.exact_domains, exact, "geosite exact domains")?;
			let suffix = check_slice(c.suffix_start.to_native(), c.suffix_len.to_native(), sf, "geosite suffix")?;
			check_ascending(&gs.suffix_domains, suffix, "geosite suffix domains")?;
			let keyword = check_slice(c.keyword_start.to_native(), c.keyword_len.to_native(), kw, "geosite keyword")?;
			check_ascending(&gs.keyword_domains, keyword, "geosite keyword domains")?;
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
			let v4_slice = check_slice(c.v4_start.to_native(), c.v4_len.to_native(), v4, "geoip v4")?;
			check_ranges(v4_slice, "geoip v4", |i| {
				let r = &gi.v4_ranges[i];
				(r.start.to_native(), r.end.to_native())
			})?;
			let v6_slice = check_slice(c.v6_start.to_native(), c.v6_len.to_native(), v6, "geoip v6")?;
			check_ranges(v6_slice, "geoip v6", |i| {
				let r = &gi.v6_ranges[i];
				(r.start.to_native(), r.end.to_native())
			})?;
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

/// Validate a `[start, start + len)` slice against a backing vector length and
/// return it as a `usize` range for the caller's ordering check.
fn check_slice(start: u32, len: u32, total: usize, what: &str) -> Result<Range<usize>, String> {
	let start = start as usize;
	let end = start
		.checked_add(len as usize)
		.ok_or_else(|| format!("{what} slice offset overflow"))?;
	if end > total {
		return Err(format!("{what} slice [{start}, {end}) out of bounds (backing len {total})"));
	}
	Ok(start..end)
}

/// Verify one category's slice of domain names is strictly ascending. The
/// builder sorts and dedups every list and the queries binary-search it, so an
/// out-of-order slice would silently miss entries instead of failing.
fn check_ascending(v: &ArchivedVec<Archived<String>>, slice: Range<usize>, what: &str) -> Result<(), String> {
	let mut prev: Option<&str> = None;
	for i in slice {
		let cur = v[i].as_str();
		if let Some(p) = prev
			&& p >= cur
		{
			return Err(format!("{what} not sorted: {p:?} before {cur:?}"));
		}
		prev = Some(cur);
	}
	Ok(())
}

/// Verify one country's slice of ranges is ascending and disjoint. The queries
/// binary-search for the last range whose `start <= addr` and then assume at
/// most one range can contain `addr`, so overlapping or out-of-order ranges
/// would silently produce wrong answers.
fn check_ranges<T: Ord + std::fmt::Debug + Copy>(
	slice: Range<usize>,
	what: &str,
	get: impl Fn(usize) -> (T, T),
) -> Result<(), String> {
	let mut prev_end: Option<T> = None;
	for i in slice {
		let (start, end) = get(i);
		if end < start {
			return Err(format!("{what} has an inverted range: {start:?} > {end:?}"));
		}
		if let Some(pe) = prev_end
			&& start <= pe
		{
			return Err(format!("{what} ranges not sorted/disjoint: {start:?} after {pe:?}"));
		}
		prev_end = Some(end);
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
	// The full domain is a candidate ("google.com" matches the entry
	// "google.com").
	if binary_search_str(v, start, len, domain).is_some() {
		return true;
	}
	// Each label boundary yields a parent candidate ("mail.google.com" →
	// "google.com" → "com"). '.' is ASCII, so `i + 1` is always a valid UTF-8
	// boundary.
	for (i, &b) in domain.as_bytes().iter().enumerate() {
		if b == b'.' && binary_search_str(v, start, len, &domain[i + 1..]).is_some() {
			return true;
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
		// `ranges[lo - 1].start <= addr` by construction; check the upper
		// bound.
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

#[cfg(test)]
mod tests {
	use rkyv::rancor;

	use super::*;

	/// Serialise a name list into the archived representation the queries
	/// binary-search, so the tests exercise the archived bytes rather than the
	/// live `Vec<String>`.
	fn archived_domains(names: &[&str]) -> rkyv::util::AlignedVec {
		let list: Vec<String> = names.iter().map(|name| (*name).to_string()).collect();
		rkyv::api::high::to_bytes::<rancor::Error>(&list).expect("serialize a Vec<String>")
	}

	fn domains(bytes: &rkyv::util::AlignedVec) -> &ArchivedVec<Archived<String>> {
		rkyv::access::<ArchivedVec<Archived<String>>, rancor::Error>(&bytes[..]).expect("access the archived list")
	}

	fn slice<T>(items: &[T], start: usize, len: usize) -> impl Iterator<Item = (usize, &T)> {
		(start..start + len).filter_map(move |i| items.get(i).map(|item| (i, item)))
	}

	/// Independent oracle for the search: a plain linear scan, not the binary
	/// search under test.
	fn linear_scan(v: &ArchivedVec<Archived<String>>, start: usize, len: usize, needle: &str) -> Option<usize> {
		(start..start + len).find(|&i| i < v.len() && v[i].as_str() == needle)
	}

	/// Independent oracle for suffix matching: the searched entries intersected
	/// with the candidate suffixes of `domain`.
	fn suffix_model(v: &ArchivedVec<Archived<String>>, start: usize, len: usize, domain: &str) -> bool {
		if len == 0 {
			return false;
		}
		let mut candidates = vec![domain];
		for (i, &b) in domain.as_bytes().iter().enumerate() {
			if b == b'.' {
				candidates.push(&domain[i + 1..]);
			}
		}
		slice(v, start, len).any(|(_, entry)| candidates.contains(&entry.as_str()))
	}

	#[test]
	fn binary_search_str_finds_the_needle_inside_the_slice() {
		let bytes = archived_domains(&["a.example", "b.example", "c.example"]);
		let v = domains(&bytes);

		assert_eq!(binary_search_str(v, 0, 3, "a.example"), Some(0));
		assert_eq!(binary_search_str(v, 0, 3, "b.example"), Some(1));
		assert_eq!(binary_search_str(v, 0, 3, "c.example"), Some(2));
		assert_eq!(binary_search_str(v, 0, 3, "d.example"), None);
	}

	#[test]
	fn binary_search_str_only_searches_the_given_slice() {
		let bytes = archived_domains(&["a.example", "b.example", "c.example", "d.example"]);
		let v = domains(&bytes);

		// A search over the middle slice sees neither of its neighbours, even
		// though both are present in the backing vector.
		assert_eq!(binary_search_str(v, 1, 2, "b.example"), Some(1));
		assert_eq!(binary_search_str(v, 1, 2, "c.example"), Some(2));
		assert_eq!(binary_search_str(v, 1, 2, "a.example"), None);
		assert_eq!(binary_search_str(v, 1, 2, "d.example"), None);
	}

	#[test]
	fn binary_search_str_matches_a_linear_scan_on_every_slice() {
		let names = ["a.example", "b.example", "c.example", "d.example", "e.example"];
		let bytes = archived_domains(&names);
		let v = domains(&bytes);
		let mut needles: Vec<&str> = names.to_vec();
		needles.extend(["f.example", "A.example", ""]);

		// Exhaustive comparison against the linear oracle: this pins the
		// slice-bounded contract the suffix matcher now relies on directly.
		for start in 0..=names.len() {
			for len in 0..=(names.len() - start) {
				for needle in &needles {
					let expected = linear_scan(v, start, len, needle);
					assert_eq!(
						binary_search_str(v, start, len, needle),
						expected,
						"start={start} len={len} needle={needle:?}"
					);
				}
			}
		}
	}

	#[test]
	fn an_empty_or_inverted_slice_is_a_miss_instead_of_a_panic() {
		let bytes = archived_domains(&["a.example", "b.example"]);
		let v = domains(&bytes);

		// The builder never records an empty slice, but a malformed cache can;
		// both queries must answer "no match" rather than index past the end.
		assert_eq!(binary_search_str(v, 0, 0, "a.example"), None);
		assert_eq!(binary_search_str(v, 2, 0, "b.example"), None);
		assert_eq!(binary_search_str(v, 5, 0, "a.example"), None);
		assert!(!suffix_match(v, 0, 0, "a.example"));
		assert!(!suffix_match(v, 2, 0, "b.example"));
	}

	#[test]
	fn suffix_membership_covers_the_domain_and_its_parent_labels() {
		let bytes = archived_domains(&["com", "example.com", "other.example"]);
		let v = domains(&bytes);

		// The full domain is a candidate, and every label boundary yields a
		// parent candidate.
		assert!(suffix_match(v, 0, 3, "example.com"));
		assert!(suffix_match(v, 0, 3, "mail.example.com"));
		assert!(suffix_match(v, 0, 3, "com"));
		assert!(suffix_match(v, 0, 3, "a.b.other.example"));
		// A parent that is not an entry on its own is still a miss.
		assert!(!suffix_match(v, 0, 3, "example.net"));
		// "ple.com" is not an entry, but stripping its first label yields the
		// stored "com" entry, so the suffix rule matches it.
		assert!(suffix_match(v, 0, 3, "ple.com"));
		// Only suffixes inside the requested slice count.
		assert!(!suffix_match(v, 1, 1, "com"));
		assert!(suffix_match(v, 1, 1, "example.com"));
		assert!(suffix_match(v, 1, 1, "mail.example.com"));
	}

	#[test]
	fn suffix_matching_agrees_with_the_candidate_suffix_model() {
		// Ascending order, as the builder writes and `validate_offsets`
		// enforces: the binary search is only meaningful on sorted slices.
		let names = ["a.example", "b.example", "c.example", "d.example"];
		let bytes = archived_domains(&names);
		let v = domains(&bytes);
		let queries = [
			"a.example",
			"b.example",
			"mail.b.example",
			"a.b.example",
			"x.a.example",
			"example",
			"c.d.example",
			"e.example",
			"a.example.net",
			"",
		];

		for start in 0..=names.len() {
			for len in 0..=(names.len() - start) {
				for query in &queries {
					let expected = suffix_model(v, start, len, query);
					assert_eq!(
						suffix_match(v, start, len, query),
						expected,
						"start={start} len={len} query={query:?}"
					);
				}
			}
		}
	}
}
