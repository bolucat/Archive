//! Clash / Mihomo rule syntax (`MetaCubeX/mihomo`).
//!
//! Mihomo rule lines (`DOMAIN-SUFFIX,google.com,proxy`) are the canonical
//! surface form of this crate's shared rule representation:
//! [`wind_core::rule::Rule`] mirrors Mihomo's rule types one-to-one. This
//! module is the `syntax`-level entry point for turning Mihomo rule text into
//! that representation.
//!
//! Because the shared representation *is* the Mihomo model, "conversion" here
//! is parsing — no lossy mapping step is involved.

pub use wind_core::rule::{Rule, RuleParseError};

/// Parse a single Mihomo rule line, e.g. `DOMAIN-SUFFIX,google.com,proxy`.
///
/// Blank lines and `#` comments are rejected as
/// [`RuleParseError::EmptyOrComment`]; use [`parse_multiline`] or
/// [`parse_lines`] to skip them.
pub fn parse_rule(line: &str) -> Result<Rule, RuleParseError> {
	Rule::parse(line)
}

/// Parse Mihomo rules from a multiline string into the shared representation.
///
/// Blank lines and `#` comments are skipped. Returns an error identifying the
/// first invalid line by its 1-based line number.
pub fn parse_multiline(input: &str) -> eyre::Result<Vec<Rule>> {
	let mut rules = Vec::new();
	for (idx, raw) in input.lines().enumerate() {
		let line = raw.trim();
		if line.is_empty() || line.starts_with('#') {
			continue;
		}
		match Rule::parse(line) {
			Ok(rule) => rules.push(rule),
			Err(e) => eyre::bail!("invalid mihomo rule on line {} ({line:?}): {e}", idx + 1),
		}
	}
	Ok(rules)
}

/// Convert an iterator of Mihomo rule lines into the shared representation.
///
/// Each item MAY itself span multiple lines; blank lines and `#` comments are
/// skipped. Returns an error identifying the first invalid line. This is the
/// form [`AclEngineBuilder::clash_rules`](crate::AclEngineBuilder::clash_rules)
/// uses.
pub fn parse_lines<I, S>(lines: I) -> eyre::Result<Vec<Rule>>
where
	I: IntoIterator<Item = S>,
	S: AsRef<str>,
{
	let joined = lines
		.into_iter()
		.map(|s| s.as_ref().to_string())
		.collect::<Vec<_>>()
		.join("\n");
	parse_multiline(&joined)
}

#[cfg(test)]
mod tests {
	use wind_core::rule::{MatchContext, RuleType};

	use super::*;

	#[test]
	fn parse_single_rule() {
		let rule = parse_rule("DOMAIN-SUFFIX,google.com,proxy").unwrap();
		assert_eq!(rule.target, "proxy");
		assert!(matches!(rule.rule_type, RuleType::DomainSuffix(ref s) if s == "google.com"));
		assert!(rule.matches(&MatchContext {
			domain: Some("www.google.com"),
			..Default::default()
		}));
	}

	#[test]
	fn parse_rule_rejects_comment() {
		assert!(matches!(parse_rule("# a comment"), Err(RuleParseError::EmptyOrComment)));
	}

	#[test]
	fn multiline_skips_comments_and_blanks() {
		let rules = parse_multiline(
			"
			# leading comment
			DOMAIN-SUFFIX,google.com,proxy

			IP-CIDR,10.0.0.0/8,direct
			MATCH,reject
			",
		)
		.unwrap();
		assert_eq!(rules.len(), 3);
		assert_eq!(rules[2].target, "reject");
	}

	#[test]
	fn multiline_reports_offending_line_number() {
		let err = parse_multiline("DOMAIN-SUFFIX,ok.com,proxy\nNOPE,bad,target")
			.unwrap_err()
			.to_string();
		assert!(err.contains("line 2"), "error should name line 2: {err}");
	}

	#[test]
	fn parse_lines_joins_and_parses() {
		let rules = parse_lines(["DOMAIN,a.com,proxy", "# skip", "DST-PORT,443,direct"]).unwrap();
		assert_eq!(rules.len(), 2);
	}
}
