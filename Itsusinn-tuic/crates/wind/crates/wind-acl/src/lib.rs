//! `wind-acl` — an nftables-shaped intermediate representation for wind ACL
//! routing.
//!
//! This crate implements the design in `specs/acl-ir.md`: a [`Ruleset`] of
//! typed [`Match`] expressions, [`NamedSet`]s, [`VerdictMap`]s, and [`Chain`]s
//! evaluated with statement-then-verdict, chain-jump semantics. It keeps the L7
//! match vocabulary (domain / process / inbound identity / …) that nftables
//! lacks by delegating those leaves to `wind_core::rule::Rule` via
//! [`Match::Predicate`].
//!
//! # Pipeline
//!
//! ```ignore
//! use wind_acl::{Ruleset, compile};
//! use wind_core::rule::Rule;
//!
//! let rules: Vec<Rule> = Rule::parse_rules(config)
//!     .into_iter()
//!     .filter_map(Result::ok)
//!     .collect();
//!
//! // 1. Degenerate embedding — byte-for-byte equivalent to the legacy engine.
//! let rs = Ruleset::from_rules(rules, "direct");
//! // 2. Order-preserving optimization — folds safe runs into sets / maps.
//! let rs = compile(rs);
//! // 3. Evaluate.
//! let action = rs.route(&ctx);
//! ```
//!
//! The degenerate embedding ([`Ruleset::from_rules`]) is normatively equivalent
//! to the first-match-wins engine, and [`compile`] preserves that equivalence
//! (see the differential tests in `tests/`).
//!
//! # Scope of this first implementation
//!
//! * Typed leaves cover the optimizer-relevant subset (domain exact/suffix/
//!   keyword, IP CIDR, port, protocol). Every other legacy rule kind — geoip,
//!   asn, geosite, process, uid, dscp, inbound, domain wildcard/regex, and the
//!   `AND`/`OR`/`NOT`/`SUB-RULE` compounds — is carried as [`Match::Predicate`]
//!   and still evaluates correctly.
//! * The optimizer implements Pass 1 (contiguous same-verdict bucketing, always
//!   safe) and Pass 2 (port verdict maps over disjoint keys). IP/domain verdict
//!   maps and the optional non-adjacent hoisting (spec §7.6) are future work.

mod embed;
mod engine;
mod eval;
mod model;
mod optimize;
pub mod syntax;

pub use engine::{AclEngine, AclEngineBuilder, GuardConfig};
pub use model::{
	Chain, DomainSet, DomainTest, IrRule, MapField, Match, NamedSet, Ruleset, SetData, Side, Statement, Verdict, VerdictMap,
};
pub use optimize::compile;
