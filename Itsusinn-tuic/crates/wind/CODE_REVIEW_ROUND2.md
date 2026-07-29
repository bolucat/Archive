# wind/tuic 项目逐行代码审阅报告（第二轮）

> 本轮在当前工作树（分支 `claude/beautiful-easley-2d7b2b`，基线 commit `3b7ce55`）对 `crates/` 下全部一手 Rust 代码（16 个 crate，约 3.5 万行）做逐行复审，并逐条核对首轮 [`CODE_REVIEW.md`](CODE_REVIEW.md) 中 99 项发现的当前状态。审阅方式：按子系统并行 8 路 fan-out，全文通读每个 `.rs` 文件；对最高影响的结论（GEOIP 接线、`is_private_ip`、`handle_udp` 循环、ACL 文法边界）由主审人复核代码属实。`patches/` 仅审阅本项目自有的两处本地修改。

## 摘要

首轮 99 项问题绝大多数已修复（协议 panic、uni 流 `finish()`、rename 字段互换、TLS loader、优雅停机、按包 spawn、无界 channel、UDP assoc 回绕、规则热路径重复编译等基本清零）。本轮**新确认 73 项**，整体质量明显提升，但仍暴露若干系统性缺陷，集中在三条主线：

1. **QUIC quiche 后端的 driver 是本轮最薄弱环节** —— 存在两个 high 级并发缺陷（入站补投递依赖无关事件唤醒 → 纯上传尾部数据/FIN 可永久滞留；出站 `out_queue` 无上限 → 单个慢对端可 OOM），外加一批 medium 正确性问题（对端 RESET 被吞成干净 EOF、`stream_send` 错误被静默丢弃、drop 未 finish 发 FIN 而非 RESET）。quiche 后端在语义完整性上明显落后于 quinn 后端。

2. **`handle_udp` 生命周期缺陷贯穿 wind-tuic 与 tuic-client** —— `TuicOutbound::handle_udp` 只在全局 shutdown 时才返回，导致每个 UDP 会话结束后仍残留空转任务、`udp_session` 缓存条目与 assoc_id 永不释放；tuic-client 侧 `abort()` 路径还跳过了 Dissociate 发送。长跑客户端上会无界累积。

3. **多处 fail-open 安全降级** —— GEOIP/GEOSITE 规则未接线（geodata crate 无任何消费者，`geoip:cn` 等规则永不匹配）；ACL 文法关键字无边界断言（`private` 前缀域名被吞成网段规则）；无效 CIDR 的 reject 规则静默丢弃；未知 outbound `type` 拼写错误静默回退 direct；`is_private_ip` 不识别 IPv4-mapped IPv6，配合出站二次解析可被 DNS rebinding 绕过。

**建议修复顺序**：(1) fail-open 类安全降级（GEOIP 接线 / ACL 边界 / 未知类型 / `is_private_ip`）；(2) quiche driver 的 lost-wakeup 与无界缓冲；(3) `handle_udp` 会话生命周期与 Dissociate；(4) 被解析却不生效的配置项（`stream_timeout`、`certificates`、`max_*_receive_window`、dns serde default）；(5) 测试断言缺失（两个 integration 测试完全空转）；(6) 其余正确性与质量清理。

## 统计

- **新确认问题数**: 73（不含首轮已修复项）
- **按严重度**: critical 0, high 7, medium 30, low 27, nit 9
- **按类别**: correctness 41, concurrency 6, security 15, performance 2, quality 9
- **首轮 99 项状态**: 约 70 项 FIXED，约 25 项 STILL PRESENT（多为 nit/低危或有意保留的 pub API），少数 UNVERIFIABLE（代码已重构消失）

---

## High 问题（7）

### H1. quiche driver：入站补投递依赖无关事件唤醒，纯上传场景尾部数据与 FIN 可永久滞留
- 位置: `crates/wind-quic/src/quiche/driver.rs:370-393, 519-534, 644-647`
- 类别: concurrency / 置信度: high
- 描述: 流的 inbound 通道（容量 64）满后，数据积压在 `pending_in`（≤256KB）。`flush_inbound` 只在 `read_stream`（收到新包）与 `process_writes`（`wait_for_data` 因 outbound chunk/命令返回）时被调用；`QuicheRecv::poll_read` 排空通道**不产生任何 driver 事件**（`wait_for_data` 只 select `waiters` 与 `cmd_rx`）。在"对端已发完/被流控阻塞、本地无反向流量、无命令"的场景（典型纯上传），应用读空通道后剩余数据与 FIN 永不再投递，连接只能被 idle timeout 杀死、尾部数据丢失。双向中继因反向流量频繁唤醒 worker 而掩盖此问题。
- 建议: recv handle 在通道由满转空时经 `cmd_tx` 发轻量 `FlushInbound(sid)`；或用 `Sender::reserve` 型 future 放入 `waiters`，使"通道有空位"成为可唤醒事件。

### H2. quiche driver：`out_queue` 无上限，应用背压失效，单个慢对端可 OOM
- 位置: `crates/wind-quic/src/quiche/driver.rs:123, 539-551, 627-638`
- 类别: concurrency / 置信度: high
- 描述: `wait_for_data` 收到 `Ev::Out` 后无条件把 chunk 推入 `out_queue` 并立即重新武装 `WaitOut`。`stream_send` 因对端流控返回 `Done` 时数据留在 `out_queue`，但 driver 仍继续从通道抽取，通道腾空后 `QuicheSend::poll_write` 又成功——背压完全失效。代理场景（上游快、QUIC 对端慢）会把上游数据无限堆入 `out_queue`。`out_datagrams` 同理（`SendDatagram` 命令无界）。
- 建议: `out_queue` 超阈值（如 256KB）时不再武装该流 `WaitOut`，把 `rx` 存回 `StreamIo`，待 `write_stream` 排空到阈值以下再 push；`out_datagrams` 超限丢弃最旧项。

### H3. wind-tuic：开启 masquerade 时，纯 TUIC 连接关闭后 `run_masquerade` 停放任务泄漏至进程退出
- 位置: `crates/wind-tuic/src/server/mod.rs:380, 551-563` + `crates/wind-tuic/src/server/masquerade.rs:67-70`
- 类别: concurrency / 置信度: high
- 描述: `spawn_h3_router` 为每连接 spawn 一个 `run_masquerade`，它在建 h3 server 前停放于 `select! { cancel.cancelled(), go.notified() }`。纯 TUIC 连接 `go` 永不触发；而 `serve_connection` teardown 只 cancel `acceptor_cancel` 与 `udp_root_cancel`，**从不 cancel `cancel` 本身**（仅全局关闭/kick 时被父级触发）。于是每条正常 TUIC 连接关闭后，该停放任务连同它持有的 `conn`/`close_conn`/`uni_rx`/`bidi_rx` clone 一直存活到进程级关闭，任务数随连接数无界增长。
- 建议: 让 masquerade 的停放 `select!` 增加 `_ = conn.closed() => break`，或把交给它的 token 改为 `acceptor_cancel.child_token()`（已在 560 行随之取消）。

### H4. `handle_udp` 只在全局 shutdown 才返回：UDP 会话/assoc_id 永不释放，abort 路径跳过 Dissociate（跨 wind-tuic 与 tuic-client）
- 位置: 根因 `crates/wind-tuic/src/quinn/outbound.rs:525-542`；受害调用点 `crates/tuic-client/src/forward.rs:204-215, 247-263`、`crates/tuic-client/src/socks5/handle_task.rs:114-125, 178`
- 类别: concurrency / 置信度: high
- 描述: `handle_udp` 分配 `cancel = self.token.child_token()` 却从不 cancel 它，外层 `loop { select! { sleep(30s) => info, ctx.token.cancelled() => break } }` 只有全局 token 能退出。因此客户端 UDP 会话结束（内部 `udp_task` 因通道关闭而退出）后：(1) `forward.rs` idle 回收只 drop `tx_to_out`，外层 `handle_udp` 及其 spawn 永生（每 30s 打一条 active 日志），每次换端口/超时重建都累积一个任务；(2) `handle_task.rs` 用 `outbound_handle.abort()` 强杀，使 `drop_udp`（Dissociate）**从不发送**，服务器端关联只能等 GC 超时。两路径下 `udp_session` 缓存条目也永不 remove。（主审人已复核 `outbound.rs:525-542`：循环确实仅 `cancel_healthy = self.ctx.token`，退出后才 `drop_udp`。）
- 建议: `handle_udp` 应在 client_stream 两端关闭（udp_task 退出）时返回并执行 `drop_udp` + `udp_session.remove(assoc_id)`；tuic-client 侧把 `abort()` 改为协作取消，确保 Dissociate 发出。

### H5. wind-acl：GEOIP/GEOSITE/ASN 规则从未接线，编译成功但永不匹配（fail-open）
- 位置: `crates/wind-acl/src/engine.rs:106-119`（另见 `crates/wind-acl/src/syntax/apernet.rs:416,424`）
- 类别: correctness / 置信度: high
- 描述: `do_route` 构造 `MatchContext` 用 `..Default::default()`，`geoip_lookup`/`geosite_lookup`/`asn_lookup` 恒为 `None`，且 `AclEngineBuilder` 无任何注入 geodata 的 API。wind-core 的 `RuleType::GeoIp/GeoSite/IpAsn` 在 lookup 为 `None` 时 `unwrap_or(false)`。因此 apernet 里最常见的 `reject(geoip:cn)`、`geosite:` 及 Clash `GEOIP,...` 规则全部静默失效，流量落入默认出站。全仓 grep 确认 `wind-geodata` crate 无任何消费者。（主审人已复核 `engine.rs:111-119`，属实。）
- 建议: 给 `AclEngineBuilder` 增加 `geodata(Arc<GeoData>)` 入口，构造 ctx 时填入 lookup 闭包；至少在 build 时检测到 Geo/ASN 规则但无 geodata 时 `bail!` 或大声 warn，而非静默 never-match。

### H6. tuic-tests：`test_server_client_integration` 完全空转，无任何断言
- 位置: `crates/tuic-tests/tests/integration_tests.rs:361, 370, 385, 391, 481`
- 类别: correctness / 置信度: high
- 描述: `test_tcp_through_socks5`/`test_udp_through_socks5` 返回 `bool` 但返回值全被丢弃，三个子测试块又被 `let _ = timeout(...)` 包裹，整段 260 行无一条 `assert`。TCP/UDP 中继彻底断裂时该测试仍绿灯。此外服务器被 10s `timeout` 包裹（line 278），而测试体串行 sleep 最长约 17s，concurrent 阶段大概率在服务器已死后运行——同样因无断言而无声通过。
- 建议: 捕获两个 `test_*_through_socks5` 的返回值并 `assert!`；去掉外层 `let _ = timeout`（改 `.expect`）；取消/延长服务器内部 10s timeout。

### H7. tuic-tests：`test_ipv6_server_client_integration` 同样空转
- 位置: `crates/tuic-tests/tests/integration_tests.rs:651, 657, 670, 676`
- 类别: correctness / 置信度: high
- 描述: 与 H6 同构，IPv6 TCP/UDP 结果被丢弃、无断言。测试注释声称"验证 IPv6 地址错误"，但即便 IPv6 路径完全回归（或 CI 无 IPv6）也通过。
- 建议: 同 H6 加 `assert!`；若要容忍无 IPv6 环境，先探测 `[::1]` 可绑定再决定 skip，而非静默通过。

---

## Medium 问题（30）

### 安全（security）

**M1. `is_private_ip` 不识别 IPv4-mapped IPv6，私网/回环守卫可被绕过**
- 位置: `crates/wind-core/src/utils.rs:54-72`
- V6 分支只查 `fc00::/7`、`fe80::/10`。`::ffff:10.0.0.1`、`::ffff:192.168.1.1` 的 `octets()[0]==0x00`，返回 `false`。该函数是 `wind-acl/src/engine.rs:97`、`tuic-server/src/wind_adapter.rs:160`、`legacy/mod.rs:175` 的私网防护基石，故 `drop_private` 可被映射地址绕过；回环守卫用 std 的 `is_loopback()`，对 `::ffff:127.0.0.1` 亦返回 `false`。（主审人已复核，属实。）
- 建议: 守卫内先 `let ip = ip.to_canonical();` 归一化再判定；并考虑补 `100.64.0.0/10`（CGNAT）。

**M2. tuic-server ACL 文法关键字无边界断言，`private`/`localhost` 前缀域名被吞成关键字+hijack**
- 位置: `crates/tuic-server/src/legacy/acl.pest:9, 15-31, 100-102`
- `localhost_kw`/`private_kw` 是裸字面量且无 `& (WHITESPACE|EOI)` 边界，PEG 选中不回溯。`proxy privatetracker.org` 解析为 `addr=Private, hijack="tracker.org"`——把整个 RFC1918 网段路由到 proxy；`allow localhost5.com` 解析为 `Localhost + port 5 + hijack ".com"`。规则语义被静默篡改。（主审人已复核文法，属实。）
- 建议: 给三个关键字追加 `~ &(WHITESPACE | EOI)` 边界断言（同 ipv6 修法）。

**M3. tuic-server：无效 CIDR 的 ACL 规则静默丢弃（无 warn），reject 规则 fail-open**
- 位置: `crates/tuic-server/src/legacy/mod.rs:668-674`
- `AclAddress::Cidr` 解析失败直接返回 `vec![]`，无任何日志（相邻 Ip 分支有 warn）。文法允许 `/999`，故 `reject 10.0.0.0/99` 会通过加载然后整条 reject 无声消失，本应拒绝的流量全部放行。
- 建议: 与 Ip 分支一致 warn；更好是 ACL 编译失败拒绝启动。

**M4. tuic-server：未知 outbound `type` 静默回退为 direct，拼写错误使流量绕过代理**
- 位置: `crates/tuic-server/src/wind_adapter.rs:80-101`
- `match rule.kind.as_str() { "socks5" => ..., _ => Direct }`。`type = "Socks5"`/`"sock5"` 等拼写错误不报错不告警，直接建 DirectOutbound，本应经 SOCKS5 隧道的流量以服务器本机 IP 直连。
- 建议: 显式匹配 `"direct"`，其他值配置阶段报错或至少 warn。

**M5. tuic-server：drop_private/drop_loopback 守卫与出站各自独立解析 DNS，可被 rebinding 绕过**
- 位置: `crates/tuic-server/src/wind_adapter.rs:150-171`（参照 `wind-base/src/direct.rs:57`）
- 守卫先 `resolve_target` 检查，通过后 `DirectOutbound::handle` 又独立解析建连。攻击者权威 DNS 可第一次返回公网 IP、第二次返回内网 IP（TOCTOU rebinding），绕过 drop_private 实现 SSRF；同时每连接对同域名解析两次。
- 建议: 解析一次并把结果 IP 透传给出站，既堵 rebinding 又省一次查询。

**M6. SOCKS5 UDP associate 源校验只比 IP 不比端口，同 IP 进程/NAT 用户可劫持（两处独立代码路径）**
- 位置: `crates/wind-socks/src/udp.rs:133-160, 174` 与 `crates/wind-socks/src/ext.rs:59`；`crates/tuic-client/src/socks5/udp_session.rs:83-94` 与 `crates/tuic-client/src/socks5/mod.rs:198-201`
- 两处均只比对 `src.ip()`，且都丢弃客户端在 UDP ASSOCIATE 请求里声明的 DST.ADDR/DST.PORT。与合法客户端同 IP 的其他进程/同 NAT 出口用户可抢发首包劫持关联。wind-socks 侧更严重：每个解析成功的包都 `source_addr_rx.store(addr)` 重设回复目的端口。
- 建议: 客户端声明非零端口时 latch 前额外比对端口；wind-socks 侧首包后锁定端口，后续端口变更仅告警不切换回复目的。

**M7. tuic-core：分片重组缓冲无字节预算，一致声明 `frag_total=255` 可占数百 MB 并挤出合法分组**
- 位置: `crates/tuic-core/src/udp.rs:66-71, 170-174`
- 三项字段校验已到位，但只按条目数限界：`Cache::new(1000)` × 每组 ≤255 片 × 每片最大 QUIC datagram ≈ 300MB+，无每关联配额、无总字节预算，无 weigher。已认证对端用递增 pkt_id 各发 254 片"永不完成"的组即可在 30s 窗口顶满，同时 moka 容量驱逐把受害者正在重组的分组挤出。
- 建议: 给 buffer 增加按 payload 字节计权的 `weigher` + 总字节上限，或按 assoc_id 设组数配额；收紧 `FRAGMENT_TIMEOUT_MS`（30s 对 UDP 分片过长，3-5s 足够）。

**M8. geodata 缓存 start/len 偏移未校验，恶意/损坏缓存在每次查询时 OOB panic**
- 位置: `crates/wind-geodata/src/lib.rs:56-76`；`query.rs:17-33, 55-70, 161-181`
- `validate()` 只做 magic/version + rkyv 结构校验，`CategoryInfo`/`CountryInfo` 的 `exact_start`/`v4_len` 只是普通 u32，不校验"偏移落在对应 Vec 内"。被篡改/位翻转后仍过结构校验的缓存会让 `v[mid]` 越界，在每次路由查询时 panic（DoS）。与注释承诺的"fully type-check ... so snapshot() can use unchecked accessor"不符。
- 建议: `validate()` 遍历 categories/countries，校验 `start+len <= vec.len()` 及 name 有序性，失败返回 `GeoDataError::Validate`。

**M9. ACME/自签私钥以默认文件权限写盘（无 0600）**
- 位置: `crates/wind-acme/src/http01.rs:185-186`；`crates/wind-acme/src/selfsigned.rs:26-27`
- `tokio::fs::write(key_path, pem)` 在 Unix 上按默认 umask（通常 0644）创建私钥，同机其他用户可读。目前调用方尚未接线（quiche 后端预留），属潜在暴露。
- 建议: Unix 下用 `OpenOptions::mode(0o600)` 写 key，或写后 `set_permissions`。

### 正确性（correctness）

**M10. quiche：对端 RESET_STREAM 被吞成干净 EOF，截断数据被当完整**
- 位置: `crates/wind-quic/src/quiche/driver.rs:355-362`；`stream.rs:130-144`
- `stream_recv` 任何错误（含 `StreamReset(code)`）只 `trace!` 后置 `in_fin=true`，`QuicheRecv::poll_read` 把 sender 关闭当 clean EOF。quinn 后端同场景返回 `Reset(code)` io 错误。对 h3 adapter 尤害（丢失 reset 码语义），对中继则把残缺 payload 当完整转发。
- 建议: 通道类型改 `mpsc::Sender<Result<Bytes, u64>>`，把 reset 映射为 `io::Error`。

**M11. quiche：`stream_send` 错误被吞，handle 后续写入永远"成功"；超对端 MAX_STREAMS 额度的流数据静默丢失**
- 位置: `crates/wind-quic/src/quiche/driver.rs:287-306, 413-431, 541-544`
- `write_stream` 遇 `StreamStopped`/`StreamLimit` 等只 `debug!` 后清 `out_queue` 置 `out_done`，但不关闭 out 通道，`poll_write` 继续返回 `Ok`。且 `open_local_bi/uni` 本地直接分配 sid、从不等对端流额度，超额流首次 send 即 `StreamLimit` → 静默丢弃，应用看到 open 成功、写成功、数据全丢。
- 建议: 出错时关闭该流 out 通道让 `poll_write` 返回 `BrokenPipe`；`alloc_id` 前检查 `peer_streams_left_*`，不足则挂起 pending open。

**M12. quiche：`QuicheSend` 未 finish 就 drop 时发干净 FIN 而非 RESET，与 quinn 语义相反**
- 位置: `crates/wind-quic/src/quiche/stream.rs:37-43`（无 Drop）+ `driver.rs:546-550`
- drop send half → 通道关闭 → `out_done=true` → 排空后发 FIN。任务被取消/panic 中途丢弃 send half 时，对端看到"干净 EOF"、把半截数据当完整。quinn 对未 finish 的 drop 是隐式 reset，两后端在同一 trait 下行为相反。
- 建议: 给 `QuicheSend` 实现 `Drop`：`!finished` 时发 `StreamShutdown { write: true, code: 0 }` 再关通道；driver 收到后不补 FIN。

**M13. quinn 后端忽略 `max_conn_receive_window`/`max_stream_receive_window`，与 config 文档矛盾**
- 位置: `crates/wind-quic/src/quinn/mod.rs:250-276`（文档 `config.rs:140-152`）
- `build_transport` 只读 `t.receive_window` 设 `stream_receive_window`，四个 `init_*`/`max_*` 字段无人读取，连接级 `receive_window` 从未设置。运营者按文档调大窗口在 quinn 后端静默无效。
- 建议: `stream_receive_window(t.max_stream_receive_window.unwrap_or(t.receive_window))`，`max_conn_receive_window` 为 `Some` 时调 `tr.receive_window(..)`。

**M14. wind-naive TCP 桥不支持 half-close：本地 EOF 立即撕毁隧道丢弃在途响应；远端 EOF 后桥接永久挂起**
- 位置: `crates/wind-naive/src/lib.rs:411-446`
- (1) 本地 `read` 返回 `Ok(0)`（客户端半关闭等响应，HTTP 常见）即 `break`，drop 通道取消隧道，服务端未送达响应被丢；(2) io 线程读到 naive EOF 退出后 `naive_read_rx.recv()` 返 `None` 使该分支禁用，若客户端此后沉默则 `stream.read` 分支仍活跃，桥接协程永挂，连接泄漏。
- 建议: 本地 EOF 时只停 uplink（半关闭 NaiveConn 写端）继续泵 downlink；远端 EOF 时对本地 `shutdown().await` 后退出。

**M15. tuic-client `wind_adapter` 静默丢弃大半 relay 配置（含自定义 CA 证书）**
- 位置: `crates/tuic-client/src/wind_adapter.rs:71-86`（对照 `config.rs:96-160`）
- `TuicOutboundOpts` 无 `certificates`/`disable_native_certs`/`congestion_control`/`send_window`/`receive_window`/`initial_mtu`/`min_mtu`/`gso`/`pmtu`/`udp_relay_mode`/`disable_sni`/`timeout`/`proxy` 字段，全被丢弃。`certificates` 最伤：自签/私有 CA 证书永不加载（`utils::load_certs`/`ServerAddr` 成死代码），连接必败，等于逼用户开 `skip_cert_verify`；`udp_relay_mode="quic"` 静默按 native 运行。
- 建议: 把关键字段（certificates/disable_native_certs/udp_relay_mode/congestion_control）接入 `TuicOutboundOpts`，或对不支持的非默认值报错/告警。

**M16. tuic-server：一批配置项被解析、文档化但从未生效（含 `stream_timeout` 被硬编码为 ZERO）**
- 位置: `crates/tuic-server/src/config.rs:107-137, 296-300`；`wind_adapter.rs:87, 95, 270-295`
- `udp_relay_ipv6`/`dual_stack`/`task_negotiation_timeout`/`gc_interval`/`gc_lifetime`/`max_external_packet_size`/`stream_timeout`/`backend.quinn.pmtu` 无消费点。`stream_timeout`（默认 60s）被 `make_outbound_action` 硬编码为 `Duration::ZERO`，而 wind-base 文档明确 ZERO=禁用半关闭回收，即用户配置被静默忽略且实际关闭该功能。`error.rs` 未构造变体正是这些死配置残留。
- 建议: `stream_timeout` 接入 `DirectOutboundOpts`；其余接入 wind-tuic 或删除并靠 `deny_unknown_fields` 报错。

**M17. tuic-client：`UdpSession::new` 无条件应用 `dual_stack`，IPv4 路径 UDP ASSOCIATE 必然失败**
- 位置: `crates/tuic-client/src/socks5/udp_session.rs:45-49`（对照 `mod.rs:66-72`）
- TCP 监听只在 `is_ipv6()` 时才 `set_only_v6`，但 UDP 关联 socket 只要配了 `dual_stack` 就无条件调用。配 `dual_stack` 且 SOCKS5 监听 IPv4（默认 `127.0.0.1:1080`）时，AF_INET socket 上设 `IPV6_V6ONLY` 返回错误 → 每个 UDP ASSOCIATE 都 GeneralFailure。
- 建议: 与 `Server::new` 对齐，仅当 `local_ip.is_ipv6()` 且非 v4-mapped 时应用 `set_only_v6`。

**M18. wind：`WIND_` 前缀 env 合并无分隔符配置，且 CLI 承诺的 base64 输入从未实现**
- 位置: `crates/wind/src/conf/persistent.rs:294`；`cli.rs:9`
- `figment.merge(Env::prefixed("WIND_"))` 无 split 分隔符，带 tag 的枚举数组 `inbounds`/`outbounds` 无法正确表达、可能意外注入顶层键。且 CLI `value_name = "FILE/BASE64-TEXT"` 宣称 `-f` 可接受 base64，但 `load()` 只当文件路径处理，从不解码。
- 建议: 明确文档化/移除 Env 合并（至少 `.split("__")`）；实现或删除 base64 输入路径。

**M19. wind-dns：`DnsConfig` 的 `mode`/`stack_prefer` 缺 `#[serde(default)]`，部分 `[dns]` 配置反序列化失败**
- 位置: `crates/wind-dns/src/config.rs:40-42, 66-67`
- 容器无 `#[serde(default)]`，`#[educe(Default)]` 不影响 serde。写 `[dns]\nmode="google"`（省略 `stack_prefer`）得 "missing field `stack_prefer`"。与已修的 `attempts` 同类 bug，当时只补了一个字段。
- 建议: 容器加 `#[serde(default)]`（保留 `deny_unknown_fields`）一次覆盖所有字段。

**M20. wind-dns：文档称默认 OS resolver，实际默认 CloudflareTls**
- 位置: `crates/wind-dns/src/config.rs:17-30`
- `#[educe(Default)]` 标在 `CloudflareTls` 上，但 doc 写"Defaults to the OS resolver, matching pre-existing behaviour"。省略 `[dns]` 的用户会不知情地把全部出站解析改走 Cloudflare DoT（853 被封环境直接解析失败，且有隐私影响）。
- 建议: 若求兼容把默认移到 `System`；若有意改默认，更新 doc 并在 release note 标注。

**M21. wind-acme：`ensure_acme_cert` 每次签发都注册全新 ACME 账户，凭据被丢弃**
- 位置: `crates/wind-acme/src/http01.rs:144-155`
- `Account::builder()?.create(...)` 每次新建 LE 账户，`_credentials` 丢弃不持久化。LE 有"10 accounts/IP/3h"限速，频繁重启或多域名逐个续期会触发限流锁死签发。对照 `resolver.rs` 的 `DirCache` 会缓存 account，此处缺失。
- 建议: 用 `from_credentials(...)` 加载缓存凭据，首次创建后序列化到 cert 同目录。

### 并发（concurrency）

**M22. wind-socks：UDP 中继回复方向的计量任务与 `handle_udpstream` 任务在关联结束时不随控制连接取消**
- 位置: `crates/wind-socks/src/inbound.rs:278-296, 311-342`
- `count_udp` spawn 的两个任务仅在 channel 关闭时退出（依赖 drop 顺序而非显式取消）；`handle_udpstream` 挂的是会话级 `cancel`，单纯 UDP 关联正常结束（控制 TCP 断开）不触发该 token。整体依赖 channel 级联关闭，缺显式生命周期闭环。
- 建议: 每 UDP 关联建子 CancellationToken，`run_udp_proxy` 返回后 cancel，两类任务都 `select!` 该 token。

### 性能（performance）

**M23. quiche driver：每 16KB chunk / 每条命令触发一次完整 worker pass，且每 pass 做 O(streams) 的 Vec 收集**
- 位置: `crates/wind-quic/src/quiche/driver.rs:519-555, 577, 640-647`
- `wait_for_data` 每次只消费一个事件就返回，随之跑完整 `process_reads`/`process_writes`/`conn.send`；`process_writes` 每 pass 调 `stats()`、collect 两次全量 sid Vec 并遍历所有流。高吞吐上传/datagram 密集时开销随并发流数线性放大。
- 建议: 消费首个事件后 `try_recv` 批量吸干 `cmd_rx` 与就绪 waiters；维护"脏流"集合只遍历有 pending 的流；复用 `sids` 缓冲。

### 测试（correctness / quality，masking real bugs）

**M24. TUIC 错误密码/未知用户测试是空转，从不验证认证被拒绝**
- 位置: `crates/wind-test/src/tuic.rs:293-314, 319-341`
- 只断言 `TuicOutbound::new` 成功（QUIC 传输层无论密码对错都成功），后续无任何验证服务器拒绝了 Auth 流。若出现"任意密码均接受"的认证旁路回归，这两个以 auth 命名的测试照样通过，全套无其他负路径认证兜底。
- 建议: 连接后用错误凭据发一次 `handle_tcp` 并断言在 auth_timeout 内失败，或等待并断言连接被服务器关闭。

**M25. `test_client_proxy_configuration` 把失败标记为"may be expected"**
- 位置: `crates/tuic-tests/tests/integration_tests.rs:811-817`
- 唯一功能验证结果 `success` 失败时只打印警告后 `Ok(())`。TUIC client 经 SOCKS5 前置代理出站的能力回归时测试仍通过。
- 建议: `assert!(success, ...)`；确不可用的环境用 cfg/env 显式 skip。

**M26. "bind 后 drop" 的 UDP 端口预留存在 TOCTOU 竞态（4 处）**
- 位置: `crates/wind-test/src/tuic.rs:184-187, 679-681, 723-725, 869-871`
- 先 bind 取端口、drop、再让服务器（约 300ms 甚至数秒后）重绑该端口，窗口期内并发测试/外部进程可抢占导致 bind 失败，而错误藏在被丢弃的 JoinHandle 中，表现为数秒后的超时 flake。
- 建议: 让 `TuicInbound` 暴露实际绑定地址后直接 listen `:0`；或返回前 poll 一次 JoinHandle，已出错即以 bind 错误失败。

**M27. socks5 测试 harness 架构文档与实现不符：TUIC inbound 被启动但零流量经过**
- 位置: `crates/wind-test/src/socks5.rs:386-388, 466-476, 560-581`
- 文档声称链路 `SOCKS5 → TUIC Outbound → TUIC Inbound → Direct`，但 `handle_tcpstream/handle_udpstream` 对 SOCKS 流量直接 `TcpStream::connect` 出站，`TuicInbound` 虽监听却无客户端连它——死配置。读者会误以为这些 SOCKS 测试覆盖了 TUIC 加密中继（实则未）。
- 建议: 删掉 TUIC inbound 与失实注释，或真正把 SOCKS 流量接到 TuicOutbound→TuicInbound 链上。

**M28. socks5 测试硬编码端口且 bind 失败被静默吞掉**
- 位置: `crates/wind-test/src/socks5.rs:604, 624, 705, 958`（16666-16669）；`416-423, 572-581`
- 四个测试硬编码 SOCKS 端口；`run_test_proxy` 经 `ctx.tasks.spawn` 启动后立即返回 Ok，listen/bind 的 Result 无人检查，`start_test_proxy` 仅 `sleep(500ms)` 认为就绪。端口占用时表现为误导性连接失败而非明确 bind 错误。
- 建议: listener 改绑 `:0` 暴露实际端口；bind 结果经 oneshot 回传，就绪后再返回。

**M29. 依赖公网的测试（example.com / 8.8.8.8）导致离线/受限 CI 必然失败**
- 位置: `crates/wind-test/src/socks5.rs:592, 598, 607`
- `test_direct_tcp_connection`（example.com:80）、`test_direct_udp_connection`（8.8.8.8:53）、`test_tcp_through_proxy`（经代理访问 example.com:80）需真实外网。
- 建议: 用本地 echo server 替代；直连 smoke 测试标 `#[ignore]` 或 env 开关控制。

**M30. tuic-server：`infer_config_format` 对以 `[` 开头的 TOML 文件误判为 JSON**
- 位置: `crates/tuic-server/src/config.rs:665-667`
- 代码内测试注释已自认是 pre-existing bug：首个非空字符为 `[` 直接判 JSON，而 TOML 可以 `[server]` table 开头。Docker 无扩展名 + 无 `TUIC_CONFIG_FORMAT` 时，`[users]` 开头的合法 TOML 会被送进 JSON5 失败。
- 建议: 对 `[` 开头者先试 TOML table-header 正则再回退 JSON。

---

## Low 与 Nit（36，摘要）

**wind-core**: `dispatch_udp` 首包重放代理任务用裸 `tokio::spawn` 不入 `TaskTracker`（`dispatcher.rs:201-211`，low/concurrency）；`RuleType::IpSuffix`/`SrcIpSuffix` 变体已成死代码（`rule.rs:153,166,346,364`，nit）。

**wind-quic/naive**: `QuicheRecv` drop 后 driver 不发 STOP_SENDING 且 `TrySendError::Closed` 不清 `inbound_tx`，流条目滞留（`driver.rs:379-391,437-448`，low）；h3 adapter `close` 丢弃 h3 错误码固定用 0（`h3_adapter.rs:292-294,319-321`，low）；h3 adapter 把"accept 通道关闭"映射为 `Timeout` 误导诊断（`h3_adapter.rs:332,343`，nit）；wind-naive IPv6 字面量 server_address 派生 SNI 出错（`lib.rs:131-134`，STILL PRESENT）；wind-naive 每包重分配 64KiB 缓冲（`lib.rs:420`，STILL PRESENT nit）；Cronet 库搜索路径仅 `.so`（`lib.rs:453-459`，已 cfg 门控）。

**tuic-core**: 三个 Decoder 把格式错误一律映射 `BytesRemaining` 且 `decode_eof` 在干净 EOF 报错（`addr.rs:64-74`/`cmd.rs:43-53`/`header.rs:48-58`，low，仅测试使用）；`From<io::Error> for ProtoError` 在 debug 构建 `panic!`（`error.rs:46-57`，low）；乱序到达时 `frag_id==0` 只回填 target 不回填 source（`udp.rs:162-164`，low）；`cleanup_expired` 的 Err 被无声 warn（`udp.rs:190-196`，STILL PRESENT nit）；`hex` 应移入 dev-dependencies（nit）。

**tuic-client**: `log_level` filter target 过时（`tuic` 不匹配 `tuic_out`），debug 日志不生效、warn 压不掉每包 info（`main.rs:37-39`，low）；UDP associate 接收缓冲默认 1500，超长 datagram 被内核静默截断（`udp_session.rs:76-77`，low）；`deserialize_server` 对不带方括号的 IPv6 静默错误拆分（`config.rs:358-375`，low）；`wind_adapter` DNS 忽略 `ipstack_prefer`（`wind_adapter.rs:26-31`，STILL PRESENT）；`socks5::Server::set_config` 二次调用 panic（`mod.rs:36-45`，STILL PRESENT）；socks5/mod.rs:121 正常启动信息仍用 `warn!`（nit）；ALPN 经 `from_utf8_lossy` 静默替换非 UTF-8（nit）；forward/socks bind 失败仍只在任务内 warn、主流程无感（STILL PRESENT）。

**tuic-server**: `acme-cache` 目录硬编码相对 cwd 忽略 `data_dir`，多实例互踩（`config.rs:891-898`/`wind_adapter.rs:241`，low）；`clone_rule_type` 经 Display→parse 往返克隆，失败静默降级为 `Match`（`legacy/mod.rs:760-765`，low，fail-open）；`compat.rs` 的 `QuicClient` 死代码（nit）；`max_concurrent_bi/uni_streams` 硬编码 512 未接 Config（`wind_adapter.rs:279-280`，STILL PRESENT）；ipv4 文法接受 999.999.999.999（`acl.pest:34-39`，STILL PRESENT，但 lowering 已 warn+drop 消除 fail-open）；`1.2.3` 被当 Domain 规则永不匹配（`acl.pest:15-25,73-79`，STILL PRESENT）。

**wind-acl/geodata/dns/acme**: `clone_rule_type` 回环失败 fail-open 为 `Match`（`apernet.rs:491-494`，low，与 tuic-server 同型）；HTTP-01 挑战服务器绑 `0.0.0.0:80` 与 resolver 的 `[::]:80` 不一致（`http01.rs:48` vs `resolver.rs:223`，low）；自签证书只查文件存在不查过期，45 天后永久过期（`selfsigned.rs:13-15`，low）；`tcp://` DNS spec 仍走 `udp_and_tcp` 创建 UDP 连接（`resolver.rs:130`，low）；geosite 重复 country_code 不合并、后者静默失效（`builder.rs:57-70`，low）；geodata 缓存 `std::fs::write` 非原子（`lib.rs:45`，nit）；`wind_acl`/`wind_geodata` 未纳入日志 targets（`wind/src/log.rs:11-22`，nit，与已修问题同型复发）。

**wind-socks/wind**: `-D/--work_dir` 被解析但完全未使用（`cli.rs:16-18`，low）；`skip_auth` 字段读入但握手逻辑从不引用（`inbound.rs:34`，low）；password auth 用 hook 时 fast_socks5 已回复"成功"、被拒用户看到成功后才断连（`inbound.rs:160-181`，low，违 RFC1929，已知取舍）；shutdown 超时 `timeout(...).await?` 以错误退出掩盖正常语义（`main.rs:125`，nit）；`util::target_addr_to_socket_addr` 死代码且内部会 panic（`util.rs:16-31`，nit）；SOCKS5 UDP `FRAG` 字节被丢弃违 RFC §7（`udp.rs:170-173`，STILL PRESENT，有 TODO）。

---

## 首轮 99 项状态核对（要点）

**已修复的关键项**（约 70 项）：QUIC uni 流 `finish()`、UDP 会话缓存驱逐取消任务（wind-tuic 侧）、ALPN 采用 `opts.alpn`、`copy_to_bytes` 长度前置校验、`frag_total`/`frag_id` 校验、`next_pkt_id` 原子化、未知 UUID 常量时间比较、tuic-server rename 字段互换、TLS 私钥结构校验、cert watcher continue、优雅停机（Ctrl-C/SIGTERM + drain）、`wildcard_match` 解析期预编译、`DomainSuffix`/`DomainKeyword` 零分配匹配、空 AND/OR 拒绝、端口反转区间拒绝、`rule_target_to_action` 不再小写化、UDP 调度按首包路由（哨兵已消除）、`copy_bidirectional` 正确半关闭 + reaper、naive 单隧道多路复用 + 有界 channel、SOCKS UDP 回复源地址修正、UDP 中继双栈绑定、默认 `skip_cert_verify=false`、config 格式推断修正等。

**仍存在（多为低危/有意保留）**：0-RTT 无应用层防重放（已缓解：默认关闭 + 显式 warn + `send_half_rtt_data=false`）；`TuicheOutbound` 仍为占位、builder 不传播 `verify_certificate` 等；`start_poll` 非幂等；wind-naive 桥"写依赖读"限制、IPv6 SNI 派生、每包 64KiB 重分配；`cleanup_expired` 无声 warn；`Error` 枚举多个未构造变体（保留为 pub API）；tuic-client `ipstack_prefer` 忽略、`set_config` 二次 panic；tuic-server ACL `1.2.3`/999 文法过宽（fail-open 已由 lowering 层消除）、`max_concurrent_streams` 硬编码 512。

**#50 revert 专项**：`58505cf`（reap idle half-closed relays #50）被 `56da727` revert，随后 `14fee8f` 以重构形式重新引入半关闭 reaper。当前 `wind-core/src/io.rs` 的 `copy_bidirectional` 带 `half_close_timeout`、`direct.rs`/socks5 action 均正确传入，reaper 逻辑完整。**revert 未重新引入泄漏。**

---

## patches 本地修改专项审阅

- **`patches/tokio-quiche/src/quic/io/gso.rs`（唯一本地修改）— 正确。** `instant_to_monotonic_nanos` 用一次 `clock_gettime(CLOCK_MONOTONIC)` 与 `Instant::now()` 配对建基准，与 SO_TXTIME 的 clockid（`capabilities.rs:198` 设为 `CLOCK_MONOTONIC`）及 Rust Linux `Instant` 同源；`saturating_add`/`saturating_duration_since` 防溢出与 t 早于基准；cfg 门控与调用点一致。两处次要瑕疵（均不影响生产）：`clock_gettime` 返回值未检查（失败时退化为立即发送，可接受）；保留的上游 `transmute(0u128)` 测试在 32 位上编译不过，建议给该 test 模块补 `target_pointer_width="64"` 门控。
- **`patches/datagram-socket/src/mmsg.rs`（两处 msghdr 构造）— 正确。** 用 `mem::zeroed()` + 逐字段赋值规避 musl（libc ≥0.2.169 的私有 padding）编译失败，与上游 quiche PR #2224 一致；`msg_iov` 指向的 `SmallVec<[IoSlice;16]>` 因每 chunk ≤16 元素且前置 `clear()` 不会 realloc，指针在系统调用时刻有效；`msg_iovlen=1` 整数字面量按字段类型推断同时适配 glibc/musl。

---

## 审阅覆盖

按子系统 8 路并行，全文通读；各子系统均在其报告中逐文件列出行数确认无遗漏。已通读文件覆盖 `crates/` 下全部一手 `.rs`（wind-core / wind-tuic / wind-quic / wind-naive / wind-socks / wind-base / wind / wind-acl / wind-geodata / wind-dns / wind-acme / tuic-core / tuic-client / tuic-server / wind-test / tuic-tests）及 `patches/` 两处本地修改。`forks/`（quiche、rustls-acme 子模块）与 `patches/` 中未经本项目修改的上游代码按范围排除。
