# nyanpasu-core-manager 设计文档

- 日期:2026-07-18
- 状态:已实施(branch feat/core-manager,M1-M4 全部落地;P4 service 接线待启动)
- 范围:`crates/nyanpasu-core-manager` 实装(前设计 P3),附 `nyanpasu_service` 消费契约;service 实际接线(P4)与 `nyanpasu-utils::core` 弃用另行实施
- 前置:docs/superpowers/specs/2026-07-16-nyanpasu-utils-process-module-design.md(P1/P2 已完成,本设计构建于 `nyanpasu-utils::process` 之上)

---

## 1. 背景与需求

`nyanpasu-utils::process`(Command / ProcessHandle / Supervisor)已落地,`crates/nyanpasu-core-manager` 目前是骨架 crate。本设计完成前设计 P3 阶段,并纳入新需求:

1. **CoreManager 管理 Instance**:Instance 维护 core 进程,对非预期状态尝试恢复。
2. **启动确认**:`start` 必须通过 `clash_api::Client::version()` 请求成功才判定内核启动成功;超时则杀死进程、返回失败。
3. **epoch**:每个 Instance 对应一个单调递增的 epoch,作为平滑重启/切换的基础;pipe/unix socket 控制器路径携带 epoch。
4. **平滑切换**:配置更新需要重启内核时,在旧核存活期间以"端口置零"的派生配置启动新核,新核健康后停旧核,随即 PATCH 恢复监听——把秒级的启动耗时移出停机窗口。
5. **状态变化通知**。

### 已确认的决策(评审问答记录)

| 决策点 | 结论 |
|---|---|
| 设计范围 | P3 为主 + service 消费契约;P4 接线另行 |
| 分层模型 | Instance = 单 epoch(配置不可变) + 内部 Supervisor crash 自恢复;换配置/换内核由 CoreManager 创建新 epoch 的新 Instance |
| 健康检查层 | core-manager 层自探(Supervisor 的 `ReadinessProbe::AliveAfter` 保持不动,仅影响重启预算);不下沉 clash_api 到 utils |
| 健康检查抽象 | 不抽 trait,内部结构体;测试走假 /version 服务器 |
| 平滑切换 | 设计全量,分阶段实施(硬切换先行,平滑切换为独立末段) |
| 通知机制 | `tokio::sync::watch` 状态快照流,单通道 |
| 内核建模 | kind 与元数据分离(alpha 变体降为 binary/metadata 差异);service 边界做旧 IPC `CoreType` 映射,wire 不变 |
| 并发模型 | 混合:控制面 async Mutex 串行 + 每 Instance 一个监视 task + watch 发布 |
| 无 controller 配置 | `start` 严格报错(不降级为存活即成功) |
| Meow | 与 Mihomo 共用 launch profile(`-m -d -f`,meow 兼容 mihomo CLI) |

## 2. 目标与非目标

### 2.1 目标

- G1:Instance 状态机(Starting / Running / Restarting / Stopping / Stopped)与 crash 自恢复,构建在 `process::Supervisor` 之上。
- G2:启动确认 = version 探活成功;`startup_timeout` 为总限时,超时杀进程树并携带 stderr 摘要报错。
- G3:CoreManager 编排:start / stop / restart / switch,epoch 分配,watch 状态发布。
- G4:平滑切换(Managed 模式 + mihomo 系),含降级矩阵与失败兜底;硬切换全内核可用。
- G5:迁入 `nyanpasu-utils::core` 域逻辑(启动参数 profile、`check_config`、`parse_check_output`),消化前设计 P3。
- G6:定义 service 消费契约(IPC 状态映射、类型回显),使 P4 只做接线。

### 2.2 非目标

- 不做 service 实际接线与路由改造(P4)。
- 不弃用/删除 `nyanpasu-utils::core`(随 P4)。
- 不演进 IPC wire 协议(丰富状态暴露属协议演进)。
- 不支持多份并行运行的业务实例(manager 只有一个 current;切换窗口的新旧并存是内部瞬态)。
- 不建模完整 mihomo 配置 schema(只动明确列出的键)。

## 3. 架构

```text
nyanpasu_service (P4, 仅契约)
   └─ CoreManager          ← 编排:epoch 分配、start/stop/restart/switch、状态聚合发布(watch)
        └─ Instance        ← 单 epoch 生命周期:Supervisor + 监视 task + 健康探测 + 状态机
             ├─ nyanpasu-utils::process::Supervisor   (crash 自恢复、退避、进程树清理)
             └─ clash-api::Client                     (version 探活、PATCH /configs)
```

### 3.1 crate 布局

```text
crates/nyanpasu-core-manager/src/
├── lib.rs        # 公开出口
├── kind.rs       # CoreKind + 启动参数 profile + check_config(迁自 utils::core,含 parse_check_output)
├── state.rs      # InstanceState / CoreState / StopReason / CoreStatus 快照
├── spec.rs       # CoreSpec / InstanceSpec / InstanceOptions / ManagerOptions
├── config.rs     # 运行时配置内省(提取 controller/secret)与派生改写(M4)
├── health.rs     # 内部 HealthCheck:轮询 version() 直到成功/超时
├── instance.rs   # Instance(重写现骨架)
├── manager.rs    # CoreManager
└── error.rs      # thiserror 错误
```

新增依赖:`serde_yaml_ng`(或同等维护中的 serde-yaml fork,实施 M1 时定一个)、`tokio`、`tokio-util`、`thiserror`、`tracing`、`parking_lot`;`clash-api`、`nyanpasu-utils`(feature `process`)、`camino` 已有。

## 4. 核心类型

### 4.1 kind 与规格(kind.rs / spec.rs)

```rust
#[non_exhaustive]
pub enum CoreKind { Mihomo, ClashPremium, ClashRs, Meow }
```

- 启动参数 profile 按 kind 分派(迁自旧 `get_run_args`):
  - `Mihomo`:`-m -d <working_dir> -f <config>`
  - `ClashRs`:`-d <working_dir> -c <config>`
  - `ClashPremium`:`-d <working_dir> -f <config>`
  - `Meow`:与 `Mihomo` 相同(meow 兼容 mihomo CLI)
- alpha 变体不是独立 kind:`kind = Mihomo` + 不同 `binary_path` + `CoreSpec.version/features` 元数据。
- `check_config`:迁移旧 `CoreInstance::check_config_`,用 `process::Command::output()` 一次性执行(`mihomo -t` 等),mihomo 系错误输出经 `parse_check_output` 提炼。

```rust
pub struct CoreSpec {
    pub kind: CoreKind,
    pub binary_path: Utf8PathBuf,      // 路径解析(find_binary_path)留在 service 层
    pub version: Option<String>,       // 展示用元数据,调用方提供
    pub features: Vec<String>,
}

pub struct InstanceSpec {              // 不可变;换配置 = 新 epoch 新 Instance
    pub core: CoreSpec,
    pub config_path: Utf8PathBuf,
    pub working_dir: Utf8PathBuf,
    pub pid_file: Option<Utf8PathBuf>,
    pub options: InstanceOptions,
}

pub struct InstanceOptions {
    pub startup_timeout: Duration,     // 默认 30s:探活总限时
    pub probe_interval: Duration,      // 默认 250ms:version 轮询间隔
    pub restart_policy: RestartPolicy, // 透传 Supervisor,默认 OnFailure { max_restarts: 5 }
    pub backoff: Backoff,              // 透传 Supervisor,默认指数 1s→30s + 抖动
}
```

### 4.2 控制器模式(config.rs / spec.rs)

version 探活需要 controller 端点 + secret,而它们只存在于运行时配置 YAML 内(旧 service 从不访问内核 API)。因此 manager 具备配置内省能力(YAML 依赖自 M1 引入):

```rust
pub enum ControllerMode {
    /// 默认:配置原样启动,探活端点从配置提取
    /// (优先 external-controller-pipe / external-controller-unix,其次 external-controller HTTP;secret 键同读)。
    /// 配置无任何 controller → start 报 Error::ControllerMissing(严格策略)。
    /// 平滑切换不可用(降级硬切换)。
    Passthrough,
    /// manager 生成派生配置:移除 HTTP external-controller,
    /// 注入按 epoch 展开的 external-controller-pipe|unix = template(epoch)。
    /// CoreStatus.controller 对外暴露当前端点(GUI 经 service IPC 发现)。
    /// 平滑切换的前提模式。
    Managed {
        derived_dir: Utf8PathBuf,          // 派生配置/unix socket 存放目录
        controller_template: String,       // 含 "{epoch}" 占位;默认:
                                           //   Windows: \\.\pipe\nyanpasu\core-{epoch}
                                           //   Unix:    <derived_dir>/core-{epoch}.sock
    },
}
```

Managed 模式下普通 start 也走派生(仅改 controller 键);派生文件是运行时产物:manager 启动时按模板清扫遗留(unix socket 残留文件、旧 epoch YAML),实例停止后删除。

### 4.3 状态(state.rs)

```rust
pub enum InstanceState {
    Starting,                    // spawn 后、version 探活未通过
    Running { pid: u32 },        // 探活成功
    Restarting { attempt: u32 }, // crash 后:backoff、重拉起、re-probe 期间
    Stopping,
    Stopped(StopReason),         // 终态
}

pub enum StopReason {
    Finished,        // 干净退出 code=0(Supervisor 不重启)
    User,            // stop() 请求
    Error(String),   // GaveUp / 探活超时 / spawn 失败;含 stderr 尾部摘要
}

pub struct CoreStatus {              // watch 载荷,manager 对外快照
    pub state: CoreState,
    pub changed_at: i64,             // unix ts(毫秒),喂给 IPC state_changed_at
    pub spec: Option<SpecSummary>,   // kind、config_path,喂给 CoreInfos
    pub controller: Option<clash_api::Host>, // Managed 模式下当前端点
}

#[non_exhaustive]
pub enum CoreState {
    Stopped { reason: Option<StopReason> },   // 初始值 reason=None
    Starting   { epoch: Epoch },
    Running    { epoch: Epoch, pid: u32 },
    Restarting { epoch: Epoch, attempt: u32 },
    Switching  { from: Option<Epoch>, to: Epoch }, // 切换窗口(平滑/硬共用)
    Stopping   { epoch: Epoch },
}
```

### 4.4 错误(error.rs)

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    AlreadyRunning,
    NotStarted,
    ConfigNotFound(Utf8PathBuf),
    BinaryNotFound(Utf8PathBuf),
    ControllerMissing,                       // Passthrough 且配置无 controller
    UnsupportedCore(CoreKind),
    StartupTimeout { stderr_tail: String },  // 探活超时,进程树已杀
    StartupFailed  { stderr_tail: String },  // 启动窗口内终态失败(GaveUp/秒退)
    Process(#[from] nyanpasu_utils::process::ProcessError),
    Api(#[from] clash_api::Error),
    Yaml(...),                               // 按所选 YAML crate 定
    Io(#[from] std::io::Error),
}
```

## 5. Instance 生命周期

### 5.1 健康探测(health.rs,内部结构体)

- 按 Host + secret 构建 `clash_api::Client`(`configure_reqwest` 设单请求超时 ≈1s,防单个挂起请求吃掉整个 deadline;client 层 `NoRetry`,重试由探测循环自己做)。
- 循环:每 `probe_interval` 调一次 `version()`,成功即健康;超过 deadline 即不健康。

### 5.2 流程(监视 task 驱动)

1. `Instance::spawn(spec, epoch, cancel_token)`:预检(config/binary 存在)→ 构建 `Command`(kind profile、`current_dir=working_dir`、`pid_file`、`hide_window`)→ `Supervisor::spawn`(readiness 保持默认 `AliveAfter(1.5s)`,仅影响重启预算)→ 发布 `Starting`。
2. Supervisor 同步钩子只做转发:`on_event → mpsc`(入监视 task);`on_process_event` → tracing 日志 + stderr 环形缓冲(`Arc<parking_lot::Mutex<VecDeque<String>>>`,≈32 行)。
3. 监视 task 消费事件,驱动状态机与 `watch::Sender<InstanceState>`:
   - `Started{pid}` → 起健康探测(与后续事件 select):成功 → `Running{pid}`;**超时 → cancel Supervisor(杀进程树)→ `Stopped(Error)`**,错误文本含 stderr 尾部,mihomo 系经 `parse_check_output` 提炼。
   - `Restarting{attempt}` → `Restarting{attempt}`。
   - `Exited(payload)` → 记录,等待 Supervisor 后续决定(重启或终止)。
   - `GaveUp` → `Stopped(Error)`。
   - `Stopped`(取消触发)→ 按 user-stop 标记定 `Stopped(User)` 或 `Stopped(Error)`;干净退出(code=0,Supervisor 不重启)→ `Stopped(Finished)`。
4. **初次启动语义**:`CoreManager::start` 订阅 Instance watch,等到 `Running`(成功)或 `Stopped`(失败)才返回。`startup_timeout` 是总限时——期间进程秒崩、Supervisor 在窗口内退避重试均可,deadline 一到整体判定失败、杀树、返回 `StartupTimeout`;窗口内已到终态则返回 `StartupFailed`。与旧"start 失败即报错"行为等价,且错误信息来自实际 stderr。
5. **crash 恢复**:`Running` 后进程崩溃 → Supervisor 退避重启 → 每次 `Started` 后**重新探活**;探活超时或 `GaveUp` → 终态 `Stopped(Error)`。进程级恢复已 exhausted,manager 不再叠加更高层重试,由消费方(GUI/用户)决策。
6. `Instance::stop`:置 user-stop 标记 → 发布 `Stopping` → `supervisor.stop()`(优雅→强杀整树)→ `Stopped(User)`。

### 5.3 公开面

```rust
impl Instance {
    pub async fn spawn(spec: InstanceSpec, epoch: Epoch, controller: ResolvedController,
                       token: CancellationToken) -> Result<Instance, Error>;
    pub fn state(&self) -> watch::Receiver<InstanceState>;
    pub fn epoch(&self) -> Epoch;
    pub fn spec(&self) -> &InstanceSpec;
    pub async fn wait_ready(&self) -> Result<(), Error>;   // 等到 Running 或 Stopped
    pub async fn stop(self) -> Result<(), Error>;
}
```

`ResolvedController` = 探活/控制所用的 `Host` + secret(Passthrough:内省所得;Managed:模板展开所得),由 manager 解析后传入。

## 6. CoreManager

### 6.1 API

```rust
impl CoreManager {
    pub fn new(options: ManagerOptions) -> Self;
    // ManagerOptions { controller_mode, cancel_token, instance 默认 options 覆盖 }

    pub fn subscribe(&self) -> watch::Receiver<CoreStatus>;  // 状态通知
    pub fn status(&self) -> CoreStatus;                      // 当前值

    pub async fn start(&self, spec: InstanceSpec) -> Result<()>;   // 已 Running → AlreadyRunning
    pub async fn stop(&self) -> Result<()>;                        // 保留 last spec
    pub async fn restart(&self) -> Result<()>;                     // = switch(last_spec);从未启动过 → NotStarted
    pub async fn switch(&self, spec: InstanceSpec) -> Result<SwitchOutcome>; // Graceful | Hard(降级原因)
    pub async fn shutdown(&self) -> Result<()>;                    // 服务退出:停实例、清派生文件
    pub async fn check_config(&self, spec: &InstanceSpec) -> Result<()>;     // 一次性 -t 校验
}
```

- 控制面方法由一把 async Mutex 串行化;`switch` 进行中到达的控制命令排队等待。
- **epoch**:控制锁内的单调分配器(`EpochAllocator`)发放 `Epoch(NonZeroU64)`,由控制面 Mutex 串行化;"无"由 `Option<Epoch>` 表达而非 0 哨兵。类型止步于本 crate——文件名、pid 记录、`LogFrame`、DNS 记录与 IPC 仍是裸 `u64`,产物发现读回的数字亦然。进程内单调即满足路径唯一性(named pipe 随 server 关闭消失;unix socket 残留文件由启动清扫处理)。
- 状态聚合:manager 的转发 task 订阅当前 Instance 的 `watch<InstanceState>`,映射为 `CoreState`(补 epoch)写入对外 watch;切换流程期间由流程直接发布 `Switching`。
- `stop` 后 manager 保留 last spec(旧实现同语义:stop 后可 restart)。

### 6.2 平滑切换流程(Managed + mihomo 系)

```text
switch(spec_B):
 1. epoch_new = next()
 2. 派生配置 B':
      - 监听键置零/移除:port / socks-port / redir-port / tproxy-port / mixed-port
      - tun.enable = false(若原配置启用)
      - 移除 external-controller(HTTP),注入 external-controller-pipe|unix = template(epoch_new)
      - 写入 derived_dir/epoch-{N}.yaml
 3. Instance::spawn(B', epoch_new) → wait_ready(startup_timeout)
      失败 → 杀新实例,current 原样保留,switch 返回错误   ← 安全回滚点,旧核零影响
 4. current.stop()                                          ← 监听端口自此释放
 5. 对新核 PATCH /configs 恢复原始监听值(含 tun enable)
      失败 → 有界重试(3 次,间隔 500ms);仍失败 → 兜底:旧核已死、端口已空,
             用完整配置 B 硬重启新实例(同 epoch),保证收敛到正确监听状态
 6. current = 新 Instance;发布 Running{epoch_new};清理旧 epoch 派生文件
```

- **停机窗口如实声明**:步骤 4→5 之间监听端口短暂下线(毫秒级)。这是"最小停机"而非零停机——端口交接的固有代价;内核启动/配置装载的秒级耗时被完全移出停机窗口。
- 探活/PATCH 客户端始终指向**带 epoch 的端点**,不可能误连旧核——epoch 进 IPC 路径除避免冲突外的正确性意义。
- 硬切换 = `current.stop()` → `start(spec_B)`,epoch 照样递增,共用 `Switching` 状态。

### 6.3 降级矩阵(自动选择,`SwitchOutcome` 注明实际方式与降级原因)

| 条件 | 方式 |
|---|---|
| Managed 模式 + kind=Mihomo | 平滑切换 |
| Passthrough 模式(配置自带 HTTP controller) | 硬切换 |
| kind=ClashRs / ClashPremium(运行时 PATCH 能力存疑/无) | 硬切换 |
| 配置含 `dns.listen`(PATCH /configs 不覆盖 DNS 监听) | 硬切换(保守;验证 mihomo 行为后可放宽,见 O2) |
| 当前 Stopped | switch 等价于 start |

### 6.4 配置改写边界

- 解析为 untyped `Mapping`,只动 §6.2 明确列出的键,不建模完整配置。
- 已知损耗:YAML round-trip 丢注释/锚点——运行时配置由 GUI 机器生成,可接受。

## 7. service 消费契约(P4 只做接线)

新 `CoreManagerService` = 薄包装:持有 `CoreManager` + 桥接 task(订阅 watch → 映射 → `Event::new_core_state_changed` 推 ws;仅在映射后的 IPC 状态变化时转发,避免重复事件)。

丰富状态 → 现有 IPC `CoreState`(wire 不变)的有损映射:

| manager `CoreState` | IPC `CoreState` |
|---|---|
| `Running{..}` | `Running` |
| `Starting` / `Restarting{..}` | `Stopped(None)`(与旧行为等价:确认存活前不算 Running) |
| `Switching` / `Stopping` | `Running`(瞬态,交接中) |
| `Stopped(reason)` | `Stopped(Some(reason.to_string()))`;初始态 `Stopped(None)` |

- `CoreInfos.state_changed_at` ← `CoreStatus.changed_at`;`config_path` ← `SpecSummary`。
- `CoreInfos.r#type`(wire 类型 `nyanpasu_utils::core::CoreType`):service 记住请求时的旧 `CoreType` 原样回显;`CoreType → CoreKind + binary 名`的单向映射函数在 service 侧(沿用 `find_binary_path`),manager 不感知 alpha。
- IPC 暴露丰富状态(epoch/attempt)属协议演进,不在本 spec。

## 8. 测试策略

核心设施:**`fake_core` 测试辅助二进制**(沿用 process 模块 `tests/helpers/test_child.rs` 先例)——读取配置 YAML,绑定其中声明的 controller 端点(pipe/unix/http 三传输,axum 实现,工作区已有依赖与 nyanpasu-ipc 的本地传输服务经验),提供 `GET /version`、`PATCH /configs`(收到的 patch 落盘供断言),脚本行为:延迟就绪、永不就绪、启动即退出、就绪后 N 秒崩溃。

| 层 | 覆盖 |
|---|---|
| 单元 | kind 启动参数 profile;controller/secret 提取(三种键);派生改写(端口置零、controller 注入、无关键原样保留);IPC 状态映射表;epoch 分配 |
| 组件(Instance) | 探活成功→Running;存活但永不就绪→超时杀树(断言进程消失);坏配置秒退→错误含 stderr 摘要;崩溃→Restarting→re-probe→Running;预算耗尽→Stopped(Error);stop→Stopped(User) 不重启 |
| 组件(CoreManager) | 硬切换:epoch 递增、旧 pid 死、新 Running;AlreadyRunning/NotStarted 语义;shutdown 清理 |
| 组件(平滑切换) | 新旧并存窗口成立(旧核存活时新核已探活);PATCH 载荷=原始端口值;新核启动失败→旧核零扰动回滚;PATCH 拒绝→硬重启兜底收敛 |
| 等价性 | 对照旧 `CoreInstance`:start/crash/stop 对外可见序列等价(前设计 P3 验收要求) |

## 9. 实施阶段(每段独立可合并)

| 阶段 | 内容 | 验收 |
|---|---|---|
| M1 | 基础迁移:`kind.rs`(profile + check_config + parse_check_output)、`error/state/spec.rs`、`config.rs` 内省(提取 controller/secret) | 单元测试绿;`utils::core` 原样不动 |
| M2 | `Instance`:Supervisor 集成、监视 task、`health.rs`、watch;`fake_core` 设施 | Instance 组件测试全绿 |
| M3 | `CoreManager`:编排、epoch、硬切换(所有降级路径先可用)、状态聚合、service 契约类型 | Manager 组件测试 + 等价性测试绿 |
| M4 | 平滑切换:Managed 模式、配置派生、平滑路径、PATCH 兜底、清扫 | 平滑切换组件测试绿 |
| P4(另行) | service 接线、`utils::core` 标 deprecated、删旧 `recover_core` | 端到端 |

## 10. 风险与开放问题

| # | 问题 | 影响 | 处置 |
|---|------|------|------|
| O1 | clash-rs / clash-premium 对 `PATCH /configs` 的支持度未核验 | 平滑切换范围 | 降级矩阵已保守限定 mihomo 系;M4 实施时按官方文档/实测放宽 |
| O2 | mihomo `PATCH /configs` 是否覆盖 `dns.listen` 未核验;含 DNS 监听的配置降级硬切换 | DNS 用户享受不到平滑切换 | M4 实测后决定放宽或维持 |
| O3 | mihomo 首启远程 provider 拉取可能拖慢 API 就绪 | startup_timeout 误杀慢启动 | 默认 30s 较宽裕且可配;错误信息含 stderr 便于定位 |
| O4 | Managed 模式改变 GUI 访问内核 API 的方式(经 service 发现 pipe 端点,而非固定 HTTP 端口) | 上游 GUI 需跟进适配 | Passthrough 为默认;Managed 是可选启用的前提模式,GUI 适配属上游范围 |
| O5 | YAML fork 选型(serde_yaml 已停维护) | 依赖质量 | M1 时在 serde_yaml_ng 等维护中 fork 里定一个,接口面窄(untyped Mapping),可替换 |
