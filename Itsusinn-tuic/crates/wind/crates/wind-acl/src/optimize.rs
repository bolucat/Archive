//! The order-preserving optimizer (`specs/acl-ir.md` §7).
//!
//! The ordered rule list is ground truth; sets and verdict maps are unordered
//! and are only introduced when provably order-invariant:
//!
//! * **Pass 1** — a contiguous run of rules with identical `(stmts, verdict)`
//!   is merged into one set-backed rule. Always safe (no overlap analysis
//!   needed): all members share the verdict, and contiguity means no foreign
//!   rule is crossed.
//! * **Pass 2** — a contiguous run of single `Port` leaves on one side with
//!   *differing* verdicts becomes a verdict map, but only when the port ranges
//!   are pairwise disjoint (so at most one entry can match and order is
//!   unobservable).
//!
//! Anything not provably foldable stays an ordered rule, so the optimizer can
//! never change routing — at worst it folds less.

use std::ops::RangeInclusive;

use crate::model::{Chain, DomainSet, IrRule, MapField, Match, NamedSet, Ruleset, SetData, Side, Verdict, VerdictMap};

/// Optimize a ruleset's entry chain. Other chains (none in the degenerate
/// embedding) are passed through unchanged.
pub fn compile(rs: Ruleset) -> Ruleset {
	let Ruleset {
		mut sets,
		mut maps,
		chains,
		entry,
	} = rs;

	let mut out_chains = Vec::with_capacity(chains.len());
	for (idx, chain) in chains.into_iter().enumerate() {
		if idx == entry {
			out_chains.push(optimize_chain(chain, &mut sets, &mut maps));
		} else {
			out_chains.push(chain);
		}
	}

	Ruleset {
		sets,
		maps,
		chains: out_chains,
		entry,
	}
}

fn optimize_chain(chain: Chain, sets: &mut Vec<NamedSet>, maps: &mut Vec<VerdictMap>) -> Chain {
	let rules = chain.rules;
	let mut out: Vec<IrRule> = Vec::new();
	let mut i = 0;

	while i < rules.len() {
		// Pass 1: longest contiguous run of identical (stmts, verdict).
		let j = run_end(&rules, i, |a, b| a.stmts == b.stmts && a.verdict == b.verdict);
		if j - i >= 2 {
			out.push(bucket_same_verdict(&rules[i..j], sets));
			i = j;
			continue;
		}

		// Pass 2: longest contiguous run of single Port leaves on one side.
		let k = run_end(&rules, i, same_port_side_single_leaf);
		if k - i >= 2 {
			if let Some(rule) = try_port_vmap(&rules[i..k], maps) {
				out.push(rule);
				i = k;
				continue;
			}
		}

		out.push(rules[i].clone());
		i += 1;
	}

	Chain {
		name: chain.name,
		policy: chain.policy,
		rules: out,
	}
}

/// Return the end index (exclusive) of the maximal run starting at `start` for
/// which `pred(rules[start], rules[m])` holds for every `m` in the run.
fn run_end(rules: &[IrRule], start: usize, pred: impl Fn(&IrRule, &IrRule) -> bool) -> usize {
	let anchor = &rules[start];
	let mut m = start + 1;
	while m < rules.len() && pred(anchor, &rules[m]) {
		m += 1;
	}
	m
}

fn same_port_side_single_leaf(a: &IrRule, b: &IrRule) -> bool {
	match (&a.matches, &b.matches) {
		(Match::Port { side: sa, .. }, Match::Port { side: sb, .. }) => sa == sb,
		_ => false,
	}
}

/// Pass 1: collapse a contiguous, identical-(stmts,verdict) run into one rule
/// whose match is an `Any` of per-type set memberships plus any
/// non-set-able leaves kept verbatim.
fn bucket_same_verdict(run: &[IrRule], sets: &mut Vec<NamedSet>) -> IrRule {
	let mut domains = DomainSet::default();
	let mut dst_ips = Vec::new();
	let mut src_ips = Vec::new();
	let mut dst_ports: Vec<RangeInclusive<u16>> = Vec::new();
	let mut src_ports: Vec<RangeInclusive<u16>> = Vec::new();
	let mut alts: Vec<Match> = Vec::new();

	for r in run {
		match &r.matches {
			Match::Domain(test) => push_domain(&mut domains, test),
			Match::Ip { side: Side::Dst, net } => dst_ips.push(*net),
			Match::Ip { side: Side::Src, net } => src_ips.push(*net),
			Match::Port { side: Side::Dst, range } => dst_ports.push(range.clone()),
			Match::Port { side: Side::Src, range } => src_ports.push(range.clone()),
			// Proto / Predicate / compound / Always / InSet: not set-able; keep.
			other => alts.push(other.clone()),
		}
	}

	if !domains.exact.is_empty() || !domains.suffix.is_empty() || !domains.keyword.is_empty() {
		let id = intern(sets, SetData::Domains(domains));
		// Domain sets ignore `side`; Dst is conventional.
		alts.push(Match::InSet {
			side: Side::Dst,
			set: id,
		});
	}
	if !dst_ips.is_empty() {
		let id = intern(sets, SetData::Ips(dst_ips));
		alts.push(Match::InSet {
			side: Side::Dst,
			set: id,
		});
	}
	if !src_ips.is_empty() {
		let id = intern(sets, SetData::Ips(src_ips));
		alts.push(Match::InSet {
			side: Side::Src,
			set: id,
		});
	}
	if !dst_ports.is_empty() {
		let id = intern(sets, SetData::Ports(dst_ports));
		alts.push(Match::InSet {
			side: Side::Dst,
			set: id,
		});
	}
	if !src_ports.is_empty() {
		let id = intern(sets, SetData::Ports(src_ports));
		alts.push(Match::InSet {
			side: Side::Src,
			set: id,
		});
	}

	let matches = if alts.len() == 1 {
		alts.into_iter().next().unwrap()
	} else {
		Match::Any(alts)
	};

	IrRule {
		matches,
		stmts: run[0].stmts.clone(),
		verdict: run[0].verdict.clone(),
	}
}

fn push_domain(ds: &mut DomainSet, test: &crate::model::DomainTest) {
	use crate::model::DomainTest::*;
	match test {
		Exact(s) => ds.exact.push(s.clone()),
		Suffix(s) => ds.suffix.push(s.clone()),
		Keyword(s) => ds.keyword.push(s.clone()),
	}
}

fn intern(sets: &mut Vec<NamedSet>, data: SetData) -> usize {
	sets.push(NamedSet { data });
	sets.len() - 1
}

/// Pass 2: build a port verdict map from a same-side run of single `Port`
/// leaves, iff every statement list is empty and the ranges are pairwise
/// disjoint. Returns `None` (fall back to ordered rules) otherwise.
fn try_port_vmap(run: &[IrRule], maps: &mut Vec<VerdictMap>) -> Option<IrRule> {
	if run.iter().any(|r| !r.stmts.is_empty()) {
		return None;
	}

	let side = match &run[0].matches {
		Match::Port { side, .. } => *side,
		_ => return None,
	};

	let mut entries: Vec<(RangeInclusive<u16>, Verdict)> = Vec::with_capacity(run.len());
	for r in run {
		let Match::Port { range, .. } = &r.matches else {
			return None;
		};
		entries.push((range.clone(), r.verdict.clone()));
	}

	// Require pairwise-disjoint keys — the soundness condition for dropping
	// order. Overlapping ranges (e.g. via differing first-match verdicts) bail.
	for a in 0..entries.len() {
		for b in (a + 1)..entries.len() {
			if ranges_intersect(&entries[a].0, &entries[b].0) {
				return None;
			}
		}
	}

	maps.push(VerdictMap {
		side,
		field: MapField::Port,
		entries,
		default: None,
	});
	let map_id = maps.len() - 1;

	Some(IrRule {
		matches: Match::Always,
		stmts: Vec::new(),
		verdict: Verdict::Map(map_id),
	})
}

fn ranges_intersect(a: &RangeInclusive<u16>, b: &RangeInclusive<u16>) -> bool {
	a.start() <= b.end() && b.start() <= a.end()
}
