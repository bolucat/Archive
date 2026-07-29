# wind/tuic 项目逐行代码审阅报告

## 摘要

本次审阅在 wind/tuic Rust 工作区的 13 个子系统中共确认 **99 项**问题。整体而言,核心协议层(`proto`)与 Quinn 服务端/客户端实现存在多处**高危正确性与并发缺陷**:QUIC 单向流未调用 `finish()` 导致接收端永远看不到 EOF、UDP 会话缓存驱逐不释放任务(资源泄漏)、ALPN 硬编码 `"h3"` 忽略调用者配置、`Bytes::copy_to_bytes` 不校验长度可被远端 panic、`tuic-server` 旧版字段通过 `#[serde(rename)]` 互换导致 QUIC 窗口参数静默错配等。**安全默认值**亦有重大风险:默认配置关闭 TLS 证书校验、0-RTT 启用但应用层无重放保护、SOCKS5 UDP 中继与 tuic-client 的 UDP 关联均未校验源地址,任何能预测端口的攻击者均可劫持会话。**Naive/SOCKS** 子系统的 UDP 路径存在 IPv4-only 套接字、按包 spawn、未限速无界 channel、SOCKS5 回复头使用错误地址等多重协议/性能问题。规则引擎与调度器中存在多处影响面广的正确性偏差:`rule_target_to_action` 将出站名小写化造成路由错配、UDP 调度统一用 `0.0.0.0:0` 哨兵让所有 UDP ACL 失效、空 AND/OR 形成"恒真/恒假"悖论。建议修复顺序:**(1)** 安全默认值与认证/0-RTT/源校验;**(2)** 协议/编解码 panic 与流终止;**(3)** 会话/任务泄漏与并发控制;**(4)** 路由/规则/配置正确性;**(5)** 性能与代码质量清理。

## 统计

- **总确认问题数**: 99
- **按严重度**: critical 0, high 26, medium 35, low 27, nit 11
- **按类别**: correctness 53, concurrency 10, security 14, performance 9, quality 13
- **按子系统**:
  - proto-core: 6
  - proto-udp-stream: 7
  - quinn-inbound: 6
  - quinn-rest: 13
  - core-rule: 9
  - core-dispatcher-net: 4
  - core-misc: 5
  - socks-base-naive: 18
  - dns-acme-binary: 7
  - tuic-server-config: 4
  - tuic-server-acl: 5
  - tuic-server-rest: 8
  - tuic-client: 11

## Critical 问题

(无)

## High 问题

### QUIC uni 流发送后未 `finish()`,接收方永远看不到 EOF
- 位置: `crates/wind-tuic/src/proto/mod.rs:161-163, 206-208, 252-254, 258-264`
- 类别: correctness / 置信度: high
- 描述:所有单向流帮助函数(`encode_and_send_uni`、`send_auth`、UDP 非 datagram 分支、`drop_udp`)调用了 `write_chunk`/`write_all_chunks` 后从不调用 `send.finish()`。Quinn 在 `finish()` 被调用或 `SendStream` 被 drop 之前不会刷新流结束信号——而未 finish 的 SendStream 被 drop 时会 **reset** 流。接收端通常以 `read_to_end` 读取直至 EOF,因此要么挂起,要么看到流 reset 而非干净结束。这在实际环境下破坏了 auth/dissociate/uni-udp 路径。
- 建议:在返回前调用 `send.finish()?`(必要时 `send.stopped().await?`),让对端看到干净的流结束而非 drop 时的 reset。
- 证据:
```rust
let mut send = conn.open_uni().await?;
send.write_chunk(buf.into()).await?;
Ok(())
```

### UDP 会话缓存驱逐不释放关联任务(资源泄漏)
- 位置: `crates/wind-tuic/src/quinn/inbound.rs:726-793`
- 类别: concurrency / 置信度: high
- 描述:`get_or_create_session` 中 `tuic_stream: Arc<UdpStream>` 同时被 spawn 的两个桥接任务捕获,缓存(容量 `u16::MAX`)驱逐条目时只丢弃自己的 Arc,任务持有的 Arc 仍存活;`from_outbound_rx`/`to_outbound_tx` 循环唯有在 channel 对端关闭时才退出,而 channel 对端正是被任务自身持有的对象——形成持有环,任务永生。客户端循环切换 assoc_id 可让单连接累积无界内存。
- 建议:为每个 session 引入 `CancellationToken`(连接 token 的子 token),`handle_dissociate` 和缓存 `eviction_listener` 中触发 cancel,桥接任务 `tokio::select!` 在 token 上感知退出。
- 证据:
```rust
let response_stream = tuic_stream.clone();
tokio::spawn(async move {
    while let Some(packet) = from_outbound_rx.recv().await {
        if let Err(e) = response_stream.send_packet(packet).await { ... break; }
    }
});
```

### ALPN 硬编码 `"h3"`,忽略 `opts.alpn`
- 位置: `crates/wind-tuic/src/quinn/tls.rs:27`
- 类别: security / 置信度: high
- 描述:客户端 TLS ALPN 被硬编码为 `["h3"]`,无视调用方提供的 `opts.alpn`。TUIC 服务通常协商 `"tuic"`,只播报 `"h3"` 会与正确配置的对端发生 ALPN 不匹配,而静默忽略用户配置也会迫使客户端连上 HTTP/3 端点,弱化安全态势。
- 建议:`opts.alpn.iter().map(|a| a.as_bytes().to_vec()).collect()`,仅在列表为空时回退到合理默认(例如 `"tuic"`)。
- 证据:
```rust
config.alpn_protocols = vec![String::from("h3")].into_iter().map(|alpn| alpn.into_bytes()).collect();
```

### 未校验的 `buf.copy_to_bytes(size as usize)` 可被远端触发 panic
- 位置: `crates/wind-tuic/src/quinn/outbound.rs:180`
- 类别: correctness / 置信度: high
- 描述:`size` 是从网络直接解析的 16 位负载长度,`Bytes::copy_to_bytes` 在剩余字节不足时会 panic。恶意对端只需在 Packet 命令中声明 `size` 大于实际剩余字节,即可使出站任务(进而 spawn 路径 panic)崩溃。
- 建议:在调用 `copy_to_bytes` 前校验 `size as usize <= buf.remaining()`,否则返回解码错误。
- 证据:
```rust
let payload = buf.copy_to_bytes(size as usize);
```

### 入站循环根本未驱动任何 QUIC 连接状态
- 位置: `crates/wind-tuic/src/quiche/inbound.rs:75-108`
- 类别: correctness / 置信度: high
- 描述:接受循环只读取 UDP 包并日志输出;没有 `quiche::accept`、`conn.recv`、`conn.send`、没有连接表、无定时器处理。`listen` 静默返回但根本不提供服务,`self.users` 也未消费。客户端会无诊断地超时。
- 建议:要么实现完整状态机,要么从 `listen` 返回明确错误并将其特性门置为 `unimplemented` 而非 `server`。
- 证据:
```rust
warn!("Received QUIC packet from {}, DCID len: {}", src, hdr.dcid.len());
```

### `wildcard_match` 每次调用都重新编译 Regex
- 位置: `crates/wind-core/src/rule.rs:833-852`
- 类别: performance / 置信度: high
- 描述:对 `DomainWildcard` 规则的每次匹配都从头编译正则,并重新小写输入。正则编译开销大且发生在每连接的路由热路径上。应在解析时预编译并存进 `RuleType` 变体。
- 建议:在 `parse_type` 处理 DOMAIN-WILDCARD 分支时即将通配符转换为 `Regex`,在 `DomainWildcard` 中存储 `Regex`。
- 证据:
```rust
let mut re = String::from("^");
for ch in pattern.chars() {
    match ch { '*' => re.push_str(".*"), ... }
}
Regex::new(&re).is_ok_and(|r| r.is_match(&text))
```

### `rule_target_to_action` 将出站名小写化导致路由错配
- 位置: `crates/wind-core/src/dispatcher.rs:294-298`
- 类别: correctness / 置信度: high
- 描述:在匹配 reject/block/deny 后,fallback 分支以 `name.to_string()` 转发,而 `name` 已被绑定为小写串。`Dispatcher::resolve_handler` 的 HashMap 查找是区分大小写的,因此 `Proxy_Out` 被静默小写后落入 default,无错误。
- 建议:只为 reject 关键字匹配做小写,转发时使用原始 `target` 字符串。
- 证据:
```rust
match target.to_ascii_lowercase().as_str() {
    "reject" | "block" | "deny" => RouteAction::Reject(format!("rejected by rule: {}", rule)),
    name => RouteAction::Forward(name.to_string()),
}
```

### UDP 调度用 `0.0.0.0:0` 哨兵路由,UDP ACL 全部失效
- 位置: `crates/wind-core/src/dispatcher.rs:187-206`
- 类别: correctness / 置信度: high
- 描述:`dispatch_udp` 构造 `TargetAddr::IPv4(0.0.0.0, 0)` 哨兵传入 `router.route(...)`,导致路由器看不到任何真实目的,所有 UDP 会话被同样路由,IP-CIDR/DOMAIN-SUFFIX/DST-PORT 等规则对 UDP 完全失效。
- 建议:将 `Router::route` 改为对 UDP 接受 `Option<&TargetAddr>` 并在看到首个包后路由,或新增 `route_udp` API。
- 证据:
```rust
let sentinel = TargetAddr::IPv4(std::net::Ipv4Addr::UNSPECIFIED, 0);
let action = self.router.route(&sentinel, false).await?;
```

### `copy_io` 在任一方向 half-close 时同时关闭两个方向
- 位置: `crates/wind-core/src/io.rs:18-55`
- 类别: correctness / 置信度: high
- 描述:`select!` 循环遇到 EOF(num == 0)立即 break 返回。TCP/HTTP/QUIC 隧道经常先半关闭一方向(例如 HTTP 客户端在请求后发 FIN),代理需要继续转发另一方向直至其也 EOF。当前实现丢数据并提前终止连接。
- 建议:改用 `tokio::io::copy_bidirectional`,或在 EOF 时对反向 writer 调用 `shutdown()` 并继续转发剩余方向。
- 证据:
```rust
Ok(num) => {
   // EOF
   if num == 0 {
      break;
   }
```

### SOCKS UDPAssociate 回复 IP 硬编码 `127.0.0.1`,忽略 `public_addr`
- 位置: `crates/wind-socks/src/inbound.rs:117`
- 类别: correctness / 置信度: high
- 描述:`SocksInboundOpt::public_addr` 文档明确为"用于 UDP 回复",但 UDPAssociate 回复 IP 被硬编码为 `127.0.0.1`,远端客户端会向自己的环回发送 UDP,导致中继永远收不到。
- 建议:`allow_udp` 启用时使用 `opts.public_addr.ok_or(...)?`(或回退到 listener 本地地址);若未设置则在配置层拒绝。
- 证据:
```rust
let reply_ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
crate::ext::run_udp_proxy(proto, &target_addr, None, reply_ip, move |inbound| async move {
```

### SOCKS5 UDP 回复头使用客户端地址而非远端源地址
- 位置: `crates/wind-socks/src/udp.rs:142-147`
- 类别: correctness / 置信度: high
- 描述:RFC 1928 §7 规定 UDP 回复中的 ATYP/DST.ADDR/DST.PORT 必须标识远端来源主机;此处 `new_udp_header(current_client)` 将客户端自身地址写入回复源,客户端永远只看到自己的地址,无法对多个远端进行多路复用解复用。
- 建议:回复头来自 `packet.source`(或 `packet.target`),`current_client` 仅作为 `send_to` 目的。
- 证据:
```rust
if let Ok(mut packet_with_header) = new_udp_header(current_client) {
    packet_with_header.extend_from_slice(&packet.payload);
    if let Err(e) = socket.send_to(&packet_with_header, current_client).await {
```

### SOCKS UDP 中继接受任意源,允许劫持
- 位置: `crates/wind-socks/src/udp.rs:97-108`
- 类别: security / 置信度: high
- 描述:`source_addr` 初始化为未指定地址,被最近一个发包者覆盖。任何能猜到中继 `ip:port` 的链外攻击者只需发一个 UDP 包即可劫持关联,合法上游回复将被转发给攻击者。RFC 1928 §6 明确要求丢弃源不匹配的报文。
- 建议:从 TCP 控制连接固定预期客户端地址(或首个观察到的对端),其他源一律丢弃。
- 证据:
```rust
let source_addr = Arc::new(ArcSwap::new(Arc::new(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0))));
...
Ok((len, addr)) => {
    source_addr_rx.store(Arc::new(addr));
```

### UDP 中继 socket 仅 IPv4,发往 IPv6 目标必失败
- 位置: `crates/wind-base/src/direct.rs:89`
- 类别: correctness / 置信度: high
- 描述:`UdpSocket::bind("0.0.0.0:0")` 仅创建 IPv4 socket。当 `resolve_target` 返回 IPv6 时 `send_to` 失败 `EAFNOSUPPORT`,所有 IPv6 UDP 流量静默损坏。
- 建议:绑定双栈 `[::]:0`(并禁用 `IPV6_V6ONLY`)或按目标族选择套接字。
- 证据:
```rust
let relay_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
```

### Naive UDP 路径按包 `tokio::spawn` 并按包建立 CONNECT 隧道
- 位置: `crates/wind-naive/src/lib.rs:199-223`
- 类别: concurrency / 置信度: high
- 描述:每收到一个 UDP 包就 spawn 一个分离任务并新开一条 CONNECT 隧道写入再丢弃。(1) spawn 无并发上限,UDP 洪水即 DoS;(2) 每包付完整 TLS+HTTP 握手代价,等同不可用;(3) `tx`(响应)被命名为 `_tx` 并丢弃,UDP 回复永不送回——本质单向。
- 建议:按 Naive 协议实现 UDP-over-TLS 多路复用(每远端或单流多路复用),限制并发 spawn,且将 `tx` 接通至应答传递。或暂时移除该实现并返回错误。
- 证据:
```rust
let UdpStream { tx: _tx, mut rx } = udp_stream;
...
tokio::spawn(async move {
    if let Err(e) = udp_tunnel_tx(client, &target_str, &payload).await {
```

### 默认配置关闭 TLS 证书校验
- 位置: `crates/wind/src/conf/persistent.rs:65, 182`
- 类别: security / 置信度: high
- 描述:默认 `PersistentConfig` 与 `skip_cert_verify` 的 serde 默认值均为 true。运行 `wind init` 或依赖默认值的用户得到的 TUIC 出站接受任意证书,完全失去 TLS 意义,允许对上游中继的轻易 MITM。
- 建议:将默认改为 false,关闭校验须显式 opt-in(且应在日志中大声警告)。
- 证据:
```rust
skip_cert_verify: true, ...
#[serde(default = "default_true")]
pub skip_cert_verify: bool,
```

### 旧版 `send_window`/`receive_window` 字段被 `#[serde(rename)]` 互换
- 位置: `crates/tuic-server/src/config.rs:182-187`
- 类别: correctness / 置信度: high
- 描述:`__send_window` 被声明为 `Option<u64>` 但 `rename = "receive_window"`,反之亦然。用户提供的旧顶层 `send_window` 实际反序列化到 `__receive_window`,`migrate()` 再写入 `quic.send_window`——结果旧 `send_window` 变成新 `quic.receive_window`,QUIC 流控参数被静默互换。
- 建议:交换 `rename` 字符串(或字段名),使 `__send_window` rename 为 `"send_window"`。
- 证据:
```rust
#[serde(default, rename = "receive_window")]
#[deprecated]
pub __send_window: Option<u64>,
#[serde(default, rename = "send_window")]
#[deprecated]
pub __receive_window: Option<u32>,
```

### 格式推断由于运算符优先级把任何含 `=` 的行视为 TOML
- 位置: `crates/tuic-server/src/config.rs:610-613`
- 类别: correctness / 置信度: high
- 描述:`A && B && C || D` 解析为 `(A && B && C) || D`,任何非空、非注释、含 `=` 的行(如 YAML 值 `secret: aGVsbG8=`)都被识别为 TOML;tie-break 又偏 TOML——YAML 文件被当作 TOML 解析失败。
- 建议:显式加括号,且把 `=` 检测限制到 `^[A-Za-z_][\w-]*\s*=` 这样的键值形态。
- 证据:
```rust
let has_toml_patterns = lines.iter().any(|line| {
    let trimmed_line = line.trim();
    trimmed_line.starts_with('[') && trimmed_line.contains(']') && !trimmed_line.contains(':') || trimmed_line.contains('=')
});
```

### ipv4 文法接受 `999.999.999.999`,无八位组/前缀范围校验
- 位置: `crates/tuic-server/src/acl.pest:34-39, 51-56`
- 类别: correctness / 置信度: high
- 描述:`ASCII_DIGIT{1,3}` 接受到 999,CIDR 前缀也无范围限制。`parse_address_from_pair` 仅存原文,运行时要么不匹配,要么落到 `address_to_rule_types` 的 `0.0.0.0/32` 兜底,掩盖了用户的配置错误。
- 建议:在文法层收紧到 0-255 与有效前缀,或在 `parse_address_from_pair` 中以 `Ipv4Addr`/`IpNetwork` 校验。
- 证据:
```rust
ipv4 = @{
    ASCII_DIGIT{1,3} ~ "." ~
    ASCII_DIGIT{1,3} ~ "." ~
    ASCII_DIGIT{1,3} ~ "." ~
    ASCII_DIGIT{1,3}
}
```

### 证书 watcher 在瞬时 I/O 错误后永久退出
- 位置: `crates/tuic-server/src/tls.rs:46-58`
- 类别: correctness / 置信度: high
- 描述:`start_watch` 返回 `Result<()>` 并由 `tokio::spawn` await;`calc_hash` 任意瞬时错误(ACME 续期时 rename 中途的 ENOENT、权限抖动)直接传播退出循环,此后热重载永久失效。
- 建议:在循环内 match `calc_hash` 结果,出错只记日志并 `continue`,不要从 watcher 任务中传播错误。
- 证据:
```rust
let hash = Self::calc_hash(&self.cert_path, &self.key_path).await?;
if &hash != self.hash.swap(hash.into()).deref() {
    match self.reload_cert_key().await {
```

### 哈希在 reload 成功前已 commit,失败后不再重试
- 位置: `crates/tuic-server/src/tls.rs:50-56`
- 类别: correctness / 置信度: high
- 描述:`self.hash.swap(...)` 无条件存入新哈希后才执行 `reload_cert_key`;若 reload 失败(写入中途、不支持的 key),新哈希已写入,下次轮询认为"未变化"不再重试,旧(可能过期)证书被一直使用且无后续警告。
- 建议:先 `load()` 对比,仅在 `reload_cert_key` 成功后 `swap` 新哈希。
- 证据:
```rust
if &hash != self.hash.swap(hash.into()).deref() {
    match self.reload_cert_key().await {
        Ok(_) => warn!("Successfully reloaded TLS certificate and key"),
        Err(e) => warn!("Failed to reload TLS certificate and key: {e}"),
```

### Forwarder/SOCKS 启动错误被吞没,`run()` 不 await forwarder
- 位置: `crates/tuic-client/src/lib.rs:26-37`
- 类别: correctness / 置信度: high
- 描述:`forward::start()` 返回 `()` 并只 spawn 监听任务,bind 失败仅在被吞任务中 `warn!`;之后 `socks5::Server::start()` 永久循环。主线程无法感知启动错误,错误配置的 forwarder 静默消失。
- 建议:`start()` 返回 `Result`,在主线程使用 `JoinSet` 监控所有 forwarder。
- 证据:
```rust
forward::start(cfg.local.tcp_forward.clone(), cfg.local.udp_forward.clone()).await;
match socks5::Server::set_config(cfg.local) {
```

### UDP forwarder 每个入站包打开新出站流
- 位置: `crates/tuic-client/src/forward.rs:147-235`
- 类别: correctness / 置信度: high
- 描述:每收到一个 UDP 包就 spawn 一个新任务建立一次性 `UdpStream`,发送后立即 drop,无法维持 5 元组状态,NAT 语义被破坏;每包还伴随 spawn 和 mpsc 创建。
- 建议:每 `src_addr` 维持一条出站 `UdpStream`,在 recv 循环中喂入;reply-bridge 每会话仅 spawn 一次。
- 证据:
```rust
tokio::spawn(async move {
    let (tx_to_out, rx_from_local) = tokio::sync::mpsc::channel::<UdpPacket>(8);
    let (tx_to_local, mut rx_from_out) = tokio::sync::mpsc::channel::<UdpPacket>(8);
    let _ = tx_to_out.send(UdpPacket { source: None, target: target.clone(), payload: pkt }).await;
    drop(tx_to_out);
```

### 首发包竞争把 UDP associate socket 绑定到攻击者
- 位置: `crates/tuic-client/src/socks5/udp_session.rs:139-141`
- 类别: security / 置信度: high
- 描述:首个 datagram 触发 `self.socket.connect(src_addr).await` 永久绑定对端。任何能在合法客户端之前发送 UDP 至中继端口的攻击者即可独占中继,无需认证。
- 建议:在 connect 前用 ACL 控制连接的 `ctrl_addr.ip()` 校验 `src_addr.ip()`。
- 证据:
```rust
if let Ok(connected_addr) = self.socket.peer_addr() {
    ...
} else {
    self.socket.connect(src_addr).await?;
}
```

## Medium 问题

### `send_datagram` 路径拷贝整个 UDP 负载
- 位置: `crates/wind-tuic/src/proto/mod.rs:248-251`
- 类别: performance / 置信度: high
- 描述:`Chain` 后立刻 `copy_to_bytes(combined.remaining())` 又分配又 memcpy,正好抵消了 Chain 的初衷,UDP 热路径上每包多一次拷贝。
- 建议:`BytesMut::with_capacity(12 + payload.len())` 预分配再 `extend_from_slice`,然后 `self.send_datagram(buf.freeze())?`。
- 证据:
```rust
if datagram {
    let mut combined = buf.freeze().chain(payload);
    self.send_datagram(combined.copy_to_bytes(combined.remaining()))?;
}
```

### `send_packet` 单包路径 `next_pkt_id` 读取与递增非原子
- 位置: `crates/wind-tuic/src/proto/udp_stream.rs:291-293`
- 类别: correctness / 置信度: high
- 描述:`load` 后再 `fetch_add` 不原子,两个并发 `send_packet` 可能用同一 pkt_id 发出两条 datagram,与接收端的分片重组状态发生冲突。
- 建议:替换为 `let pkt_id = self.next_pkt_id.fetch_add(1, Ordering::Relaxed);`,并使用返回值。
- 证据:
```rust
self.connection.send_udp(self.assoc_id, self.next_pkt_id.load(Ordering::Relaxed), ...).await?;
self.next_pkt_id.fetch_add(1, Ordering::Relaxed);
```

### 攻击者可控的 `frag_total`/`frag_id` 未校验,可触发混淆状态 DoS
- 位置: `crates/wind-tuic/src/proto/udp_stream.rs:92-131`
- 类别: security / 置信度: medium
- 描述:`add_fragment` 直接信任 wire 上的 `frag_total`/`frag_id`,既不校验 `frag_total > 0`,也不校验 `frag_id < frag_total`,后续片段也不校验 `frag_total` 一致;可触发空包立即"重组"、永久污染子缓存或者强占巨大缓存。
- 建议:拒绝 `frag_total == 0`、`frag_id >= frag_total`、`frag_total` 与首片不一致的片。
- 证据:
```rust
fragments: Cache::new(frag_total.into()),
...
meta.value().fragments.insert(frag_id, payload).await;
```

### `saturating_sub` 导致除零/无限分片数
- 位置: `crates/wind-tuic/src/proto/udp_stream.rs:266-281`
- 类别: correctness / 置信度: medium
- 描述:小 `max_datagram_size` 时 `subsequent_frag_max_payload` 可为 0,`div_ceil(0)` panic;敌意域名也能让首片头开销膨胀触发同问题。
- 建议:若任一最大负载为 0,提前返回错误。
- 证据:
```rust
let first_frag_max_payload = max_datagram_size.saturating_sub(first_frag_header_overhead);
let subsequent_frag_max_payload = max_datagram_size.saturating_sub(subsequent_frag_header_overhead);
1 + remaining.div_ceil(subsequent_frag_max_payload)
```

### `info!` 级日志在每个 datagram 发送时打印
- 位置: `crates/wind-tuic/src/proto/udp_stream.rs:269-270, 348-352`
- 类别: performance / 置信度: high
- 描述:每分片每发送都 `info!` 分配格式化字符串并 I/O,远比包本身昂贵。
- 建议:降级为 `debug!`/`trace!`,`info!` 仅用于状态变化。
- 证据:
```rust
wind_core::info!(target: "[UDP]", "Fragmentation params: payload={}, ...");
wind_core::info!(target: "[UDP]", "Sending fragment {}/{}: {} bytes", frag_id + 1, frag_total, datagram_size);
```

### `cleanup_expired` 的 Err 被无声警告
- 位置: `crates/wind-tuic/src/proto/udp_stream.rs:147-153`
- 类别: quality / 置信度: high
- 描述:`invalidate_entries_if` 失败实为静态构造期错误;warn 并继续会掩盖编程缺陷。
- 建议:构造时显式开启 `support_invalidation_closures` 并在此 `expect`。
- 证据:
```rust
if let Err(e) = self.fragments.invalidate_entries_if(move |_, meta| { ... }) {
    wind_core::warn!(target: "[UDP]", "Failed to register fragment cleanup predicate: {:?}", e);
    return;
}
```

### 每连接 UDP 会话缓存容量 `u16::MAX` 易 DoS
- 位置: `crates/wind-tuic/src/quinn/inbound.rs:334`
- 类别: security / 置信度: medium
- 描述:容量 65535 等于整个 16 位 assoc_id 空间,每新 id spawn 三个任务并分配多个 channel,认证后的对端可在单连接上耗光资源。
- 建议:容量收紧到可配置较低值(256-1024),LRU/TTI 策略并在驱逐时取消任务。
- 证据:
```rust
udp_sessions: Cache::new(u16::MAX.into()),
```

### `tokio::select!` 在 `endpoint.accept()` 返回 `None` 时 panic
- 位置: `crates/wind-tuic/src/quinn/inbound.rs:242-264`
- 类别: correctness / 置信度: medium
- 描述:可拒绝模式且无 `else =>` 分支,Endpoint 正常关闭时 `select!` 全部分支被禁用,panic。
- 建议:增加 `else => break,`,或用 `match ... { Some(i) => ..., None => break }`。
- 证据:
```rust
tokio::select! {
    _ = self.cancel.cancelled() => { ... break; }
    Some(incoming) = endpoint.accept() => { ... }
}
```

### 启用 0-RTT 但无应用层重放保护
- 位置: `crates/wind-tuic/src/quinn/inbound.rs:199-202`
- 类别: security / 置信度: medium
- 描述:`max_early_data_size = u32::MAX` 且 `send_half_rtt_data = true`,服务端接受所有 0-RTT 应用数据,无 nonce/计数器/幂等;攻击者可重放 Connect/Packet。
- 建议:0-RTT 仅允许 Heartbeat,或为 0-RTT 处理引入重放缓存/nonce 校验。
- 证据:
```rust
if self.opts.zero_rtt {
    crypto.max_early_data_size = u32::MAX;
    crypto.send_half_rtt_data = true;
}
```

### UDP assoc id 分配器静默回绕,与活会话冲突
- 位置: `crates/wind-tuic/src/quinn/outbound.rs:255`
- 类别: correctness / 置信度: medium
- 描述:`u16` 计数器 fetch_add Relaxed 回绕,重用仍存活的 id 会让 `insert` 静默覆盖原 Arc。
- 建议:探测未用 id 或显式上限,满时拒绝新会话。
- 证据:
```rust
let assoc_id = self.udp_assoc_counter.fetch_add(1, Ordering::SeqCst);
self.udp_session.insert(assoc_id, tuic_stream.clone()).await;
```

### `spawn_handler` 在单个慢消费者上终止 accept 循环
- 位置: `crates/wind-tuic/src/quinn/task.rs:51-54`
- 类别: concurrency / 置信度: high
- 描述:`send_timeout(_, 1s)` 超时即 `break`,慢消费者杀死整个连接的接收。
- 建议:区分接收方 drop 与超时;超时应应用反压(丢最旧、记录并继续)而非退出循环。
- 证据:
```rust
if let Err(e) = tx.send_timeout(item, Duration::from_secs(1)).await {
    warn!("{} channel send failed (receiver dropped or timeout): {e:?}", name);
    break;
}
```

### `TuicheOutbound` 没有任何客户端实现
- 位置: `crates/wind-tuic/src/quiche/outbound.rs:10-123`
- 类别: correctness / 置信度: high
- 描述:仅存配置和 builder,无 connect、无 `AbstractOutbound` 实现、无网络。`client` 特性下发布会产生无用 handle。
- 建议:实现连接逻辑,或先移除类型与 `client` feature。
- 证据:
```rust
#[allow(dead_code)]
pub struct TuicheOutbound { ... password: Vec<u8>, opts: ConnectionOpts }
```

### Builder 收集 `max_idle_time`/`connect_timeout`/`verify_certificate` 但不传播
- 位置: `crates/wind-tuic/src/quiche/outbound.rs:32-114`
- 类别: quality / 置信度: high
- 描述:fluent setter 暴露这些字段但 `build()` 不带入,静默失效。
- 建议:删除 setter,或注入到 `TuicheOutbound`/`ConnectionOpts`。
- 证据:
```rust
Ok(TuicheOutbound { server_addr, server_name, uuid, password: password.into_bytes(), opts: self.opts })
```

### `start_poll` 必须显式调用且非幂等
- 位置: `crates/wind-tuic/src/quinn/outbound.rs:100-112`
- 类别: correctness / 置信度: high
- 描述:`new` 不调用 `start_poll`,忘记调用则心跳与入站不工作;重复调用会 spawn 重复 handler。
- 建议:在 `new` 内自动调用,或用 `AtomicBool` 守卫幂等性。
- 证据:
```rust
pub async fn start_poll(&self) -> eyre::Result<()> { ... }
```

### `DomainSuffix` 每次匹配分配 2 个 String 并多余拼接
- 位置: `crates/wind-core/src/rule.rs:319-321`
- 类别: correctness / 置信度: high
- 描述:每次连接评估都分配并 `format!`,可预先小写并以字节切片比较。
- 建议:解析时预小写,字节级不分配比较。
- 证据:
```rust
h.eq_ignore_ascii_case(suffix) || h.to_ascii_lowercase().ends_with(&format!(".{}", suffix.to_ascii_lowercase()))
```

### `DomainKeyword` 每次匹配分配两个 String
- 位置: `crates/wind-core/src/rule.rs:323-325`
- 类别: performance / 置信度: high
- 描述:同上,关键词应预先小写,使用无分配的 ASCII 不敏感子串搜索。
- 建议:存储小写 keyword,手动 ASCII 不敏感子串匹配。
- 证据:
```rust
RuleType::DomainKeyword(kw) => ctx.domain
    .is_some_and(|h| h.to_ascii_lowercase().contains(&kw.to_ascii_lowercase())),
```

### 空 AND/OR 接受导致悖论语义
- 位置: `crates/wind-core/src/rule.rs:608-624`
- 类别: correctness / 置信度: high
- 描述:空 AND 永远为真(等同于 MATCH),空 OR 永远为假;`AND,(),DIRECT` 类配置直接成为 catch-all,危险。
- 建议:解析时拒绝空 AND/OR。
- 证据:
```rust
"AND" => { let sub = Self::parse_compound(value)?; Ok(RuleType::And(sub)) }
"OR"  => { let sub = Self::parse_compound(value)?; Ok(RuleType::Or(sub)) }
```

### IP-SUFFIX 与 IP-CIDR 语义重复
- 位置: `crates/wind-core/src/rule.rs:337-345`
- 类别: correctness / 置信度: medium
- 描述:IpSuffix 也用 `net.contains(&ip)`,与 IpCidr 完全等价。要么实现真正的后缀位语义,要么删掉。
- 建议:实现后缀位语义或移除变体。
- 证据:
```rust
RuleType::IpSuffix(net) => ctx.dst_ip.is_some_and(|ip| net.contains(&ip)),
```

### `Option<impl AbstractOutbound + Sized + Send>` 迫使 `None` 调用方杜撰类型
- 位置: `crates/wind-core/src/outbound.rs:9, 15`
- 类别: quality / 置信度: high
- 描述:正因如此 dispatcher.rs 才有带 `unreachable!()` 的 `NoChain`。
- 建议:拆分两个方法或用 `Option<&dyn AbstractOutbound>`/`Box<dyn AbstractOutbound>`。
- 证据:
```rust
fn handle_tcp(
    &self,
    target_addr: TargetAddr,
    stream: impl AbstractTcpStream,
    via: Option<impl AbstractOutbound + Sized + Send>,
) -> impl Future<Output = eyre::Result<()>> + Send;
```

### `interface::StackPrefer` 重复定义
- 位置: `crates/wind-core/src/interface.rs:44-89`
- 类别: quality / 置信度: high
- 描述:`utils::StackPrefer` 已存在且更完善,`interface::StackPrefer` 通过 `pub use` 被覆盖,但仍残留并带不同 serde 别名,易混淆。
- 建议:删除 `interface::StackPrefer`,统一使用 `utils::StackPrefer`。
- 证据:
```rust
#[allow(dead_code)]
pub enum StackPrefer { V4, V6, V4V6, V6V4 }
```

### `TargetAddr` 反序列化接受空 host
- 位置: `crates/wind-core/src/types.rs:82-100`
- 类别: correctness / 置信度: high
- 描述:`":80"` 被解析为 `Domain("", 80)`、`"x x:80"` 被解析为 `Domain("x x", 80)`,后续 DNS 失败时报错不明显。
- 建议:用 `rsplit_once(':')`,校验 host 非空且至少做基本域名语法检查。
- 证据:
```rust
let parts: Vec<&str> = s.split(':').collect();
if parts.len() != 2 { return Err(Error::custom("Invalid address format, expected host:port")); }
Ok(TargetAddr::Domain(parts[0].to_string(), port))
```

### `warn!`/`error!` 默认分支与 `info!` 不一致地省略 target
- 位置: `crates/wind-core/src/log.rs:37-39`
- 类别: quality / 置信度: high
- 描述:`info!` 默认带 `target = crate-name`,而 `warn!`/`error!` 无,过滤器配置失效。
- 建议:对 `warn!`/`error!` 也补上 `target: $crate::extract_crate_name!()`。
- 证据:
```rust
($($arg:tt)*) => {
   $crate::log::tracing::warn!($($arg)*)
};
```

### 直连 UDP 任务在 select! 退出后未 abort
- 位置: `crates/wind-base/src/direct.rs:93-132`
- 类别: concurrency / 置信度: high
- 描述:`send_task`/`recv_task` 是分离 `tokio::spawn`,select! 完成后另一任务仍持有 socket 与 channel 继续运行。
- 建议:用 `JoinSet`/`abort_on_drop`,或直接 `select!` 两个 future 不 spawn。
- 证据:
```rust
let send_task = tokio::spawn(async move { ... });
let recv_task = tokio::spawn(async move { ... });
tokio::select! { _ = send_task => {} _ = recv_task => {} }
```

### Naive 桥接 I/O 线程"写依赖读"导致写延迟挂起
- 位置: `crates/wind-naive/src/lib.rs:284-317`
- 类别: concurrency / 置信度: medium
- 描述:单线程先 drain 写、再阻塞读;上游静默时写无法发送。
- 建议:读写分线程,或将 socket 设非阻塞配合 eventfd/Waker。
- 证据:
```rust
if let Ok(data) = naive_write_rx.try_recv() {
    if naive.write_all(&data).is_err() { return; }
    let _ = naive.flush();
}
match naive.read(&mut read_buf) {
```

### Naive 使用无界 mpsc(背压/OOM)
- 位置: `crates/wind-naive/src/lib.rs:275-276`
- 类别: concurrency / 置信度: medium
- 描述:`unbounded_channel` 无背压,慢消费者直接 OOM。
- 建议:有界 `mpsc::channel(64)`。
- 证据:
```rust
let (naive_write_tx, mut naive_write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
let (naive_read_tx, mut naive_read_rx) = mpsc::unbounded_channel::<Vec<u8>>();
```

### IPv6 字面量 server_address 派生 SNI 出错
- 位置: `crates/wind-naive/src/lib.rs:125-128`
- 类别: correctness / 置信度: high
- 描述:`[2001:db8::1]:443` 被 split `':'` 取首段得到 `"[2001"`,Cronet 收到无意义 SNI。
- 建议:用合适解析器或在 IPv6 字面量时强制要求 `server_name`。
- 证据:
```rust
.unwrap_or_else(|| opts.server_address.split(':').next().unwrap_or("").to_string());
```

### 配置路径扩展名检测对非 yaml/toml 静默归为 TOML
- 位置: `crates/wind/src/conf/persistent.rs:291-297`
- 类别: correctness / 置信度: medium
- 描述:`foo.json` 被当成 TOML 解析;CLI 中宣称的 `BASE64-TEXT` 输入也未被处理。
- 建议:显式 `.toml` vs `.yaml/.yml`,未知扩展返回错误;若要支持 base64 则先解码再 merge。
- 证据:
```rust
figment = if path.ends_with(".yaml") || path.ends_with(".yml") {
    figment.merge(Yaml::file(path))
} else {
    figment.merge(Toml::file(path))
};
```

### `attempts` 缺少 `#[serde(default)]`,反序列化失败
- 位置: `crates/wind-dns/src/config.rs:61-62`
- 类别: correctness / 置信度: high
- 描述:`deny_unknown_fields` 加上没有默认值的可选字段在某些格式下会拒绝省略字段的配置。
- 建议:添加 `#[serde(default)]`。
- 证据:
```rust
/// Retry attempts per query. Defaults to the Hickory library default.
pub attempts: Option<usize>,
```

### Ctrl-C 未触发优雅停机
- 位置: `crates/tuic-server/src/main.rs:37-53`
- 类别: correctness / 置信度: high
- 描述:`AppContext::cancel` 仅被 spawn 任务作为父 token 取子 token,主线程 ctrl_c 后未 `cancel.cancel()`,任务依赖 runtime 退出强制中止,且未 flush 日志守卫。
- 建议:从 `run` 暴露/接收 cancel token,ctrl_c 后取消并 await server 任务。
- 证据:
```rust
res = tokio::signal::ctrl_c() => {
    if let Err(err) = res { ... } else {
        tracing::info!("Received Ctrl-C, shutting down.");
    }
}
```

### `max_concurrent_bi/uni_streams` 硬编码 512
- 位置: `crates/tuic-server/src/wind_adapter.rs:213-215`
- 类别: quality / 置信度: high
- 描述:DoS 相关旋钮被硬编码,只能改源码。
- 建议:接入 `Config`。
- 证据:
```rust
max_concurrent_bi_streams: 512,
max_concurrent_uni_streams: 512,
```

### `AclAddress::Ip` 是 IPv6 时用 `/32` 构造导致网络扩大
- 位置: `crates/tuic-server/src/acl.rs:627-636`
- 类别: correctness / 置信度: high
- 描述:`2001:db8::1` 加 `/32` 解析成功并匹配整个 `2001:db8::/32`,ACL 被静默扩大。
- 建议:先解析为 `IpAddr` 再按 v4/v6 选 32/128。
- 证据:
```rust
if let Ok(net) = format!("{ip_str}/32").parse::<ipnet::IpNet>() {
    vec![wrule::RuleType::IpCidr(net)]
} else if let Ok(net) = format!("{ip_str}/128").parse::<ipnet::IpNet>() {
```

### ACL 无效 IP 兜底为 `0.0.0.0/32`
- 位置: `crates/tuic-server/src/acl.rs:627-636`
- 类别: security / 置信度: high
- 描述:无效 IP 进入 `0.0.0.0/32`,可能导致路由错配或 fail-open。
- 建议:返回 `Result` 并在 ACL 加载失败时拒绝启动。
- 证据:
```rust
vec![wrule::RuleType::IpCidr(
    ip_str.parse().unwrap_or_else(|_| "0.0.0.0/32".parse().unwrap()),
)]
```

### ACL ipv6 文法过度宽松,屏蔽其他 token
- 位置: `crates/tuic-server/src/acl.pest:41-44`
- 类别: correctness / 置信度: high
- 描述:`(ASCII_HEX_DIGIT | ":")+` 匹配 `10`/`cafe`/`::::`,排在 ipv4 前导致 `proxy 10` 被识别为 IPv6 并保存为字符串,运行时不匹配。
- 建议:真实 IPv6 语法或解析时用 `Ipv6Addr::from_str` 校验,并把更具体的规则提前。
- 证据:
```rust
ipv6 = @{ (ASCII_HEX_DIGIT | ":")+ ~ &(WHITESPACE | EOI) }
```

### ACL `address` 选择顺序导致歧义
- 位置: `crates/tuic-server/src/acl.pest:9, 15-25`
- 类别: correctness / 置信度: medium
- 描述:`wildcard_domain | cidr | ipv6 | ipv4 | domain | any_addr` 顺序与过宽的 domain/ipv6 共同导致 `1.2.3` 等被当成 domain 存储,无声不匹配。
- 建议:收紧 ipv6,提前 cidr,解析后做合法性校验。
- 证据:
```rust
address = { localhost_kw | private_kw | suffix_localhost | wildcard_domain | cidr | ipv6 | ipv4 | domain | any_addr }
```

### `wind_adapter` 中 DNS 解析忽略 `ipstack_prefer`
- 位置: `crates/tuic-client/src/wind_adapter.rs:29-34`
- 类别: security / 置信度: high
- 描述:直接 `lookup_host(...).next()`,忽略 V4first/V6first/V4only/V6only;V4only/V6only 可能选到禁用族。
- 建议:使用 `utils::ServerAddr::resolve()` 或复制其过滤逻辑。
- 证据:
```rust
let addrs = tokio::net::lookup_host(format!("{}:{}", relay.server.0, relay.server.1)).await?;
addrs.into_iter().next().ok_or_else(|| eyre::eyre!("Failed to resolve server address"))?
```

### `socks5::Server::set_config` 二次调用 panic
- 位置: `crates/tuic-client/src/socks5/mod.rs:37-52`
- 类别: concurrency / 置信度: medium
- 描述:返回 `Result` 的函数却 `.unwrap()`,二次设置 OnceCell 直接 panic。
- 建议:映射为 `Error` 变体返回。
- 证据:
```rust
SERVER.set(Self::new(...)?).map_err(|_| "failed initializing socks5 server").unwrap();
```

### `UDP_SESSIONS::remove(...).unwrap()` 可 panic
- 位置: `crates/tuic-client/src/socks5/handle_task.rs:143`
- 类别: concurrency / 置信度: medium
- 描述:assoc_id 是 u16 易回绕碰撞,远端关闭路径下 remove 返回 None 引发 panic 终止 SOCKS5 连接处理。
- 建议:`let _ = ...remove(&assoc_id);` 并扩宽 id。
- 证据:
```rust
UDP_SESSIONS.get().unwrap().write().await.remove(&assoc_id).unwrap();
```

## Low 问题

- `decode_address` 用 `buf.chunk()` 做索引读取,非 contiguous 安全 — `crates/wind-tuic/src/proto/mod.rs:85-135`,在多 chunk 时可 panic。
- 协议重复实现的 decode helpers 与权威 codec 分歧 — `crates/wind-tuic/src/proto/mod.rs:29-42, 45-82, 85-135`,易导致解析分歧。
- `send_packet` 非分片路径下小 `max_datagram_size` 触发下溢 — `crates/wind-tuic/src/proto/udp_stream.rs:229`,改用 `saturating_sub`。
- 未知 UUID 报错携带尝试值并产生时序 oracle — `crates/wind-tuic/src/quinn/inbound.rs:664-666`,统一为 "Invalid authentication"。
- acceptor_loop 把正常断连记为 error — `crates/wind-tuic/src/quinn/inbound.rs:75-82`。
- TLS 私钥 loader 把任意非空字节当 PKCS8 — `crates/tuic-server/src/tls.rs:109-121`。
- TuicheInbound builder 收集 `max_idle_time`/`users` 但不转发 — `crates/wind-tuic/src/quiche/inbound.rs:113-128`。
- `TlsConfig` 类型定义后未引用 — `crates/wind-tuic/src/quiche/tls.rs:1-24`。
- 65KiB 栈缓冲、未用的 `_out` — `crates/wind-tuic/src/quiche/inbound.rs:66-67`。
- DST/SRC 端口范围允许 lo>hi 静默不匹配 — `crates/wind-core/src/rule.rs:367`。
- `SUB-RULE` 丢弃额外子规则并丢失 name — `crates/wind-core/src/rule.rs:625-631`。
- `NOT` / `SUB-RULE` 的 Display 输出多余括号无法回环解析 — `crates/wind-core/src/rule.rs:795-796`。
- `compat::TokioTcpCompat` 模块未使用 — `crates/wind-core/src/outbound.rs:19-57`。
- SOCKS5 UDP `FRAG` 字节被丢弃 — `crates/wind-socks/src/udp.rs:37-40`,违 RFC §7。
- `parse_udp_request_sync` 失败时仍写入 `source_addr` — `crates/wind-socks/src/udp.rs:104-133`。
- Cronet 库搜索路径仅适配 Linux — `crates/wind-naive/src/lib.rs:364-369`。
- `init_log` 对 `wind_naive`/`wind_dns`/`wind_acme` 等 crate 未生效 — `crates/wind/src/log.rs:5-26`。
- `WIND_OVERRIVE_VERSION` 环境变量名拼错 — `crates/wind/src/main.rs:96-99`。
- ACME 后台任务为 fire-and-forget,缺少取消与失败可见性 — `crates/wind-acme/src/lib.rs:81-113`。
- 推断格式时文件被读了两次,存在 TOCTOU — `crates/tuic-server/src/config.rs:763-779`。
- `format_optional_parts`/`format_protocol` 在 Display 时分配中间 String — `crates/tuic-server/src/acl.rs:30-39, 89-94`。
- `Error` 变体大量未构造,疑似遗留 — `crates/tuic-server/src/error.rs:20-31`。
- `AppContext.cancel` 仅被装饰使用,从未触发 — `crates/tuic-server/src/lib.rs:20-39`。
- `forward.rs` 中 `UDP_SESSIONS` 注册表只写不读,纯死代码 — `crates/tuic-client/src/forward.rs:32-38`。
- 默认 SNI 取 `server.0` 即便是 IP 字面量 — `crates/tuic-client/src/wind_adapter.rs:42`。
- `Error::Timeout/WrongPacketSource/Socks5` 变体未构造 — `crates/tuic-client/src/error.rs:17-26`。

## Nit / 风格建议

- `proto/cmd.rs:95-100, 109`:`assoc_id: assos_id` 是拼错的重命名,消除。
- `proto/mod.rs:149-164`:`encode_and_send_uni` 同时接 `cmd_type` 与 `command`,易传不一致;直接从 `(&command).into()` 推导。
- `proto/udp_stream.rs:44, 46`:`next_pkt_id`/`fragment_buffer` 注释提到不存在的 Mutex。
- `quiche/utils.rs:86-108`:`QuicheError` 变体全部未构造。
- `quinn/outbound.rs:53-55`:`rustls::crypto::*::install_default()` 在每次构造时重复调用,改为 `OnceLock`。
- `quinn/inbound.rs:75-82`:`error!` 对 `ApplicationClosed`/`LocallyClosed`/`TimedOut` 等普通断连噪音过大。
- `socks/ext.rs:88`:`try_join!` Ok 分支 `warn!("unreachable")` 误导。
- `socks/inbound.rs:77`:`SocksInbound::new` 标 `async` 但无 await。
- `tls.rs:162-163`:测试辅助函数中的 `key_pair.serialize_der()` 是死调用。
- `naive/lib.rs:322-333`:每包重新分配 64KiB 缓冲;`lib.rs:232-233`:`target.to_string()`/`payload.to_vec()` 每包重复分配。
- `main.rs:87-93`:clap 解析错误打到 stdout 且退出码 0,应用 `err.exit()`。
- `tuic-server/config.rs:1645`:`assert!(opt.is_none() || opt.is_some())` 永真。
- `tuic-client/forward.rs:89-93`:正常监听启动信息使用 `warn!`。

## 建议的修复路线图

### PR 1 — 安全默认值与认证/0-RTT/源校验(最高优先级)
关联问题:默认 `skip_cert_verify = true`、0-RTT 无重放保护、SOCKS5 UDP 中继接受任意源、`tuic-client` UDP 关联首发包竞争、ALPN 硬编码 `h3`、未知 UUID 错误差异化与时序 oracle、ACME 任务无可观察失败、TLS 私钥 loader 接受任意字节。这一组直接影响产品的安全语义,且修复风险可控。

### PR 2 — 协议/编解码 panic 与流终止类正确性
关联问题:`Bytes::copy_to_bytes` 缺校验、QUIC uni 流未 `finish()`、`buf.chunk()` 索引非 contiguous 安全、重复的 `decode_*` helper、`send_datagram` Chain 拷贝、分片 `saturating_sub` 除零、`saturating_sub` 漏用、`send_packet` 非原子 pkt_id、攻击者可控分片字段。重点是协议层的健壮性。

### PR 3 — 会话/任务/资源泄漏与并发控制
关联问题:`quinn/inbound` UDP 会话驱逐不取消任务、容量 `u16::MAX`、`spawn_handler` 1s 超时杀连接、`tokio::select!` `endpoint.accept()` None panic、`tuic-server` ctrl-c 未 cancel、TLS watcher 错误退出循环、hash 在 reload 前 commit、`udp_assoc_counter` 回绕、`socks/udp` 关联 `source_addr` 在 parse 前写入、`forward.rs` 每包 spawn、Naive UDP 每包新隧道、Naive 无界 channel、直连 UDP 任务 leak、`UDP_SESSIONS.remove().unwrap()` panic。优先解决会话生命周期闭环。

### PR 4 — 路由/规则/调度/配置正确性
关联问题:`rule_target_to_action` 小写化错配、UDP 调度统一哨兵导致 ACL 失效、空 AND/OR 悖论、IP-SUFFIX 与 IP-CIDR 重复、DomainSuffix/Keyword 每次分配、`wildcard_match` 每次编译正则、NOT/SUB-RULE 显示不可回环、`SUB-RULE` 丢内容、端口范围 lo>hi、`TargetAddr` 反序列化、`copy_io` half-close、`tuic-server` 配置 send/receive_window rename 互换、格式推断 `=` 误判、扩展名分流、`attempts` 缺省、`acl.pest` 文法过宽、`AclAddress::Ip` `/32` 误用。覆盖最大用户面错配。

### PR 5 — 性能、代码质量与死代码清理
关联问题:`info!` 在每包打印、`format_optional_parts` 分配、`rustls::install_default()` 重复调用、`StackPrefer` 重复定义、`compat::TokioTcpCompat` 死模块、`TlsConfig` 未引用、`QuicheError`/`Error` 未构造、`UDP_SESSIONS` 注册表只写不读、`init_log` 漏 crate、`WIND_OVERRIVE_VERSION` 拼写、clap 错误流、`tuic-client/wind_adapter` 默认 SNI、各种注释/命名 nit。集中清理可显著降低维护成本。
