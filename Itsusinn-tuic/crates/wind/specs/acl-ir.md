RFC: wind ACL Intermediate Representation (acl-ir)
Category: Informational
Date: June 2026

# wind ACL Intermediate Representation (acl-ir)

## Status of This Memo

This document specifies the in-repository ACL intermediate representation used
by `wind-acl`. It is not an Internet standard and it does not define a wire
protocol. The normative implementation references are the `wind-acl` and
`wind-core` crates in this workspace.

## Abstract

`acl-ir` is the internal routing program format used by `wind-acl`. It lowers
Clash/Mihomo rule lines (and any externally converted `wind_core::rule::Rule`
values, such as tuic-server's legacy ACL dialect) into a single `Ruleset` that
preserves first-match-wins routing, default outbound fallback, and the legacy
`wind_core::rule::Rule` matching semantics.

The IR is shaped like a small nftables-inspired engine: boolean match
expressions, set membership, verdict maps, ordered chains, statements, and
terminal verdicts. The v1 implementation deliberately keeps the compatibility
surface narrow. Optimizer-relevant leaves are represented as typed IR nodes
(domain exact/suffix/keyword, IP CIDR, source/destination port, and network
protocol); every other Mihomo rule type is carried as `Match::Predicate` and
delegates evaluation to `wind_core::rule::Rule`.

## Table of Contents

1. [Introduction](#1-introduction)
2. [Conventions and Terminology](#2-conventions-and-terminology)
3. [Compilation Pipeline](#3-compilation-pipeline)
4. [Data Model](#4-data-model)
5. [Evaluation Semantics](#5-evaluation-semantics)
6. [Degenerate Embedding](#6-degenerate-embedding)
7. [Surface Dialect Lowering](#7-surface-dialect-lowering)
8. [Order-Preserving Optimization](#8-order-preserving-optimization)
9. [Implementation Scope and Extensions](#9-implementation-scope-and-extensions)
10. [Security Considerations](#10-security-considerations)
11. [References](#11-references)

## 1. Introduction

An ACL router answers one question: given a connection context, which outbound
should serve it, or should it be rejected? Historically, wind used a flat
`Vec<wind_core::rule::Rule>` evaluated in declaration order. That model is
simple and compatible with Clash/Mihomo syntax, but it is hard to optimize and
does not give converted rules (e.g. tuic-server's legacy ACL) a structured
target.

`acl-ir` provides that structured target. It has three goals:

- preserve existing routing decisions exactly for rules that already worked in
  the legacy engine;
- expose enough typed structure to build safe sets and verdict maps;
- leave explicit extension points for richer routing constructs without
  forcing all of them into the initial implementation.

The IR borrows its engine shape from nftables, but it is not an nftables
frontend. It runs inside wind, reads `wind_core::rule::MatchContext`, and keeps
proxy-specific layer-7 concepts such as domains, process identity, inbound
metadata, GeoIP/GeoSite lookups, and rule-set placeholders.

## 2. Conventions and Terminology

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT",
"SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this
document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when,
and only when, they appear in all capitals.

- **Match**: a boolean expression evaluated against a `MatchContext`.
- **Predicate**: a `wind_core::rule::Rule` embedded in the IR as an opaque
  matcher.
- **Statement**: a non-terminal action associated with a matching rule.
- **Verdict**: a routing or control-flow decision: forward, reject, drop,
  return, jump, goto, or verdict-map lookup.
- **Rule**: one match expression, zero or more statements, and one verdict.
- **Chain**: an ordered list of rules. The entry chain also has the observable
  fallback policy.
- **Set**: an unordered lookup table used by `Match::InSet`.
- **Verdict map**: an unordered lookup table from a key range to a verdict.
- **Degenerate embedding**: the single-chain IR form that is equivalent to the
  legacy flat rule engine.
- **First-match-wins**: the first rule in declaration order whose match is true
  decides the route.

Rust snippets in this document are illustrative, but they follow the public
types in `crates/wind-acl/src/model.rs`.

## 3. Compilation Pipeline

`AclEngineBuilder` builds an engine in this order:

1. Parse real Hysteria 2 (apernet) ACL entries through `syntax::apernet` and
   convert them to `wind_core::rule::Rule` values with `apernet::acl_to_rules`.
2. Parse Clash/Mihomo rule lines through `syntax::metacubex`.
3. Concatenate apernet-derived rules before Clash/Mihomo rules.
4. Build the degenerate `Ruleset` with `Ruleset::from_rules`.
5. Run `compile`, the order-preserving optimizer.
6. At route time, build a `MatchContext` from `TargetAddr`, protocol, and any
   configured static inbound metadata, then evaluate the `Ruleset`.

The apernet-before-Clash ordering is normative for `AclEngine`: if both
surfaces produce a rule matching the same connection, the apernet-derived rule
wins.

Callers with *other* rule sources convert them to `wind_core::rule::Rule`
themselves and route those values directly (via `wind_core::AclRouter` or the
degenerate embedding). tuic-server does this for its space-separated `legacy`
dialect: it lowers entries with `tuic_server::legacy::acl_to_rules` and
concatenates the converted rules before its Clash/Mihomo rules.

`AclEngine::route` currently fills only the fields available at that call site:
destination domain or IP, destination port, network protocol, optional inbound
name, and optional inbound type. Source IP, source port, inbound user, process
metadata, and external GeoIP/ASN/GeoSite lookup functions are absent unless a
caller evaluates a `Ruleset` directly with a richer `MatchContext`.

## 4. Data Model

### 4.1. Matches

```rust
enum Side {
    Dst,
    Src,
}

enum Match {
    All(Vec<Match>),
    Any(Vec<Match>),
    Not(Box<Match>),
    Always,

    Ip     { side: Side, net: IpNet },
    Port   { side: Side, range: RangeInclusive<u16> },
    Proto  (NetworkType),
    Domain (DomainTest),

    InSet { side: Side, set: usize },
    Predicate(Arc<wind_core::rule::Rule>),
}

enum DomainTest {
    Exact(String),
    Suffix(String),
    Keyword(String),
}
```

`All([])` is true and `Any([])` is false by normal boolean convention, but
lowering code SHOULD avoid constructing empty logical nodes. `Always` is the IR
form of `MATCH`.

`DomainTest::Suffix` matches both the exact suffix and subdomains of that
suffix. Exact and suffix comparisons are ASCII case-insensitive. Keyword
matching is also ASCII case-insensitive.

`Predicate` is the compatibility escape hatch. It MUST evaluate by calling
`Rule::matches(ctx)`, so opaque rules keep the exact behavior of
`wind_core::rule`, including `RULE-SET` currently matching false and
`SUB-RULE` currently using the legacy contained-rule semantics.

### 4.2. Sets

```rust
struct NamedSet {
    data: SetData,
}

enum SetData {
    Domains(DomainSet),
    Ips(Vec<IpNet>),
    Ports(Vec<RangeInclusive<u16>>),
}

struct DomainSet {
    exact: Vec<String>,
    suffix: Vec<String>,
    keyword: Vec<String>,
}
```

The implementation stores sets in `Ruleset::sets` and refers to them by table
index. The term "NamedSet" is retained for the conceptual role; a future
serialized form MAY assign stable names.

Membership is type-directed:

- `Domains` reads `ctx.domain` and ignores `side`;
- `Ips` reads `ctx.dst_ip` or `ctx.src_ip` according to `side`;
- `Ports` reads `ctx.dst_port` or `ctx.src_port` according to `side`.

### 4.3. Statements and Verdicts

```rust
enum Statement {
    Counter,
    Log(String),
    Mark(u32),
    Dnat(String),
}

enum Verdict {
    Forward(String),
    Reject(String),
    Drop,
    Return,
    Jump(String),
    Goto(String),
    Map(usize),
}
```

Statements are non-terminal. An implementation that exposes statement side
effects MUST execute them in rule order before applying the rule's verdict. The
current `RouteAction` API observes only the routing decision, so the built-in
evaluator ignores statement side effects. The degenerate embedding never emits
statements.

`Forward` selects a named outbound. `Reject` rejects with a reason string.
`Drop` is available in the IR, but `wind_core::RouteAction` currently has no
drop variant; the built-in evaluator reports `Drop` as a rejection with the
reason `"dropped"`.

### 4.4. Verdict Maps, Chains, and Rulesets

```rust
enum MapField {
    Port,
}

struct VerdictMap {
    side: Side,
    field: MapField,
    entries: Vec<(RangeInclusive<u16>, Verdict)>,
    default: Option<Verdict>,
}

struct IrRule {
    matches: Match,
    stmts: Vec<Statement>,
    verdict: Verdict,
}

struct Chain {
    name: String,
    policy: Verdict,
    rules: Vec<IrRule>,
}

struct Ruleset {
    sets: Vec<NamedSet>,
    maps: Vec<VerdictMap>,
    chains: Vec<Chain>,
    entry: usize,
}
```

In v1, verdict maps key only on source or destination port ranges. The
optimizer only creates maps whose ranges are pairwise disjoint.

`entry` is an index into `chains`; evaluation always starts there.

## 5. Evaluation Semantics

Evaluation starts at `Ruleset::entry` and scans the entry chain from top to
bottom.

For each rule:

1. Evaluate `rule.matches` against the supplied `MatchContext`.
2. If the match is false, continue to the next rule.
3. If the match is true, process `rule.stmts`, then apply `rule.verdict`.

Terminal verdicts behave as follows:

- `Forward(outbound)` terminates with `RouteAction::Forward(outbound)`.
- `Reject(reason)` terminates with `RouteAction::Reject(reason)`.
- `Drop` terminates as a rejection in the current public API.

Control-flow verdicts behave as follows:

- `Return` produces fallthrough to the caller.
- `Jump(name)` evaluates the named chain. If that chain produces a terminal
  verdict, the terminal verdict wins. If it falls through, evaluation resumes at
  the next rule after the jump.
- `Goto(name)` evaluates the named chain without establishing a semantic return
  point. In the current evaluator, a non-terminal result from the target chain
  is still represented as fallthrough at the call site. Configurations SHOULD
  use explicit terminal rules in `Goto` targets until stricter tail-call
  semantics are implemented.
- `Map(index)` looks up the current key in `Ruleset::maps[index]`. A hit applies
  the entry verdict. A miss with `default` applies the default verdict. A miss
  without `default` falls through to the next rule.

If the entry chain ultimately falls through, `Ruleset::route` applies the entry
chain policy. Non-entry chain policies are reserved for future multi-base-chain
semantics; v1 callers SHOULD use explicit terminal fallback rules inside
subchains.

Implementations MUST prevent unbounded chain recursion. The current evaluator
uses a maximum chain depth of 64 and treats excess depth as fallthrough.

## 6. Degenerate Embedding

`Ruleset::from_rules(rules, default_outbound)` embeds legacy rules as a
single-chain ruleset:

```rust
Ruleset {
    sets: vec![],
    maps: vec![],
    entry: 0,
    chains: vec![Chain {
        name: "main".to_string(),
        policy: Verdict::Forward(default_outbound),
        rules: rules.into_iter().map(rule_to_ir).collect(),
    }],
}
```

This embedding is normative: before optimization, routing MUST match the legacy
first-match-wins engine for the same `MatchContext`. After optimization,
routing MUST still match it.

The following rule types become typed IR leaves:

| `wind_core::rule::RuleType` | IR match |
| --- | --- |
| `Domain` | `Domain(Exact)` |
| `DomainSuffix` | `Domain(Suffix)` |
| `DomainKeyword` | `Domain(Keyword)` |
| `IpCidr`, `IpSuffix` | `Ip { side: Dst }` |
| `IpCidr6` | `Ip { side: Dst }` |
| `SrcIpCidr`, `SrcIpSuffix` | `Ip { side: Src }` |
| `DstPort`, `DstPortRange` | `Port { side: Dst }` |
| `SrcPort`, `SrcPortRange` | `Port { side: Src }` |
| `Network` | `Proto` |
| `Match` | `Always` |

All other rule types are embedded as `Predicate(Arc<Rule>)`.

Targets are mapped as follows:

- `reject`, `block`, and `deny`, case-insensitively, become `Verdict::Reject`
  with a canonical reason string;
- every other target becomes `Verdict::Forward(target)`, preserving the target
  spelling.

The canonical reject reason is not a routing semantic. Tests compare rejection
as a decision, not as a string payload.

## 7. Surface Dialect Lowering

### 7.1. Clash/Mihomo

Clash/Mihomo lines are parsed by `wind_core::rule::Rule::parse`. Blank lines
and `#` comments are skipped by multiline helpers.

The shared rule model supports the following broad classes:

- domain rules: `DOMAIN`, `DOMAIN-SUFFIX`, `DOMAIN-KEYWORD`,
  `DOMAIN-WILDCARD`, `DOMAIN-REGEX`, `GEOSITE`;
- destination IP rules: `IP-CIDR`, `IP-CIDR6`, `IP-SUFFIX`, `IP-ASN`, `GEOIP`;
- source IP rules: `SRC-IP-CIDR`, `SRC-IP-SUFFIX`, `SRC-IP-ASN`, `SRC-GEOIP`;
- ports: `DST-PORT`, `SRC-PORT`, including inclusive ranges;
- inbound metadata: `IN-PORT`, `IN-TYPE`, `IN-USER`, `IN-NAME`;
- process and user identity: `PROCESS-PATH`, `PROCESS-PATH-REGEX`,
  `PROCESS-NAME`, `PROCESS-NAME-REGEX`, `UID`;
- protocol and traffic metadata: `NETWORK`, `DSCP`;
- compounds and catch-all: `AND`, `OR`, `NOT`, `SUB-RULE`, `RULE-SET`, `MATCH`.

Only the subset listed in Section 6 is typed in the IR today. The rest remains
semantically correct through `Predicate`.

### 7.2. tuic-server legacy ACL

The tuic-server legacy ACL is a space-separated dialect specific to tuic-server
(it is **not** Hysteria's ACL, which uses a `outbound(address, proto/port,
hijack)` function-call form). Its parser and lowering live in the `tuic-server`
crate's `legacy` module, not in `wind-acl`; this section documents the lowering
because its output is embedded through Section 6. Lines have the shape:

```text
<outbound> [address] [ports] [hijack]
```

Lowering first converts each `AclRule` to one or more `wind_core::rule::Rule`
values (`tuic_server::legacy::acl_to_rules`), then embeds those rules through
Section 6.

Address lowering:

| legacy address | Lowered rule type |
| --- | --- |
| omitted or `*` | `MATCH` |
| IPv4 literal | `IP-CIDR` host route `/32` |
| IPv6 literal | `IP-CIDR` host route `/128` |
| CIDR | `IP-CIDR` |
| domain | `DOMAIN` |
| `*.example.com` | `DOMAIN-SUFFIX,example.com` |
| `suffix:example.com` | `DOMAIN-SUFFIX,example.com` |
| `localhost` | `127.0.0.0/8` and `::1/128` |
| `private` | `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `127.0.0.0/8`, `169.254.0.0/16`, `::1/128`, `fc00::/7`, `fe80::/10` |

Port lowering:

- an omitted port list adds no port condition;
- `80` becomes `DST-PORT,80`;
- `1000-2000` becomes `DST-PORT,1000-2000`;
- `tcp/443` or `udp/53` becomes `AND(NETWORK, DST-PORT)`.

When an address condition and a port condition are both present, lowering emits
an `AND(address, port)` rule for each combination.

Outbound lowering:

- `allow` and `default` normalize to the outbound name `default`;
- every other outbound string is preserved until target-to-verdict mapping.

`hijack` is parsed and retained on `AclRule`, but it is not currently honored.
`Statement::Dnat` is the intended IR home for future redirect support.

### 7.3. apernet ACL (real Hysteria 2)

The apernet dialect is the genuine Hysteria 2 ACL — a **function-call** form,
`outbound(address[, proto/port[, hijack]])` — parsed by `syntax::apernet` in
`wind-acl`, mirroring apernet/hysteria's `extras/outbounds/acl` parser. Lowering
converts each `AclRule` to one or more `wind_core::rule::Rule` values
(`apernet::acl_to_rules`), then embeds them through Section 6.

Address dispatch is ordered and structural (first match wins, after lower-casing
and trailing-dot trimming):

| apernet address | Lowered rule type |
| --- | --- |
| `all` or `*` | `MATCH` |
| IPv4 literal | `IP-CIDR` host route `/32` |
| IPv6 literal | `IP-CIDR` host route `/128` |
| CIDR (v4/v6) | `IP-CIDR` |
| `geoip:<cc>` | `GEOIP,<cc>` |
| `geosite:<name>[@attr…]` | `GEOSITE,<name>` (attributes dropped — see below) |
| `suffix:<domain>` | `DOMAIN-SUFFIX,<domain>` |
| `*`-bearing domain (`*.example.com`, `*.google.*`) | `DOMAIN-WILDCARD,<pattern>` |
| exact domain | `DOMAIN,<domain>` |

`suffix:` matches the apex and subdomains; an exact domain matches only itself; a
`*`-bearing pattern is a glob whose `*` spans label boundaries (so `*.example.com`
matches subdomains but **not** the bare apex).

Proto/port lowering (`<proto>` ∈ {`tcp`, `udp`, `*`}; `<port>` ∈ {`*`, single,
`lo-hi`}):

- omitted, `*`, or `*/*` add no port condition (both protocols, all ports);
- `tcp` / `tcp/*` (and the `udp` forms) become `NETWORK,<proto>` (no port);
- `*/<port>` becomes `DST-PORT,<port>` / `DST-PORT,lo-hi` (no protocol);
- `tcp/<port>` (and `udp`) becomes `AND(NETWORK, DST-PORT)`;
- a resulting start port of `0` is apernet's "any port" sentinel and adds no
  port condition.

When an address condition and a port condition are both present, lowering emits
an `AND(address, port)` rule for each combination (an `all`/`*` address is
match-everything, so only the port conditions are emitted).

Outbound lowering: the outbound name is passed through verbatim. The reject
keywords (`reject`/`block`/`deny`, case-insensitive) become a reject verdict via
Section 6; every other name (`direct`, `default`, or a custom outbound) is a
forward target.

Two apernet forms are faithful but not fully representable in the v1 IR:

- **geosite attributes** (`geosite:google@ads`) have no slot in `GeoSite(String)`,
  so they are dropped during lowering (retained on the parsed `AclRule`) with a
  warning;
- **hijack** (the optional IP third argument) cannot be expressed in a
  `RuleType`; it is parsed and retained but dropped during lowering with a
  warning. `Statement::Dnat` is the intended future home.

The dialect is deliberately stricter than upstream on degenerate input (it
rejects empty addresses, whitespace-only arguments, and arguments containing a
literal `)`), and it differs in two benign ways that only affect non-DNS input:
`?` in a `*`-bearing pattern is a single-character wildcard (upstream matches `?`
literally), and matching applies no IDNA `ToUnicode` to the host (punycode `xn--`
hosts compare verbatim) with ASCII-only case folding.

## 8. Order-Preserving Optimization

The ordered chain is the ground truth. Sets and verdict maps are unordered
lookup structures, so the optimizer MAY introduce them only when doing so
cannot change first-match-wins behavior.

The current optimizer runs only on the entry chain. Other chains are passed
through unchanged.

### 8.1. Pass 1: contiguous same-verdict bucketing

The optimizer finds the longest contiguous run starting at the current rule for
which every rule has identical `(stmts, verdict)`.

Such a run MAY always be replaced by one rule because every matching member
produces the same observable routing decision, and no unrelated rule is moved
across the run boundary.

Within the replacement rule:

- domain exact/suffix/keyword leaves become one `SetData::Domains` set;
- destination and source IP leaves become separate `SetData::Ips` sets;
- destination and source port leaves become separate `SetData::Ports` sets;
- non-settable leaves, including `Proto`, `Predicate`, `Always`, compound
  expressions, and existing `InSet` nodes, are kept as alternatives.

The replacement match is either the single alternative or `Match::Any(alts)`.

### 8.2. Pass 2: port verdict maps

If Pass 1 does not consume the current position, the optimizer looks for the
longest contiguous run of single `Port` leaves on the same side.

That run MAY become a `VerdictMap` only if:

- every rule has an empty statement list;
- every key is an inclusive port range;
- all ranges are pairwise disjoint.

If any two ranges overlap, the run MUST remain ordered. This preserves cases
such as:

```text
DST-PORT,1000-2000,proxy
DST-PORT,1500,direct
```

Port `1500` must still route to `proxy`, because the first rule wins.

### 8.3. No Other Reordering

The v1 optimizer does not perform non-adjacent hoisting, IP verdict maps,
domain verdict maps, or cross-chain optimization. These are future extensions
and MUST preserve the same order-invariance rule if added.

## 9. Implementation Scope and Extensions

The v1 implementation intentionally distinguishes between IR capacity and
engine behavior:

- `RULE-SET` is still a `wind_core::rule::RuleType::RuleSet` predicate and
  therefore currently matches false.
- `SUB-RULE` is still evaluated through the legacy `RuleType::SubRule`
  semantics when carried by `Predicate`.
- GeoIP, ASN, and GeoSite rules require lookup functions in `MatchContext`.
  `AclEngine::route` does not currently supply those functions.
- Source IP, source port, inbound user, process fields, and UID require the
  caller to provide them in `MatchContext`.
- `Dnat` exists in the IR, but the legacy ACL `hijack` field is not yet emitted
  or executed.
- `Drop` exists in the IR, but the public `RouteAction` currently reports it as
  rejection.
- sing-box route-rule parsing is not part of v1. The IR can grow typed leaves
  for sing-box-style environment matchers later.

Any future extension MUST keep the degenerate embedding equivalent to the
legacy rule engine and MUST keep optimization semantics order-preserving.

## 10. Security Considerations

- **Optimizer safety.** If a transformation cannot prove that order is
  unobservable, it MUST leave rules ordered. This fail-closed rule prevents
  optimizations from silently changing routing or unblocking traffic.
- **Missing context.** A rule that reads an absent `MatchContext` field does not
  match. Deployments relying on source, process, inbound-user, GeoIP, ASN, or
  GeoSite rules MUST ensure those fields or lookup functions are populated.
- **Guard behavior.** Loopback/private guards run before IR evaluation. If a
  guard is enabled, a resolver is REQUIRED at build time so domain targets can
  be resolved before the guard decision.
- **Redirect behavior.** The legacy ACL `hijack` field is parsed but not honored. Enabling
  `Dnat` in the future changes traffic destination and SHOULD be explicit and
  observable in logs.
- **Chain cycles.** Implementations MUST bound chain recursion. The current
  depth limit is 64.
- **Reject keywords.** The strings `reject`, `block`, and `deny` are reserved
  rejection targets, matched case-insensitively.

## 11. References

- **[RFC2119]** Bradner, S., "Key words for use in RFCs to Indicate
  Requirement Levels", BCP 14, RFC 2119, March 1997.
- **[RFC8174]** Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key
  Words", BCP 14, RFC 8174, May 2017.
- `crates/wind-acl/src/model.rs`, `embed.rs`, `eval.rs`, and `optimize.rs`.
- `crates/wind-core/src/rule.rs`.
- MetaCubeX/Mihomo rule syntax.
- apernet/hysteria ACL syntax (`wind-acl` crate, `syntax::apernet` module).
- tuic-server legacy ACL syntax (`tuic-server` crate, `legacy` module).
- nftables concepts: sets, maps, chains, statements, and verdicts.
