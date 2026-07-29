RFC: wind ACL 中间表示 (acl-ir)
Category: Informational
Date: 2026 年 6 月

# wind ACL 中间表示 (acl-ir)

## 本备忘录状态

本文档定义 `wind-acl` 在本仓库内使用的 ACL 中间表示。它不是互联网标准，也不定义线
路协议。规范性实现以本工作区内的 `wind-acl` 和 `wind-core` crate 为准。

## 摘要

`acl-ir` 是 `wind-acl` 使用的内部路由程序格式。它把 Clash/Mihomo 规则行（以及任何
外部转换得到的 `wind_core::rule::Rule`，例如 tuic-server 的 legacy ACL 方言）降低为
同一个 `Ruleset`，同时保留 first-match-wins 路由语义、默认出站兜底，以及旧有
`wind_core::rule::Rule` 的匹配行为。

IR 的形状类似一个小型 nftables 风格引擎：布尔匹配表达式、集合成员检查、verdict
map、有序链、语句和终结 verdict。v1 实现有意保持兼容边界较窄：对优化有价值的叶子
节点使用强类型 IR 节点表示，包括域名 exact/suffix/keyword、IP CIDR、源/目标端口和
网络协议；其它 Mihomo 规则类型通过 `Match::Predicate` 携带，并委托给
`wind_core::rule::Rule` 求值。

## 目录

1. [引言](#1-引言)
2. [约定与术语](#2-约定与术语)
3. [编译流水线](#3-编译流水线)
4. [数据模型](#4-数据模型)
5. [求值语义](#5-求值语义)
6. [退化嵌入](#6-退化嵌入)
7. [表层方言降低](#7-表层方言降低)
8. [保序优化](#8-保序优化)
9. [实现范围与扩展](#9-实现范围与扩展)
10. [安全考虑](#10-安全考虑)
11. [参考资料](#11-参考资料)

## 1. 引言

ACL 路由器回答一个问题：给定连接上下文，应该由哪个出站处理它，或者是否应该拒绝？
wind 过去使用扁平的 `Vec<wind_core::rule::Rule>`，并按声明顺序求值。这个模型简单
且兼容 Clash/Mihomo 语法，但难以优化，也没有给转换后的规则（例如 tuic-server 的
legacy ACL）提供结构化目标。

`acl-ir` 提供这个结构化目标。它有三个目标：

- 对已经由旧引擎支持的规则，精确保留现有路由决策；
- 暴露足够的强类型结构，以便安全地构建集合和 verdict map；
- 为更丰富的路由结构留下明确扩展点，而不强迫 v1 一次实现全部能力。

IR 借鉴 nftables 的引擎形状，但它不是 nftables 前端。它运行在 wind 内部，读取
`wind_core::rule::MatchContext`，并保留代理路由特有的七层概念，例如域名、进程身
份、入站元数据、GeoIP/GeoSite 查找以及 rule-set 占位符。

## 2. 约定与术语

本文档使用中文规范词“必须”、“绝对不能”、“要求”、“应当”、“不应当”、“应该”、
“不应该”、“建议”、“可以”和“可选”。这些词按 BCP 14 [RFC2119] [RFC8174] 解释。

- **Match（匹配）**：针对 `MatchContext` 求值的布尔表达式。
- **Predicate（谓词）**：作为不透明匹配器嵌入 IR 的 `wind_core::rule::Rule`。
- **Statement（语句）**：与命中规则关联的非终结动作。
- **Verdict（裁决）**：路由或控制流决策：forward、reject、drop、return、jump、
  goto 或 verdict-map 查找。
- **Rule（规则）**：一个匹配表达式、零个或多个语句、一个 verdict。
- **Chain（链）**：有序规则列表。入口链还具有可观察的兜底 policy。
- **Set（集合）**：供 `Match::InSet` 使用的无序查找表。
- **Verdict map**：从键范围到 verdict 的无序查找表。
- **退化嵌入**：与旧扁平规则引擎等价的单链 IR 形式。
- **First-match-wins**：按声明顺序，第一个匹配为真的规则决定路由。

本文档中的 Rust 片段是说明性的，但与 `crates/wind-acl/src/model.rs` 中的公开类型保
持一致。

## 3. 编译流水线

`AclEngineBuilder` 按以下顺序构建引擎：

1. 通过 `syntax::apernet` 解析真正的 Hysteria 2（apernet）ACL 条目，并用
   `apernet::acl_to_rules` 转换为 `wind_core::rule::Rule`。
2. 通过 `syntax::metacubex` 解析 Clash/Mihomo 规则行。
3. 把 apernet 派生规则放在 Clash/Mihomo 规则之前并连接起来。
4. 使用 `Ruleset::from_rules` 构建退化 `Ruleset`。
5. 运行保序优化器 `compile`。
6. 路由时，根据 `TargetAddr`、协议和已配置的静态入站元数据构建 `MatchContext`，
   然后对 `Ruleset` 求值。

apernet 先于 Clash 的顺序对 `AclEngine` 是规范性的：如果两种表层规则都能匹配同一
连接，apernet 派生规则获胜。

拥有*其它*规则来源的调用者自行把它们转换为 `wind_core::rule::Rule`，并直接对这些
值路由（通过 `wind_core::AclRouter` 或退化嵌入）。tuic-server 对它的空格分隔 legacy
方言正是这么做：用 `tuic_server::legacy::acl_to_rules` 降低条目，并把转换后的规则连
接在它的 Clash/Mihomo 规则之前。

`AclEngine::route` 目前只填充该调用点可见的字段：目标域名或 IP、目标端口、网络协
议、可选入站名称和可选入站类型。源 IP、源端口、入站用户、进程元数据以及外部
GeoIP/ASN/GeoSite 查找函数不会自动填充，除非调用者直接使用更完整的 `MatchContext`
求值 `Ruleset`。

## 4. 数据模型

### 4.1. 匹配表达式

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

按通常布尔约定，`All([])` 为真，`Any([])` 为假，但降低逻辑应该避免构造空逻辑节
点。`Always` 是 `MATCH` 的 IR 形式。

`DomainTest::Suffix` 同时匹配该 suffix 本身及其子域。Exact 与 suffix 比较是 ASCII
大小写不敏感的。Keyword 匹配同样是 ASCII 大小写不敏感的。

`Predicate` 是兼容性逃逸口。它必须通过调用 `Rule::matches(ctx)` 求值，因此不透明
规则精确保留 `wind_core::rule` 行为，包括 `RULE-SET` 当前恒为 false，以及 `SUB-RULE`
当前沿用旧有包含规则语义。

### 4.2. 集合

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

实现把集合存入 `Ruleset::sets`，并通过表索引引用。名称 `NamedSet` 保留其概念角色；
未来的序列化形式可以分配稳定名称。

成员检查由集合类型决定：

- `Domains` 读取 `ctx.domain`，忽略 `side`；
- `Ips` 根据 `side` 读取 `ctx.dst_ip` 或 `ctx.src_ip`；
- `Ports` 根据 `side` 读取 `ctx.dst_port` 或 `ctx.src_port`。

### 4.3. 语句与 verdict

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

语句是非终结动作。若某个实现暴露语句副作用，它必须在应用规则 verdict 之前，按规
则中的顺序执行这些语句。当前 `RouteAction` API 只观察路由决策，因此内置求值器忽略
语句副作用。退化嵌入从不产生语句。

`Forward` 选择一个命名出站。`Reject` 携带原因字符串并拒绝连接。IR 中存在 `Drop`，
但 `wind_core::RouteAction` 目前没有 drop 变体；内置求值器会把 `Drop` 报告为原因是
`"dropped"` 的拒绝。

### 4.4. Verdict map、链与 ruleset

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

v1 中，verdict map 只以源端口或目标端口范围为键。优化器只会创建范围两两不相交的
map。

`entry` 是 `chains` 的索引；求值总是从这里开始。

## 5. 求值语义

求值从 `Ruleset::entry` 开始，并自上而下扫描入口链。

对每条规则：

1. 用给定 `MatchContext` 求值 `rule.matches`。
2. 如果匹配为假，继续下一条规则。
3. 如果匹配为真，处理 `rule.stmts`，然后应用 `rule.verdict`。

终结 verdict 行为如下：

- `Forward(outbound)` 以 `RouteAction::Forward(outbound)` 终结。
- `Reject(reason)` 以 `RouteAction::Reject(reason)` 终结。
- `Drop` 在当前公开 API 中以拒绝形式终结。

控制流 verdict 行为如下：

- `Return` 对调用方产生 fallthrough。
- `Jump(name)` 求值命名链。若该链产生终结 verdict，则该终结 verdict 获胜。若该链
  fall through，则从 jump 之后的下一条规则继续。
- `Goto(name)` 求值命名链，但不建立语义上的返回点。在当前求值器中，目标链的非终结
  结果仍以调用点 fallthrough 表示。在更严格的尾调用语义实现之前，配置应该在
  `Goto` 目标链中使用显式终结规则。
- `Map(index)` 在 `Ruleset::maps[index]` 中查找当前键。命中时应用条目 verdict。未命
  中但存在 `default` 时应用默认 verdict。未命中且无 `default` 时 fall through 到下
  一条规则。

如果入口链最终 fall through，`Ruleset::route` 应用入口链 policy。非入口链 policy
保留给未来多 base-chain 语义；v1 调用者应该在子链中使用显式终结兜底规则。

实现必须防止无限链递归。当前求值器最大链深度为 64，超过后按 fallthrough 处理。

## 6. 退化嵌入

`Ruleset::from_rules(rules, default_outbound)` 把旧规则嵌入为单链 ruleset：

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

该嵌入是规范性的：优化之前，对同一个 `MatchContext` 的路由必须与旧 first-match-wins
引擎一致。优化之后仍必须与其一致。

以下规则类型会变为强类型 IR 叶子：

| `wind_core::rule::RuleType` | IR 匹配 |
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

所有其它规则类型都会嵌入为 `Predicate(Arc<Rule>)`。

目标字符串映射如下：

- `reject`、`block` 和 `deny` 按大小写不敏感匹配，变为携带规范化原因字符串的
  `Verdict::Reject`；
- 所有其它目标变为 `Verdict::Forward(target)`，并保留目标字符串拼写。

规范化 reject 原因不是路由语义。测试按“是否拒绝”比较决策，而不是比较原因字符串。

## 7. 表层方言降低

### 7.1. Clash/Mihomo

Clash/Mihomo 规则行由 `wind_core::rule::Rule::parse` 解析。多行 helper 会跳过空行和
`#` 注释。

共享规则模型支持以下大类：

- 域名规则：`DOMAIN`、`DOMAIN-SUFFIX`、`DOMAIN-KEYWORD`、`DOMAIN-WILDCARD`、
  `DOMAIN-REGEX`、`GEOSITE`；
- 目标 IP 规则：`IP-CIDR`、`IP-CIDR6`、`IP-SUFFIX`、`IP-ASN`、`GEOIP`；
- 源 IP 规则：`SRC-IP-CIDR`、`SRC-IP-SUFFIX`、`SRC-IP-ASN`、`SRC-GEOIP`；
- 端口：`DST-PORT`、`SRC-PORT`，包括闭区间范围；
- 入站元数据：`IN-PORT`、`IN-TYPE`、`IN-USER`、`IN-NAME`；
- 进程与用户身份：`PROCESS-PATH`、`PROCESS-PATH-REGEX`、`PROCESS-NAME`、
  `PROCESS-NAME-REGEX`、`UID`；
- 协议与流量元数据：`NETWORK`、`DSCP`；
- 复合与兜底：`AND`、`OR`、`NOT`、`SUB-RULE`、`RULE-SET`、`MATCH`。

今天只有第 6 节列出的子集会在 IR 中强类型化。其余规则通过 `Predicate` 保持语义正
确。

### 7.2. tuic-server legacy ACL

tuic-server legacy ACL 是 tuic-server 特有的空格分隔方言（它**不是** Hysteria 的
ACL——后者使用 `outbound(address, proto/port, hijack)` 函数调用形式）。它的解析器与
降低逻辑位于 `tuic-server` crate 的 `legacy` 模块，而非 `wind-acl`；本节记录其降低
过程，因为它的输出会按第 6 节嵌入。行形如：

```text
<outbound> [address] [ports] [hijack]
```

降低过程先把每个 `AclRule` 转换为一个或多个 `wind_core::rule::Rule`
（`tuic_server::legacy::acl_to_rules`），再按第 6 节嵌入这些规则。

地址降低：

| legacy 地址 | 降低后的规则类型 |
| --- | --- |
| 省略或 `*` | `MATCH` |
| IPv4 字面量 | `IP-CIDR` 主机路由 `/32` |
| IPv6 字面量 | `IP-CIDR` 主机路由 `/128` |
| CIDR | `IP-CIDR` |
| domain | `DOMAIN` |
| `*.example.com` | `DOMAIN-SUFFIX,example.com` |
| `suffix:example.com` | `DOMAIN-SUFFIX,example.com` |
| `localhost` | `127.0.0.0/8` 和 `::1/128` |
| `private` | `10.0.0.0/8`、`172.16.0.0/12`、`192.168.0.0/16`、`127.0.0.0/8`、`169.254.0.0/16`、`::1/128`、`fc00::/7`、`fe80::/10` |

端口降低：

- 省略端口列表时，不增加端口条件；
- `80` 变为 `DST-PORT,80`；
- `1000-2000` 变为 `DST-PORT,1000-2000`；
- `tcp/443` 或 `udp/53` 变为 `AND(NETWORK, DST-PORT)`。

当地址条件和端口条件同时存在时，降低会为每个组合发出一条 `AND(address, port)` 规
则。

出站降低：

- `allow` 和 `default` 规范化为出站名 `default`；
- 所有其它出站字符串会保留到 target-to-verdict 映射阶段。

`hijack` 会被解析并保留在 `AclRule` 上，但当前不会执行它。`Statement::Dnat` 是未来
redirect 支持预期使用的 IR 位置。

### 7.3. apernet ACL（真正的 Hysteria 2）

apernet 方言是真正的 Hysteria 2 ACL——一种**函数调用**形式
`outbound(address[, proto/port[, hijack]])`——由 `wind-acl` 的 `syntax::apernet`
解析，对齐 apernet/hysteria 的 `extras/outbounds/acl` 解析器。降低过程把每个
`AclRule` 转换为一个或多个 `wind_core::rule::Rule`（`apernet::acl_to_rules`），再按
第 6 节嵌入。

地址分派是有序且结构化的（小写化并去除尾随 `.` 后，先匹配者胜）：

| apernet 地址 | 降低后的规则类型 |
| --- | --- |
| `all` 或 `*` | `MATCH` |
| IPv4 字面量 | `IP-CIDR` 主机路由 `/32` |
| IPv6 字面量 | `IP-CIDR` 主机路由 `/128` |
| CIDR（v4/v6） | `IP-CIDR` |
| `geoip:<cc>` | `GEOIP,<cc>` |
| `geosite:<name>[@attr…]` | `GEOSITE,<name>`（属性被丢弃——见下） |
| `suffix:<domain>` | `DOMAIN-SUFFIX,<domain>` |
| 含 `*` 的域名（`*.example.com`、`*.google.*`） | `DOMAIN-WILDCARD,<pattern>` |
| 精确域名 | `DOMAIN,<domain>` |

`suffix:` 匹配 apex 及子域；精确域名只匹配自身；含 `*` 的模式是通配（`*` 跨越标签
边界，因此 `*.example.com` 匹配子域但**不**匹配裸 apex）。

端口降低（`<proto>` ∈ {`tcp`、`udp`、`*`}；`<port>` ∈ {`*`、单端口、`lo-hi`}）：

- 省略、`*` 或 `*/*` 不增加端口条件（双协议、所有端口）；
- `tcp` / `tcp/*`（以及 `udp` 形式）变为 `NETWORK,<proto>`（无端口约束）；
- `*/<port>` 变为 `DST-PORT,<port>` / `DST-PORT,lo-hi`（无协议约束）；
- `tcp/<port>`（以及 `udp`）变为 `AND(NETWORK, DST-PORT)`；
- 起始端口为 `0` 是 apernet 的“任意端口”哨兵，不增加端口条件。

当地址条件和端口条件同时存在时，降低会为每个组合发出一条 `AND(address, port)` 规
则（`all`/`*` 地址是匹配全部，因此只发出端口条件）。

出站降低：出站名原样透传。reject 关键字（`reject`/`block`/`deny`，大小写不敏感）经
第 6 节变为 reject 裁决；其它名称（`direct`、`default` 或自定义出站）是转发目标。

有两种 apernet 形式忠实保留但无法在 v1 IR 中完整表达：

- **geosite 属性**（`geosite:google@ads`）在 `GeoSite(String)` 中没有槽位，因此降低
  时被丢弃（保留在解析后的 `AclRule` 上），并发出警告；
- **hijack**（可选的 IP 第三参数）无法用 `RuleType` 表达；它会被解析并保留，但降低
  时被丢弃并发出警告。`Statement::Dnat` 是其预期的未来归宿。

该方言对退化输入比上游更严格（拒绝空地址、纯空白参数、以及含字面量 `)` 的参数），
并在两处无害地不同（仅影响非 DNS 输入）：含 `*` 的模式中 `?` 是单字符通配（上游按字
面量匹配 `?`），且匹配不对主机做 IDNA `ToUnicode`（punycode `xn--` 主机按原样比较），
大小写折叠仅限 ASCII。

## 8. 保序优化

有序链是事实来源。集合和 verdict map 是无序查找结构，因此优化器可以仅在不会改变
first-match-wins 行为时引入它们。

当前优化器只运行在入口链上。其它链原样保留。

### 8.1. Pass 1：连续同 verdict 分桶

优化器从当前位置寻找最长连续片段，要求片段内每条规则的 `(stmts, verdict)` 完全相
同。

这样的片段总是可以替换为一条规则，因为片段内任何成员命中都会产生同一个可观察路
由决策，并且没有无关规则跨过片段边界。

替换规则内部：

- 域名 exact/suffix/keyword 叶子进入一个 `SetData::Domains` 集合；
- 目标 IP 与源 IP 叶子分别进入不同的 `SetData::Ips` 集合；
- 目标端口与源端口叶子分别进入不同的 `SetData::Ports` 集合；
- 不可入集的叶子，包括 `Proto`、`Predicate`、`Always`、复合表达式和已有 `InSet` 节
  点，会作为备选项原样保留。

替换后的 match 是单个备选项，或者 `Match::Any(alts)`。

### 8.2. Pass 2：端口 verdict map

如果 Pass 1 没有消费当前位置，优化器会寻找最长连续片段，要求片段内都是同一 side
上的单个 `Port` 叶子。

该片段可以变为 `VerdictMap`，但仅当：

- 每条规则的语句列表为空；
- 每个键都是闭区间端口范围；
- 所有范围两两不相交。

如果任意两个范围相交，片段必须保持有序。例如：

```text
DST-PORT,1000-2000,proxy
DST-PORT,1500,direct
```

端口 `1500` 仍必须路由到 `proxy`，因为第一条规则获胜。

### 8.3. 不做其它重排

v1 优化器不做非相邻上提、IP verdict map、域名 verdict map 或跨链优化。这些属于未
来扩展；若新增，必须遵守同样的 order-invariance 规则。

## 9. 实现范围与扩展

v1 实现有意区分 IR 容量与引擎行为：

- `RULE-SET` 仍是 `wind_core::rule::RuleType::RuleSet` 谓词，因此当前恒为 false。
- `SUB-RULE` 由 `Predicate` 携带时，仍按旧有 `RuleType::SubRule` 语义求值。
- GeoIP、ASN 和 GeoSite 规则需要 `MatchContext` 中的查找函数。
  `AclEngine::route` 当前不会提供这些函数。
- 源 IP、源端口、入站用户、进程字段和 UID 需要调用者填入 `MatchContext`。
- `Dnat` 存在于 IR 中，但 legacy ACL 的 `hijack` 字段目前不会被发出或执行。
- `Drop` 存在于 IR 中，但公开 `RouteAction` 目前会把它报告为拒绝。
- sing-box 路由规则解析不属于 v1。未来可以为 sing-box 风格的环境匹配器增加强类型
  IR 叶子。

任何未来扩展必须保持退化嵌入与旧规则引擎等价，并必须保持优化语义保序。

## 10. 安全考虑

- **优化器安全性。** 如果某个转换无法证明顺序不可观察，它必须保留规则顺序。这个
  fail-closed 规则可以防止优化静默改变路由或解除拦截。
- **缺失上下文。** 读取缺失 `MatchContext` 字段的规则不会匹配。依赖源地址、进程、
  入站用户、GeoIP、ASN 或 GeoSite 的部署必须确保相应字段或查找函数已填充。
- **Guard 行为。** loopback/private guard 在 IR 求值之前运行。若启用 guard，构建时
  要求提供 resolver，以便在作出 guard 决策前解析域名目标。
- **重定向行为。** legacy ACL 的 `hijack` 字段会被解析但不会执行。未来启用 `Dnat` 会改变流
  量目的地，应该显式开启并在日志中可观察。
- **链循环。** 实现必须限制链递归。当前深度上限是 64。
- **Reject 关键字。** 字符串 `reject`、`block` 和 `deny` 是保留拒绝目标，按大小写
  不敏感匹配。

## 11. 参考资料

- **[RFC2119]** Bradner, S., "Key words for use in RFCs to Indicate
  Requirement Levels", BCP 14, RFC 2119, March 1997.
- **[RFC8174]** Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key
  Words", BCP 14, RFC 8174, May 2017.
- `crates/wind-acl/src/model.rs`、`embed.rs`、`eval.rs`、`optimize.rs`。
- `crates/wind-core/src/rule.rs`。
- MetaCubeX/Mihomo 规则语法。
- apernet/hysteria ACL 语法（`wind-acl` crate，`syntax::apernet` 模块）。
- tuic-server legacy ACL 语法（`tuic-server` crate，`legacy` 模块）。
- nftables 概念：set、map、chain、statement 和 verdict。
