//! Evaluation of a [`Ruleset`] against a `wind_core::rule::MatchContext`.
//!
//! Semantics follow `specs/acl-ir.md` §4: scan the entry chain top-to-bottom;
//! on a match run the statements then apply the verdict; `Jump` pushes a return
//! frame, `Goto` does not, `Return` / chain-exhaustion falls through; a base
//! chain that falls through applies its `policy`.

use std::net::IpAddr;

use wind_core::{RouteAction, rule::MatchContext};

use crate::model::{DomainSet, MapField, Match, Ruleset, SetData, Side, Verdict};

/// Guard against malformed `Jump`/`Goto` cycles.
const MAX_CHAIN_DEPTH: usize = 64;

/// The outcome of evaluating a chain.
enum Resolution {
	Forward(String),
	Reject(String),
	Drop,
	/// `Return`, or the chain ran out of rules without a terminal verdict.
	Fallthrough,
}

impl Ruleset {
	/// Classify a connection, returning a [`RouteAction`].
	///
	/// `Drop` is surfaced as a `Reject` (the legacy [`RouteAction`] has no drop
	/// variant); the degenerate embedding never produces `Drop`, so this only
	/// affects hand-built rulesets.
	pub fn route(&self, ctx: &MatchContext) -> RouteAction {
		match self.eval_chain(self.entry, ctx, 0) {
			Resolution::Forward(o) => RouteAction::Forward(o),
			Resolution::Reject(r) => RouteAction::Reject(r),
			Resolution::Drop => RouteAction::Reject("dropped".to_string()),
			// A base chain that falls through applies its policy. `policy` is a
			// terminal verdict by construction.
			Resolution::Fallthrough => match self.resolve_policy(self.entry, ctx) {
				Resolution::Forward(o) => RouteAction::Forward(o),
				Resolution::Reject(r) => RouteAction::Reject(r),
				_ => RouteAction::Reject("no matching rule and non-terminal policy".to_string()),
			},
		}
	}

	fn resolve_policy(&self, chain: usize, ctx: &MatchContext) -> Resolution {
		let policy = self.chains[chain].policy.clone();
		self.apply_verdict(&policy, ctx, 0)
	}

	fn eval_chain(&self, chain: usize, ctx: &MatchContext, depth: usize) -> Resolution {
		if depth >= MAX_CHAIN_DEPTH {
			return Resolution::Fallthrough;
		}
		for rule in &self.chains[chain].rules {
			if !self.match_expr(&rule.matches, ctx) {
				continue;
			}
			// Statements are non-terminal side effects; routing ignores them.
			let _ = &rule.stmts;
			match self.apply_verdict(&rule.verdict, ctx, depth) {
				Resolution::Fallthrough => continue, // e.g. a verdict map with no hit
				other => return other,
			}
		}
		Resolution::Fallthrough
	}

	fn apply_verdict(&self, verdict: &Verdict, ctx: &MatchContext, depth: usize) -> Resolution {
		match verdict {
			Verdict::Forward(o) => Resolution::Forward(o.clone()),
			Verdict::Reject(r) => Resolution::Reject(r.clone()),
			Verdict::Drop => Resolution::Drop,
			Verdict::Return => Resolution::Fallthrough,
			Verdict::Jump(name) => match self.chain_index(name) {
				Some(idx) => match self.eval_chain(idx, ctx, depth + 1) {
					Resolution::Fallthrough => Resolution::Fallthrough,
					other => other,
				},
				None => Resolution::Fallthrough,
			},
			Verdict::Goto(name) => match self.chain_index(name) {
				Some(idx) => self.eval_chain(idx, ctx, depth + 1),
				None => Resolution::Fallthrough,
			},
			Verdict::Map(idx) => self.apply_map(*idx, ctx, depth),
		}
	}

	fn apply_map(&self, idx: usize, ctx: &MatchContext, depth: usize) -> Resolution {
		let map = &self.maps[idx];
		let key = match map.field {
			MapField::Port => match map.side {
				Side::Dst => ctx.dst_port,
				Side::Src => ctx.src_port,
			},
		};
		if let Some(k) = key {
			for (range, verdict) in &map.entries {
				if range.contains(&k) {
					return self.apply_verdict(verdict, ctx, depth);
				}
			}
		}
		match &map.default {
			Some(v) => self.apply_verdict(v, ctx, depth),
			None => Resolution::Fallthrough,
		}
	}

	fn match_expr(&self, m: &Match, ctx: &MatchContext) -> bool {
		match m {
			Match::All(subs) => subs.iter().all(|s| self.match_expr(s, ctx)),
			Match::Any(subs) => subs.iter().any(|s| self.match_expr(s, ctx)),
			Match::Not(inner) => !self.match_expr(inner, ctx),
			Match::Always => true,

			Match::Ip { side, net } => side_ip(ctx, *side).is_some_and(|ip| net.contains(&ip)),
			Match::Port { side, range } => side_port(ctx, *side).is_some_and(|p| range.contains(&p)),
			Match::Proto(n) => ctx.network.is_some_and(|nn| nn == *n),
			Match::Domain(test) => ctx.domain.is_some_and(|d| domain_test_matches(test, d)),

			Match::InSet { side, set } => self.set_contains(*set, *side, ctx),

			Match::Predicate(rule) => rule.matches(ctx),
		}
	}

	fn set_contains(&self, set: usize, side: Side, ctx: &MatchContext) -> bool {
		match &self.sets[set].data {
			SetData::Domains(ds) => ctx.domain.is_some_and(|d| domain_set_contains(ds, d)),
			SetData::Ips(nets) => side_ip(ctx, side).is_some_and(|ip| nets.iter().any(|n| n.contains(&ip))),
			SetData::Ports(ranges) => side_port(ctx, side).is_some_and(|p| ranges.iter().any(|r| r.contains(&p))),
		}
	}
}

fn side_ip(ctx: &MatchContext, side: Side) -> Option<IpAddr> {
	match side {
		Side::Dst => ctx.dst_ip,
		Side::Src => ctx.src_ip,
	}
}

fn side_port(ctx: &MatchContext, side: Side) -> Option<u16> {
	match side {
		Side::Dst => ctx.dst_port,
		Side::Src => ctx.src_port,
	}
}

/// Whether a single [`DomainTest`] matches a host. The exact, allocation-free
/// algorithms are copied verbatim from `wind_core::rule` so a typed domain leaf
/// evaluates identically to the legacy `Rule::matches`.
fn domain_test_matches(test: &crate::model::DomainTest, host: &str) -> bool {
	use crate::model::DomainTest::*;
	match test {
		Exact(d) => host.eq_ignore_ascii_case(d),
		Suffix(s) => ascii_ci_ends_with_dot_or_eq(host, s),
		Keyword(k) => ascii_ci_contains(host, k),
	}
}

fn domain_set_contains(ds: &DomainSet, host: &str) -> bool {
	ds.exact.iter().any(|d| host.eq_ignore_ascii_case(d))
		|| ds.suffix.iter().any(|s| ascii_ci_ends_with_dot_or_eq(host, s))
		|| ds.keyword.iter().any(|k| ascii_ci_contains(host, k))
}

/// `host == suffix` OR `host` ends with `.{suffix}`, ASCII-case-insensitive.
/// Verbatim from `wind_core::rule`. `suffix_lc` is expected lower-cased.
fn ascii_ci_ends_with_dot_or_eq(host: &str, suffix_lc: &str) -> bool {
	let hb = host.as_bytes();
	let sb = suffix_lc.as_bytes();
	if hb.len() < sb.len() {
		return false;
	}
	if hb.len() == sb.len() {
		return hb.iter().zip(sb).all(|(a, b)| a.eq_ignore_ascii_case(b));
	}
	let dot_pos = hb.len() - sb.len() - 1;
	if hb[dot_pos] != b'.' {
		return false;
	}
	hb[dot_pos + 1..].iter().zip(sb).all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// ASCII-case-insensitive substring search. Verbatim from `wind_core::rule`.
/// `needle_lc` is expected lower-cased.
fn ascii_ci_contains(host: &str, needle_lc: &str) -> bool {
	let hb = host.as_bytes();
	let nb = needle_lc.as_bytes();
	if nb.is_empty() {
		return true;
	}
	if hb.len() < nb.len() {
		return false;
	}
	(0..=hb.len() - nb.len()).any(|i| hb[i..i + nb.len()].iter().zip(nb).all(|(a, b)| a.eq_ignore_ascii_case(b)))
}
