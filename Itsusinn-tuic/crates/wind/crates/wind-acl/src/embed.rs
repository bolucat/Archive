//! The degenerate embedding: `Vec<wind_core::rule::Rule>` → [`Ruleset`].
//!
//! Per `specs/acl-ir.md` §5 this is normative: the produced ruleset MUST route
//! identically to the legacy first-match-wins engine. It builds a single base
//! chain whose policy is `Forward(default_outbound)` and whose rules carry one
//! match plus a `Forward`/`Reject` verdict — no sets, maps, or extra chains.
//! [`crate::optimize::compile`] may then fold it into the richer forms.

use std::sync::Arc;

use ipnet::IpNet;
use wind_core::rule::{Rule, RuleType};

use crate::model::{Chain, DomainTest, IrRule, Match, Ruleset, Side, Verdict};

/// Canonical reject reason. The legacy engine builds a per-rule message
/// (`rejected by rule: …`); the routing decision is "reject", and using one
/// reason here lets adjacent reject rules bucket together. See
/// [`Verdict::Reject`].
pub(crate) const REJECT_REASON: &str = "rejected by rule";

impl Ruleset {
	/// Build the degenerate single-chain ruleset from legacy rules.
	///
	/// Consumes `rules` (legacy `Rule` is not `Clone`); rules without a bespoke
	/// typed match are moved into [`Match::Predicate`].
	pub fn from_rules(rules: Vec<Rule>, default_outbound: impl Into<String>) -> Self {
		let ir_rules = rules.into_iter().map(rule_to_ir).collect();
		Ruleset {
			sets: Vec::new(),
			maps: Vec::new(),
			chains: vec![Chain {
				name: "main".to_string(),
				policy: Verdict::Forward(default_outbound.into()),
				rules: ir_rules,
			}],
			entry: 0,
		}
	}
}

/// Map a legacy target string to a verdict, mirroring
/// `wind_acl::engine::rule_target_to_action`.
pub(crate) fn verdict_of(target: &str) -> Verdict {
	match target.to_ascii_lowercase().as_str() {
		"reject" | "block" | "deny" => Verdict::Reject(REJECT_REASON.to_string()),
		_ => Verdict::Forward(target.to_string()),
	}
}

fn rule_to_ir(rule: Rule) -> IrRule {
	let verdict = verdict_of(&rule.target);
	// Decide the typed match by *reading* the rule type; small values are
	// cloned. If no typed variant fits, the whole rule moves into a Predicate.
	let typed: Option<Match> = match &rule.rule_type {
		RuleType::Domain(s) => Some(Match::Domain(DomainTest::Exact(s.clone()))),
		RuleType::DomainSuffix(s) => Some(Match::Domain(DomainTest::Suffix(s.clone()))),
		RuleType::DomainKeyword(s) => Some(Match::Domain(DomainTest::Keyword(s.clone()))),

		RuleType::IpCidr(net) => Some(Match::Ip {
			side: Side::Dst,
			net: *net,
		}),
		RuleType::IpSuffix(net) => Some(Match::Ip {
			side: Side::Dst,
			net: *net,
		}),
		RuleType::IpCidr6(net6) => Some(Match::Ip {
			side: Side::Dst,
			net: IpNet::V6(*net6),
		}),
		RuleType::SrcIpCidr(net) => Some(Match::Ip {
			side: Side::Src,
			net: *net,
		}),
		RuleType::SrcIpSuffix(net) => Some(Match::Ip {
			side: Side::Src,
			net: *net,
		}),

		RuleType::DstPort(p) => Some(Match::Port {
			side: Side::Dst,
			range: *p..=*p,
		}),
		RuleType::DstPortRange(lo, hi) => Some(Match::Port {
			side: Side::Dst,
			range: *lo..=*hi,
		}),
		RuleType::SrcPort(p) => Some(Match::Port {
			side: Side::Src,
			range: *p..=*p,
		}),
		RuleType::SrcPortRange(lo, hi) => Some(Match::Port {
			side: Side::Src,
			range: *lo..=*hi,
		}),

		RuleType::Network(n) => Some(Match::Proto(*n)),
		RuleType::Match => Some(Match::Always),

		// Everything else (geoip/asn/geosite, src-geo/src-asn, process/uid,
		// dscp, inbound, domain wildcard/regex, AND/OR/NOT/SUB-RULE, rule-set)
		// delegates to the legacy matcher.
		_ => None,
	};

	let matches = typed.unwrap_or_else(|| Match::Predicate(Arc::new(rule)));
	IrRule {
		matches,
		stmts: Vec::new(),
		verdict,
	}
}
