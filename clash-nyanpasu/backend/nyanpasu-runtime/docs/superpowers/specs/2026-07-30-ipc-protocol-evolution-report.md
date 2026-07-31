# nyanpasu-service IPC 协议演进改进报告:状态暴露、update_config 与 config 一致性

- 日期:2026-07-30
- 状态:改进报告(提案,待评审)
- 前置:docs/superpowers/specs/2026-07-29-service-evolution-design.md(S1–S5 已合入 main @ 940076b);其 §2.2 将"丰富状态暴露、switch 专用端点"明确列为非目标延后,O1/O6 记录了延后决定。本报告即该"协议演进"阶段的具体化。
- 目标:**调用方使用 service client 与直接使用 `nyanpasu-core-manager` 拥有对等能力**(观测 + 运行时操作;不含嵌入期策略,见 §3)。
- 调查方式:两个独立代码级调查(IPC 操作面 / core-manager 能力面),全部结论有 file:line 佐证。

---

## 1. 现状确认(代码证据)

### 1.1 内核 IPC 端点信息:manager 已持有,桥接层丢弃

- manager 侧完整持有:`CoreStatus.controller: Option<clash_api::Host>`(`crates/nyanpasu-core-manager/src/state.rs:181-190`),`Host = NamedPipe(PathBuf) | UnixSocket(PathBuf) | Http(Url)`(`crates/clash-api/src/client.rs:20-24`);secret 在 `ResolvedController { host, secret }`(`crates/nyanpasu-core-manager/src/spec.rs:52-57`)。
- 丢弃点唯一:`CoreManagerService::status`(`crates/nyanpasu-service-runtime/src/server/manager_bridge.rs:165-173`)构造 wire `CoreInfos` 时仅保留 `type/state/state_changed_at/config_path`,`controller`、`health`、`revision`、`capabilities`、`pid`、`epoch` 全部读过即丢。
- **secret 从不发布**:所有 publish 位点(`src/manager/publish.rs`)只发 host 不发 secret;secret 仅 `Instance::controller()` 可得,而 manager 不外借 `Instance`。
- `ControllerMode::Passthrough` 硬编码于 `manager_bridge.rs:46-51`,无任何参数可改。Managed 模式的 epoch 级端点生成(`config/mod.rs:244-266`)、controller 重写(`config/clash.rs:61-76`)、graceful 零停机切换(`switching.rs:171-368`)对 service 全部是死代码。
- Passthrough 下端点**来自调用方自己的 config**(`config/clash.rs:39-51` inspect;无 controller 声明则 `Error::ControllerMissing` 拒绝启动,`config/mod.rs:215-226`)。

### 1.2 状态推送:二值枚举,有损且有语义缺陷

- 事件枚举仅两变体:`Event::Log(TraceLog) | CoreStateChanged(CoreState)`(`nyanpasu_ipc/src/api/ws/events.rs:8-21`);`CoreState` 仅 `Running | Stopped(Option<String>)`。
- `map_core_state`(`manager_bridge.rs:226-240`)有损映射:manager 六态 `Stopped/Starting/Running/Restarting/Switching/Stopping`(`state.rs:108-131`)压缩后丢弃 `epoch/pid/attempt/from/to`。两个尖锐后果:
  1. **崩溃循环显示为已停止**:`Restarting { attempt }` → `Stopped(None)`,客户端无法区分真停止与自动重启中;
  2. **Starting 期间自相矛盾**:事件流报 `Stopped(None)`,但此刻调 `start` 会收到 `"core is already running"`(`manager_bridge.rs:129-134`)。
- ws 无重放/补发/快照:断线或 `Lagged` 后 `resubscribe()` 跳到实时尾部(`routing/ws.rs:60-66`),客户端必须另行轮询 `/status` 重新同步。
- 兼容性事实(已实测确认):客户端 `EventStream` 逐条解码,未知变体只产生**单条** `Err(Decode)`,流不中断(`client/shortcuts.rs:89-108` filter_map 语义)。GUI 消费侧是否把单条错误当致命错误未验证。

### 1.3 update_config:wire 零命中,manager 侧整套引擎无人调用

- 契约封闭为 7 操作(`nyanpasu_ipc/src/api/contract.rs:45-113`):status / core-start / core-stop / core-restart / logs×2 / set_dns。grep `update_config|apply_config|patch_config|reload|switch` 全仓 wire 层零命中。
- manager 侧**已实现且公开、但零调用方**的能力:
  - `apply_config(spec, expected_revision)`(`manager/apply.rs:19`):自动分类 → `Noop | Patched(PATCH /configs,22+42+55 字段白名单)| Reloaded(PUT /configs)| Restarted(同 epoch,带备份回滚)| RolledBack | DurabilityUncertain`;`RevisionId` 乐观并发(`RevisionConflict` 错误);patch 后 `GET /configs` **逐叶校验**(`config/mihomo.rs:179-185`)。
  - `switch(spec)`(`switching.rs:56`)、`check_config`(干跑校验,`manager/mod.rs:462`)、`recover_quarantine`(`quarantine.rs:18`)。
- 设计稿 §4.1 预留的 `last_revision: RwLock<Option<RevisionId>>` 字段(设计稿 :102-104)在落地的 `Inner`(`manager_bridge.rs:27-33`)中不存在。
- Passthrough 下 `restart()` 恒走 hard switch(`switching.rs:33-35` 返回 `DegradeReason::PassthroughMode`)。**服务客户端改配置的唯一途径 = 覆写文件 + `POST /core/restart` = 全量进程重启,有停机窗口,无回滚。**
- 隔离死锁:未确认死亡触发 quarantine 后,`start/restart/switch/apply` 永久报 `ManagerQuarantined`,唯一解锁 API `recover_quarantine` 无 IPC 端点——客户端只能重启整个服务。

### 1.4 config 一致性:机制存在,但对调用方完全不可观测

回答"core manager 是否全权管理推送过来的 config":**是,且比预期更深**——

- 调用方的 config 文件只是"源":manager `canonicalize`(递归按键排序、去注释、展开锚点、**拒绝** YAML tag 与非字符串键,`config/mod.rs:162-191`)→ 重新序列化 → 提交为私有运行时副本 `{service_data_dir}/core-runtime/config-{epoch}.yaml`(`runtime_store.rs:127-129,194-206`;目录 0o700/加固 ACL)。**核心运行的是运行时副本,不是调用方的文件**(`switching.rs:443-445` 重定向 `effective_spec.config_path`)。
- Passthrough 下不注入/不改写任何语义字段(controller 三键与 secret 原样);变化仅在序列化形态与文件位置。
- `/status` 回显的 `config_path` 是**源路径**(`publish.rs:116-126` 始终用 `source_spec`)——调用方改了源文件后读 status,路径不变、无 revision 可看,**无从判断运行中的核心是否已采纳修改**。
- "最终一致"的保证机制 manager 内部其实完备:patch 逐叶验证、reload/restart 以运行时文件为准、失败显式回滚为 `RolledBack`、`effective_hash`(FNV-1a over canonical YAML,`config/mod.rs:197-206`)作为变更身份。**缺的只是可观测性**:`ApplyOutcome` 不可达、`SwitchOutcome` 被 `Ok(_outcome) => Ok(())` 丢弃(`manager_bridge.rs:158-159`)、`DurabilityUncertain` 压平成字符串。若 `apply_config` 接线后不暴露结果,`RolledBack`(核心实际在跑**旧**配置)将与成功不可区分——这是必须一并解决的。

### 1.5 根因收敛

整个差距面收敛于两点,均为上轮设计的**有意延后**(O1/O6),非疏漏:

1. `manager_bridge.rs:47` 的 `ControllerMode::Passthrough` 硬编码 —— 独自关闭了 managed 端点、graceful 切换、无 controller 配置启动等一整类能力;
2. `contract.rs` 七操作封闭集 —— 缺 apply/switch/check/recover 四个操作与状态载荷的丰富化。

---

## 2. 能力对等矩阵(摘要)

完整 36 行矩阵见调查原文;按类别归并:

| 类别 | 直接使用 manager | 经 service IPC | 差距定级 |
|---|---|---|---|
| 生命周期 start/stop/restart | ✅ | ✅(restart 恒 hard) | 可接受 |
| 运行时改配置(patch/reload/同epoch重启+回滚/graceful切换) | ✅ `apply_config`/`switch` | ❌ 只能重启 | **P0** |
| 乐观并发(RevisionId) | ✅ | ❌ | **P0**(随 apply) |
| 观测:controller 端点 | ✅ host | ❌ | **P0**(O6 前置) |
| 观测:revision(epoch/generation/双哈希/runtime_path) | ✅ | ❌ | **P0**(一致性确认的载体) |
| 观测:health(状态/连败/最近错误) | ✅ | ❌(status.rs:38 留有 TODO) | **P1** |
| 观测:epoch/pid/attempt/六态 | ✅ | ❌(压成二值) | **P1** |
| 观测:apply/switch 结果(RolledBack 等) | ✅ | ❌ | **P0**(随 apply) |
| check_config 干跑 | ✅ | ❌ | P1 |
| recover_quarantine | ✅ | ❌(只能重启服务) | **P1**(运维死锁) |
| 结构化错误(23 变体) | ✅ | ❌(压成字符串) | P2 |
| 结构化 LogFrame | ✅ | 部分(压进 TraceLog.fields) | P2 |
| 观测:生效 config 全文 | ❌(manager 也只给哈希+路径) | ❌ | 不做(见 §3) |

## 3. 对等的边界:明确不暴露清单

"对等"定义为**运行时操作 + 观测**对等。以下属嵌入期/进程内策略,不应过 IPC(避免过度设计):

- probe 注入(`CoreManagerBuilder`)、`InstanceOptions`(超时/重启策略/退避)、`LocalIpcPolicy` 每请求定制 —— 服务级启动配置即可;
- `Instance`/`InstanceBuilder` 直接监督、`RuntimeConfigStore` 直接读写 —— 服务私有;
- **secret 不上 wire**:两种模式下 secret 都源自调用方 config(manager 从不生成),调用方本来就知道;而 IPC 的 HTTP 层无认证、socket ACL 是唯一门槛(`nyanpasu_ipc/src/server/mod.rs:106-128`),暴露 secret 只增风险无收益。若未来 Managed 模式自动生成 secret,再作为独立决策(带权限设计)重提;
- 生效 config **全文**不暴露:`Active.effective_document` 连 manager 都是私有;哈希 + 结果枚举足以支撑一致性确认。

## 4. 改进方案

### P0-A:状态暴露增强(纯加字段,wire 兼容)

`CoreInfos` 新增字段,全部 `Option` + `#[serde(default)]`(旧 GUI serde 默认忽略未知字段,旧 golden 样例继续通过):

```rust
pub struct CoreInfos {
    pub r#type: Option<CoreType>,
    pub state: CoreState,                      // wire 二值枚举不动(兼容)
    pub state_changed_at: i64,
    pub config_path: Option<PathBuf>,          // 语义不变:源路径
    // ---- 新增 ----
    pub controller: Option<CoreControllerInfo>, // NamedPipe(PathBuf)|UnixSocket(PathBuf)|Http(String)
    pub health: Option<CoreHealthInfo>,         // state/changed_at/consecutive_failures/last_error
    pub revision: Option<ConfigRevisionInfo>,   // epoch/generation/source_hash/effective_hash
    pub detail: Option<CoreStateDetail>,        // 六态全息:epoch/pid/attempt/from/to
}
```

- `CoreControllerInfo` 定义在 `nyanpasu_ipc::api`,**不直接复用** `clash_api::Host`(clash-api 目前只是 core-manager 的内部依赖,不让它泄漏进 wire 依赖树);
- `runtime_path` 不进 wire:客户端因 0o700/ACL 读不了,暴露只会误导;一致性确认靠哈希(见 P0-C);
- 顺带补齐 specta 派生缺口(现状仅 status 类型有 TS 绑定,请求载荷与事件全部没有 —— `api/core/start.rs`、`api/ws/events.rs` 等)。

### P0-B:`/core/apply` 端点(update_config 的正式形态)

```rust
pub struct CoreApplyReq<'n> {
    pub core_type: Cow<'n, CoreType>,
    pub config_file: Cow<'n, PathBuf>,
    pub expected_revision: Option<RevisionIdInfo>,  // 乐观并发,None = 不检查
}
pub struct CoreApplyData {
    pub outcome: ApplyOutcomeKind,   // noop|patched|reloaded|restarted|switched|rolled_back
    pub revision: ConfigRevisionInfo,
    pub warning: Option<String>,     // DurabilityUncertain 等降级信息,结构化保留
    pub failed_apply: Option<String>,// rolled_back 时:失败原因
}
```

- 直通 `manager.apply_config`;`process_spec` 变化(含换核)manager 内部自动路由到 switch,**无需独立 `/core/switch` 端点**(O1 以此方式收敛);
- 语义定界:核心未启动时报错(保持 `start` 的显式性,不做隐式启动);`start` 在运行中仍报错(旧语义不动);
- `RevisionConflict` 等错误:**不动** `ResponseCode` 枚举(旧客户端对未知变体会解码失败),在 envelope `R` 上新增 `error_kind: Option<String>` 字段(additive-safe)或约定 msg 前缀;倾向前者;
- 适配器补上设计稿预留的 `last_revision` 字段,`apply` 成功后更新,`expected_revision` 缺省时可选地以其兜底(细节实施时定);
- CLI 同步:`rpc apply-config --core-type mihomo --config <path> [--expected-revision <id>]`。

### P0-C:一致性确认闭环(回答"最终 config 应与修改一致")

一致性语义定义:**canonical 语义等价**,以 `effective_hash` 为身份;字节级差异(排序/注释/锚点)是 canonicalize 的设计使然,不视为漂移。

调用方确认协议:

1. `apply` 响应携带 `revision.source_hash` —— 调用方持有源文件,service 回显它算出的源哈希,调用方对照即知"service 读到的就是我写的那份"(无需在客户端复刻 FNV-1a 实现);
2. `outcome` 显式区分 `rolled_back`(此时核心运行**旧**配置,revision 为旧值)—— 消除"回滚被当成成功"的静默错误;
3. `/status` 携带同一 `revision` —— 任意时刻可复核;`CoreStateChanged` v2 载荷同(见 P1-A),推送即自带一致性凭据;
4. 漂移检测:调用方改盘上源文件未 apply → 本地文件 mtime/内容 与 status.revision.source_hash 不符,GUI 可提示"配置已修改未生效"。

既有行为注明:canonicalize 拒绝 YAML tag 与非字符串键,比 mihomo 本身更严格(`config/mod.rs:166-171,186-188`)——`/core/check`(P1-B)可让 GUI 在保存前预检。

### P1-A:事件推送增强

- 新增变体 `Event::CoreStatusChanged(CoreStatusPayload)`,载荷 = P0-A 的完整快照(六态 detail + controller + revision + health)。**推送即快照**,对齐 manager 的 watch 语义;
- 兼容策略:已确认旧 client 库遇未知变体仅单条 `Err(Decode)`、流不断;但 GUI 消费侧行为未验证 → 采用 **ws 版本协商**(`/ws/events?v=2`;无参数 = v1 只发旧两变体),一个过渡版本后 GUI 升级完毕再默认 v2。旧 `CoreStateChanged` 在 v2 下仍双发一版,给脚本类消费者缓冲;
  - **2026-07-31 修订（owner 指令，S11 实施）**：ws 版本协商**已撤销**。服务二进制与 GUI 同版本分发，不存在独立/第三方消费者，`?v=2` 这一层协商是没有受益方的复杂度。现状：`/ws/events` 只有一种协议——全量事件集（含 `CoreStatusChanged`）、连接即快照、`Lagged` 后补发快照，查询串一律忽略、不解析、不拒绝；`EVENT_VERSION_PARAM`、`EVENT_VERSION_V2`、`Event::is_protocol_v1()`、`Client::events_v2()` 与服务端 `EventProtocol` 过滤器全部删除，`events()` 即快照流（client API 破坏性变更）。旧 `CoreStateChanged` 的**双发保留不变**——GUI 仍在消费，本次只删协商层，不动变体。
- **连接即快照**(snapshot-on-connect):ws 建立后先推一帧当前 `CoreStatusChanged`,消除"断线重连后必须轮询 /status"的重同步竞态,`Lagged` 后 `resubscribe()` 同样补发一帧;
- 顺带修正语义缺陷:v2 载荷中 `Starting/Restarting` 不再伪装成 `Stopped(None)`(旧 v1 映射不动,保持兼容)。

### P1-B:配套运维端点

- `POST /core/check`:直通 `check_config` 干跑(GUI"验证配置"按钮;当前校验只能靠真启动);
- `POST /core/recover`:直通 `recover_quarantine`,解除隔离死锁(当前唯一解法是重启整个服务)。与其他操作同 socket ACL,无新增权限面。

### P2:移除 `ControllerMode`,`LocalIpcPolicy` 成为唯一旋钮(O6 收敛为一次性移除)

2026-07-30 追加复核后的决策:不做"Managed 参数化 + 日后默认化翻转"的过渡态,直接删除 `ControllerMode` 枚举。依据:Managed 自带的 `LocalIpcPolicy` 三档(`spec.rs:62-73`)已覆盖 Passthrough 绝大部分场景——`Prefer` 承接不支持本地 IPC 的老内核(回落源 config 的 HTTP,`capability.rs:169-172`),`Disable` 承接 HTTP 面板直连诉求(对**仅依赖 HTTP `external-controller`** 的 config 与今日 Passthrough 逐字节等价;若 config 同时声明了本地键,Passthrough 会优先取本地键而 Disable 直接忽略之——见"代价(已接受)")。Passthrough 独有语义仅剩"尊重用户自声明的 `external-controller-pipe`/`-unix`"(完整 `inspect` `clash.rs:39-51` vs `inspect_http` `clash.rs:53-58`),属小众用法,决定放弃。

- **core-manager 侧**:删除 `ControllerMode` 枚举,`local_ipc_policy` / `derived_dir` / `controller_template` 平铺进 `ManagerOptions`;随之删除 `prepare` 的 mode match(`config/mod.rs:125-139`)、`DegradeReason::PassthroughMode` 臂(`switching.rs:33-35`)、`runtime_dir`/`derived_dir` 双目录兼容逻辑(`spec.rs:97-99`);graceful 门槛简化为 local-ipc + kind + overlap 三条件;测试矩阵少一个模式维度。
- **service 侧**:`local_ipc_policy` 从服务启动参数/配置注入(`install` 长参 + `NYANPASU_*` env,沿用 S5 模式);**过渡默认 `Disable`**——对声明了 `external-controller` 的现有 GUI config 行为不变,过渡态由 policy 天然承载,不需要保留枚举做"日后翻转";GUI 完成端点发现适配(消费 P0-A 的 `controller` 字段)后,默认切 `Prefer`。
- **收益**:服务端永久摆脱"restart 恒 hard switch"降级路径——policy 命中 LocalIpc 后,`restart()` / `switch()`(即 `POST /core/restart`)走 graceful 零停机切换;config 不再强制声明 `external-controller`(`ControllerMissing` 仅剩 Disable / Prefer 回落且无 HTTP 键一种情形);控制通道默认收敛到 0o700 目录下的 pipe/socket,不再默认暴露 localhost TCP。
  - **2026-07-30 勘误(S10 实施后复核)**:本行原称"P0-B 的 `/core/apply` 无需任何改动即自动享受 graceful 零停机切换",与代码不符。`apply_config` 的 switch 类走 `switch_with_compensation`(旧 epoch 回滚的硬 stop→start),从不调用 `graceful_switch`,policy 命中 LocalIpc 对它没有影响。让 apply 复用 graceful 需要调和两套互不兼容的失败补偿,属独立变更(S10 计划 D6)。
- **代价(已接受)**:用户自声明 pipe/unix 路径不再支持;LocalIpc 命中时 HTTP `external-controller` 被无条件移除(`clash.rs:61-70`,graceful 的 epoch 隔离所需)——第三方面板(yacd/metacubexd)直连需显式 `Disable`,以放弃 graceful 为代价。
- **约束解除**:上轮设计"crates/* 只消费不改动"的约束到此阶段正式解除;`ManagerOptions` 是公开 API,GUI 若有进程内复用计划需同版本协调。

### P3(可选,不阻塞):错误与日志结构化

- envelope 增 `error_kind`(见 P0-B)逐步映射 manager 的 23 个错误变体;
- 新变体 `Event::CoreLog(LogFrameInfo)` 结构直通(epoch/stream/level/target/fields/truncated),替代现在压进 `TraceLog.fields` 的降级形态。

## 5. 兼容与测试策略

- **wire 闸门**:所有新字段 Option+default;`wire_golden.rs` 旧样例**逐字节不变**必须继续通过(向后兼容回归闸),新字段用新样例覆盖;
- 契约测试随 op 清单增长(`routing/tests.rs:204-225` 模式);client shortcuts 一行别名(`call::<Op>` 泛型已使新 op 零成本可达);
- 事件:roundtrip 测试按 v1/v2 参数化;`Event` 新变体的 golden 快照;
- route 级测试用既有 fake_core 基建(core-manager tests/helpers)覆盖 apply 的 noop/patched/reloaded/restarted/rolled_back 各臂;
- 与 GUI 协调:P0-A/P1-A 打包为 **"IPC v2"批次**与 GUI 同版本对齐(与 CLI v2 / O4 同一模式);P0-B/P1-B 是纯新增端点,可先行合并。

## 6. 阶段划分建议

| 阶段 | 内容 | 依赖 | 与 GUI 协调 |
|---|---|---|---|
| S7 | P0-A 状态字段 + P0-C 一致性凭据 + specta 补齐 | 无 | 纯加字段,可先行 |
| S8 | P0-B `/core/apply` + P1-B check/recover + `last_revision` | S7(revision 类型) | 纯新增端点,可先行 |
| S9 | P1-A 事件 v2 + snapshot-on-connect | S7 | ⚠️ ws 版本协商,IPC v2 批次 |
| S10 | P2 移除 `ControllerMode` + policy 注入(过渡默认 `Disable`)→ GUI 端点发现 → 默认切 `Prefer` | S7(controller 字段)、S8(graceful 收益兑现) | ⚠️ GUI 适配端点发现;core-manager 公开 API 变更 |

每阶段独立可合并;S7+S8 落地后,"service client ≈ core manager"的能力对等即达成 §2 矩阵中全部 P0 行。

## 7. 风险与开放问题

| # | 问题 | 处置建议 |
|---|---|---|
| R1 | GUI 消费侧对单条事件解码错误的容错未验证 | S9 前在 GUI 仓实测；ws 版本协商本身已兜底。**2026-07-31（S11）**：协商已撤销，该兜底不复存在——服务与 GUI 同版本分发，兼容性改由发布协调保证 |
| R2 | `apply` 在核心停止态的语义(报错 vs 隐式 start) | 倾向报错,保持 start 显式;实施时与 GUI 确认交互预期 |
| R3 | envelope 加 `error_kind` 字段对第三方脚本消费者的影响 | serde 默认忽略未知字段;wire golden 会暴露任何意外 |
| R4 | Managed 模式下若未来自动生成 secret | 独立决策,需分发与权限设计;本轮明确不做 |
| R5 | `expected_revision` 缺省语义(不检查 vs 以 last_revision 兜底) | 实施时定;倾向"缺省不检查",兜底逻辑放 GUI |
| R6 | 存量用法:用户自声明 pipe/unix controller;第三方面板依赖 `external-controller` 直连 | 前者随 S10 放弃(小众,发布说明标注);后者文档化"显式 `Disable`",GUI 侧可评估面板流量代理作为后续增强 |
| R7 | S10 删除 `ControllerMode` 是 core-manager 公开 API 破坏性变更 | "crates/* 不改动"约束自 S10 起解除;与 GUI 进程内复用(如有)同版本协调 |
| R8 | `/core/start`、`/core/apply`、`/core/check` 均接受调用方任意路径,组内成员可指向任何 root 可读文件 | **接受并记录**:socket ACL 是本服务唯一的授权边界,自 `/core/start` 起一贯如此,本轮 S8 新端点未扩大该面。若日后要收窄,应做成显式的路径白名单策略,而不是在各端点里零散校验 |
| R9 | 事件流为单一协议，快照帧比旧的两变体更密，且与日志共用 256 槽广播环（`server/events.rs:6`），启动抖动期可能略早触发 `Lagged` | **接受并记录**：稳态下事件稀疏；服务端日志记录 skipped 条数（该告警被过滤，不入 EventHub）。**2026-07-31 更新（S11）**：协商撤销后不再有连接类别之分——任何连接 `resubscribe()` 之后一律补发一帧快照，"跳到实时尾部、需自行轮询 `/status` 重新同步"的情形已消失（`events.rs` 的 `lag_recovery_with_feedback_reaches_the_tail` 已固定该路径）。若实测有压力，提高容量比拆分通道便宜 |
