//! The `acl-ir` data model: matches, statements, verdicts, sets, maps, chains,
//! and the top-level [`Ruleset`].
//!
//! This mirrors the types in `specs/acl-ir.md`. Where the spec lists a typed
//! leaf for every Mihomo rule kind, this first implementation gives bespoke
//! typed variants to the *optimizer-relevant* subset (domain / ip / port /
//! proto) and routes everything else through [`Match::Predicate`], which wraps
//! a `wind_core::rule::Rule` and delegates evaluation to it. That keeps the
//! degenerate embedding (see [`crate::embed`]) byte-for-byte equivalent to the
//! legacy engine while still exposing the structure the optimizer needs on the
//! hot path.

use std::{ops::RangeInclusive, sync::Arc};

use ipnet::IpNet;
use wind_core::rule::{NetworkType, Rule};

/// Which side of the connection a leaf reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
	Dst,
	Src,
}

/// A boolean expression over a connection's `MatchContext`.
#[derive(Clone, Debug)]
pub enum Match {
	/// Logical AND. Empty = vacuously true (avoid constructing empty).
	All(Vec<Match>),
	/// Logical OR. Empty = vacuously false (avoid constructing empty).
	Any(Vec<Match>),
	/// Logical NOT.
	Not(Box<Match>),
	/// Matches every connection (`MATCH`).
	Always,

	/// Destination/source IP in a CIDR.
	Ip { side: Side, net: IpNet },
	/// Destination/source port in an inclusive range (a single port is
	/// `p..=p`).
	Port { side: Side, range: RangeInclusive<u16> },
	/// Network protocol (tcp/udp).
	Proto(NetworkType),
	/// Domain test (exact / suffix / keyword).
	Domain(DomainTest),

	/// Membership in a [`NamedSet`] (`ip daddr @set`). `set` indexes
	/// [`Ruleset::sets`]. The set's data type selects which `MatchContext`
	/// field is read; `side` selects dst/src for IP and port sets (ignored for
	/// domain sets).
	InSet { side: Side, set: usize },

	/// Escape hatch for any `wind_core` rule kind without a bespoke typed
	/// variant: geoip/asn/geosite, src-geo/src-asn, process/uid, dscp, inbound,
	/// domain wildcard/regex, and compound `AND`/`OR`/`NOT`/`SUB-RULE`.
	/// Evaluation delegates to [`Rule::matches`], so semantics are identical to
	/// the legacy engine. Wrapped in `Arc` so [`Match`] stays `Clone` even
	/// though `Rule` is not.
	Predicate(Arc<Rule>),
}

/// Domain match variants that the optimizer can pack into a [`DomainSet`].
/// Wildcard/regex domains do not appear here — they ride along as
/// [`Match::Predicate`].
#[derive(Clone, Debug)]
pub enum DomainTest {
	/// Exact match (case-insensitive).
	Exact(String),
	/// Suffix match — also matches subdomains. Stored lower-cased.
	Suffix(String),
	/// Substring match. Stored lower-cased.
	Keyword(String),
}

/// Non-terminal action executed (in order) before a rule's verdict.
///
/// The degenerate embedding never emits statements (legacy rules carry none);
/// they exist so hand-built rulesets and future lowerings (the legacy ACL
/// `hijack` field → [`Statement::Dnat`]) can use them.
#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
	Counter,
	Log(String),
	Mark(u32),
	/// Destination rewrite (legacy ACL `hijack`). Target kept as a string for
	/// v1.
	Dnat(String),
}

/// Terminal or chain-control decision.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
	/// Route to the named outbound.
	Forward(String),
	/// Reject with a reason. The embedding uses a canonical reason so that
	/// adjacent reject rules can be bucketed; the specific reason text is not a
	/// routing semantic.
	Reject(String),
	/// Silently drop.
	Drop,
	/// Pop to the calling chain (or fall through to policy in a base chain).
	Return,
	/// Call a chain by name; MAY return via [`Verdict::Return`].
	Jump(String),
	/// Tail-call a chain by name; never returns to the caller.
	Goto(String),
	/// Look the key up in [`Ruleset::maps`]`[idx]` and apply the result.
	Map(usize),
}

/// A named, typed, unordered lookup set.
#[derive(Clone, Debug)]
pub struct NamedSet {
	pub data: SetData,
}

#[derive(Clone, Debug)]
pub enum SetData {
	Domains(DomainSet),
	Ips(Vec<IpNet>),
	Ports(Vec<RangeInclusive<u16>>),
}

/// Domain set with the three match-kind buckets. Membership = match in any
/// bucket. A connection's domain is tested against all three.
#[derive(Clone, Debug, Default)]
pub struct DomainSet {
	pub exact: Vec<String>,
	pub suffix: Vec<String>,
	pub keyword: Vec<String>,
}

/// Which `MatchContext` field a verdict map keys on. v1 only builds port maps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapField {
	Port,
}

/// A verdict map: key → verdict, for O(1)-style dispatch over disjoint keys.
#[derive(Clone, Debug)]
pub struct VerdictMap {
	pub side: Side,
	pub field: MapField,
	/// Inclusive key ranges → verdict. The optimizer only builds a map when the
	/// ranges are pairwise disjoint, so at most one entry can match.
	pub entries: Vec<(RangeInclusive<u16>, Verdict)>,
	pub default: Option<Verdict>,
}

/// A single IR rule: `match → statement* → verdict`.
#[derive(Clone, Debug)]
pub struct IrRule {
	pub matches: Match,
	pub stmts: Vec<Statement>,
	pub verdict: Verdict,
}

/// An ordered list of rules plus a default policy verdict.
#[derive(Clone, Debug)]
pub struct Chain {
	pub name: String,
	pub policy: Verdict,
	pub rules: Vec<IrRule>,
}

/// The compiled program: sets, maps, chains, and the entry chain index.
#[derive(Clone, Debug)]
pub struct Ruleset {
	pub sets: Vec<NamedSet>,
	pub maps: Vec<VerdictMap>,
	pub chains: Vec<Chain>,
	/// Index into [`Ruleset::chains`] of the base chain where evaluation
	/// starts.
	pub entry: usize,
}

impl Ruleset {
	/// Find a chain index by name (used to resolve `Jump`/`Goto`).
	pub(crate) fn chain_index(&self, name: &str) -> Option<usize> {
		self.chains.iter().position(|c| c.name == name)
	}
}
