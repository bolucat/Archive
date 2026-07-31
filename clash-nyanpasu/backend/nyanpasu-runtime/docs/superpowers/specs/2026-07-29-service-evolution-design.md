# nyanpasu-service 演进设计:core-manager 接线、可测试化与免 UAC 自更新

- 日期:2026-07-29
- 状态:提案(待评审)
- 范围:`nyanpasu_service` / `nyanpasu_ipc` 的架构演进;`crates/*` 只消费不改动(自更新签名校验可能新增独立 crate)
- 前置:docs/superpowers/specs/2026-07-18-nyanpasu-core-manager-design.md(M1–M4 已合入 main,P4 接线即本设计 S1)

---

## 1. 背景与现状评估

### 1.1 测试覆盖矩阵(现状)

| crate | 测试现状 | 说明 |
|---|---|---|
| `crates/nyanpasu-utils` | ✅ 完善 | process 模块单测 + 8 个集成测试文件 |
| `crates/nyanpasu-core-manager` | ✅ 完善 | 141 测试:单元 + fake_core 组件测试 + 真核集成套件 |
| `crates/clash-api` | ✅ 完善 | 单测 + rest/pipe/stream 集成测试 |
| `crates/nyanpasu-core-metadata` | ✅ 有 | feature 表单测 |
| `nyanpasu_ipc` | ⚠️ 少量 | roundtrip 集成测试 + 2 处嵌入单测 |
| `nyanpasu_service` | ❌ 几乎为零 | 仅 `utils/service.rs` 的 macOS launchctl 参数单测 |

结论:测试债务集中在 `nyanpasu_service` 本体。原因不是"没写测试",而是**结构上不可测**——见 1.2。

### 1.2 问题清单

**P1:旧 `server/instance.rs` 的锁复杂度(用户痛点)**
- `Arc<Mutex<Option<CoreManager>>>` 三层嵌套;`start` 持锁跨 await 组装实例;
- 取消任务里 `try_lock().is_ok()` 探测再 `stop()`(竞态窗口);
- `recover_core` 递归重试 + 克隆 self 塞进 spawn;`handle_command_event` 8 个参数。
- 这些逻辑与 `crates/nyanpasu-core-manager` 完全重复,且后者已带 141 个测试。

**P2:全局态阻断单元测试**
- `server::consts::RuntimeInfos`(OnceLock,`server` 命令入口 set,路由/`find_binary_path` 处 get);
- `Logger::global()`(OnceLock 单例 + 全局订阅回调);
- `cmds::server::SHUTDOWN_TOKEN`(OnceLock,win_service 与 ctrlc 共用)。
- 后果:任何 route handler 测试都要求进程级初始化,且一个进程只能有一种配置 → 无法并行、无法多用例。

**P3:ws 事件层缺陷(`server/routing/ws.rs`)**
- **socket id 碰撞 bug**:`len() + 1` 分配 id——{1,2} 在线,1 断开后 len=1,新连接得到 id 2,`insert` 顶掉在线订阅者 2(其 rx 被 drop,连接被静默踢掉);
- 广播 `join_all` 等所有订阅者 `send().await`:单个慢客户端塞满 100 缓冲即阻塞全部广播(队头阻塞);
- 日志桥接每行 `spawn` 一个 task,顺序无保证。

**P4:IPC 契约三处漂移面**
- endpoint 常量 + Req/Res 类型(`nyanpasu_ipc::api::*`)、client 手写方法(`client/shortcuts.rs`)、server 手动挂载(`server/routing/*`)三处独立维护,无编译期/测试期一致性保证。

**P5:CLI 人体工学**
- `-V` 是 verbose(违反惯例,`-V` 通常是 version);`--version` 手动实现且 `print_version` 内 `process::exit(0)`;
- 无子命令裸调用输出 `"No command specified"` 而非 help;
- `rpc start-core --core-type` 走 simd_json 解析,用户必须输入带引号的 JSON 字符串(如 `'"mihomo"'`)而非 `mihomo`;
- `install` 四个必填长参数,无 env 回退;无 shell 补全。

**P6:更新流程每次拉 UAC**
- `update` 必须在提权进程里跑(复制自身覆盖服务二进制),GUI 每次升级都弹 UAC/sudo;
- 更新本质是"复制 + 重启",没有签名校验——提权侧从**用户可写路径**取二进制覆盖服务目录(当前由 UAC 时的用户确认背书)。

## 2. 目标与非目标

### 2.1 目标

- G1:`nyanpasu_service` 接线到 `nyanpasu-core-manager`,删除 `server/instance.rs` 及重复的进程管理逻辑(旧 P4)。
- G2:并发模型收敛——路由层零显式锁,事件层修复 P3 三缺陷。
- G3:bin/lib 拆分 + 去全局化,使 route/控制面逻辑可用普通 `#[tokio::test]` 覆盖。
- G4:IPC 契约单一事实来源,server/client 共享,契约测试兜底,wire 不变。
- G5:CLI 人体工学修缮(兼容性变更单独标注、与 GUI 协调)。
- G6:免 UAC 自更新:服务端验签 + 原子换装 + 健康回滚;UAC 只剩首次 install/uninstall。
- G7:`nyanpasu_service`/`nyanpasu_ipc` 建立覆盖率基线并入 CI。

### 2.2 非目标

- 不演进 IPC wire 协议语义(丰富状态暴露、switch 专用端点等属后续协议演进;本设计只做映射与兼容)。
- 不改 `crates/*` 四个基础 crate 的对外 API(消费即可;发现缺口单独提)。
- 不做 GUI 侧适配(Managed 模式端点发现、CLI 参数变更同步属上游范围,本文只标注协调点)。
- 不引入 actor 框架(actix/ractor 等)——见 §5 评估。

## 3. 目标 crate 拓扑

```text
nyanpasu-service        (bin:clap 解析、dispatch、win_service 入口、exit code)
   └─ nyanpasu-service-runtime   (lib:一切逻辑,可测)
        ├─ server/       axum 路由、AppState、事件枢纽、CoreManagerService 适配器
        ├─ control/      install / uninstall / start / stop / restart / status 引擎
        ├─ update/       自更新引擎(staging、验签、交接、回滚)
        └─ runtime/      RuntimeInfos、目录、日志初始化(全部注入式)
   依赖:nyanpasu-ipc(server+client)、nyanpasu-core-manager、nyanpasu-utils、clash-api
nyanpasu-ipc            (现状 + IpcOperation 契约层,wire 不变)
```

命名:lib 取 `nyanpasu-service-runtime`,置于 `crates/`(与现有新 crate 排布一致);bin 保持 `nyanpasu-service` 名称与发布产物不变。

## 4. 方案 A:core-manager 接线(S1,原 P4)

### 4.1 适配器

新 `CoreManagerService` = 薄包装(约 200 行,取代 349 行的 instance.rs):

```rust
pub struct CoreManagerService {
    manager: nyanpasu_core_manager::CoreManager,
    /// wire 类型回显:记住请求时的旧 CoreType(manager 不感知 alpha 变体)
    requested_core: parking_lot::RwLock<Option<CoreType>>,
    /// apply_config 的乐观并发:持久化上次成功 revision
    last_revision: parking_lot::RwLock<Option<RevisionId>>,
}
```

- 构造:`CoreManager::new(options)` 为 async fallible,在 `server::run` 早期构建;`runtime_dir` 指向 service_data_dir 下专属子目录。
- `ControllerMode`:默认 `Passthrough`(与旧行为、GUI 现状兼容);`Managed` 由 server 启动参数/配置显式开启(GUI 适配后)。
- 状态桥接 task:`manager.subscribe()`(watch)→ 按 core-manager 设计 §7 的有损映射表转 IPC `CoreState` → 仅在映射结果变化时发 `Event::new_core_state_changed`(去重)。
- 日志桥接:`manager.subscribe_logs()`(broadcast<LogFrame>)→ ws 事件,替代现 Logger 全局订阅回调中转核心日志的职责。

### 4.2 路由语义映射

| IPC 端点 | 旧实现 | 新实现 |
|---|---|---|
| `POST core/start` | `CoreManagerService::start` | CoreType→`CoreKind`+binary 名(`find_binary_path` 保留在 service 层)组装 `InstanceSpec` → `manager.start(spec)`;`AlreadyRunning` 映射原错误文案 |
| `POST core/stop` | `stop` | `manager.stop()` |
| `POST core/restart` | take+递归 start | `manager.restart()`(内部即 switch(last_spec),epoch 递增) |
| `GET status` | 锁内拼 CoreInfos | `manager.status()` 快照 + 回显 requested_core |

- 运行中收到 start:保持旧语义报错(wire 兼容);"运行中换配置走 `switch` 平滑切换"暴露为新能力属协议演进,记入开放问题 O1。

### 4.3 删除清单(接线完成即删)

- `server/instance.rs` 整个(含 `recover_core`、`handle_command_event`);
- `server::run` 里的 mpsc 状态通道(被 watch 桥接取代);
- `cmds/server.rs` 的 `kill_by_pid_file` 启动清扫 + `service_core_pid_file()`——manager 的 per-epoch pid 记录与孤儿回收接管;
- `nyanpasu-utils::core` 在 service 的依赖引用,utils 侧标 `#[deprecated]`(删除另行,避免影响 GUI 侧对 `CoreType` wire 类型的引用——`CoreType` 仍是 IPC wire 类型,保留在 utils 或迁至 ipc crate,见 O2)。

### 4.4 验收

- 等价性:接线前后对 start/stop/restart/status 的 IPC 可见序列做 golden 对照(roundtrip 测试扩展);
- fake_core 跑通 route 级测试(见 §10);真核集成套件(CI 已有)全绿。

## 5. 方案 B:并发模型——actor 评估与采纳范围(S2)

### 5.1 评估

| 层 | 现状 | actor 化收益 | 结论 |
|---|---|---|---|
| 控制面(start/stop/restart/switch) | core-manager 内部已是"async Mutex 串行控制面 + watch 发布"(其设计已评审确认的混合模型) | 在外面再包一层 mpsc+oneshot 只是把"锁排队"换成"信箱排队",语义相同,增加请求/应答样板与一层错误传递 | **不做**。路由直接调 `CoreManagerService`(内部无新增锁,仅两个 RwLock 元数据) |
| 事件面(ws 广播) | DashMap 订阅表 + join_all 逐个 await(P3 三缺陷) | actor 思想的核心收益在此:共享可变态收敛为 channel | **采纳**,但实现选 `tokio::sync::broadcast` 而非手写 actor task(见 5.2) |
| 每连接处理 | 已是独立 task | —— | 保持,即天然的 per-connection actor |

结论:actor 模式要消灭的"锁复杂度"90% 由 S1 接线消灭(instance.rs 整体删除);剩余事件面用 broadcast 通道达成同等效果且代码更少。如后续出现每客户端过滤/ack 等有状态需求,再把 hub 升级为真 actor,升级是局部的。

### 5.2 EventHub 设计

```rust
pub struct EventHub {
    tx: tokio::sync::broadcast::Sender<Event>,   // 容量 ~256
}
// 广播:hub.send(event) —— 无 await、无锁、慢客户端零影响
// 订阅:每个 ws 连接 subscribe() 得独立 Receiver
```

- 慢客户端:`Lagged(n)` 时该连接自行记日志/可选下发 lag 提示,其他订阅者零影响——消除队头阻塞;
- 订阅表、socket id 全部消失——**碰撞 bug 连同其数据结构一起删除**;
- service 自身日志:`Logger` 订阅回调改为向 hub `send`(同步、无 spawn),行序有保证;核心日志走 4.1 的 LogFrame 桥接。

### 5.3 去全局化明细(与 crate 拆分同期)

| 全局 | 替代 |
|---|---|
| `RuntimeInfos::global()` | 构造期注入 `Arc<RuntimeInfos>` 进 `AppState`;`find_binary_path` 改为吃 `&RuntimeInfos` 参数 |
| `Logger::global()` | `server::run` 构造 `Logger`,handle 放 `AppState`;`inspect_logs` 路由从 state 取 |
| `SHUTDOWN_TOKEN` | `run()` 接收 `CancellationToken`(已是参数);bin 层持有并接 ctrlc/win_service,OnceLock 只留在 bin |

验收:`create_router(state)` 可在测试中用任意临时目录构造的 `AppState` 实例化,多用例并行互不干扰。

## 6. 方案 C:bin/lib 拆分(S3)

- **bin(`nyanpasu_service`)保留**:`main.rs`、clap 定义与解析(`cmds/mod.rs` 的 `Cli`/`Commands`)、exit code 映射、`win_service.rs`、ctrlc/panic hook。特征:不含业务逻辑,每个分支一行调 lib。
- **lib(`crates/nyanpasu-service-runtime`)迁入**:`server/*`(去全局化后)、`cmds/{install,uninstall,start,stop,restart,status,update}` 的函数体(签名改为吃参数结构体 + `&dyn ServiceManager`,后者本就是 trait 对象——现成的 mock 缝)、`utils/*`、`logging.rs`(改为返回 guard 的注入式初始化)。
- 顺序:先在原 crate 内完成 §5.3 去全局化(可独立合并、行为不变),拆分即变成纯文件搬运 + 可见性调整,风险最低。
- `consts.rs` 的 build 信息(commit/rustc 版本等)留 bin(build.rs 产物);`APP_NAME`/`SERVICE_LABEL` 等下沉 lib。

## 7. 方案 D:IPC 契约与路由(S4)

### 7.1 契约单一事实来源

`nyanpasu_ipc::api` 新增(纯新增,不动现有类型与常量):

```rust
pub trait IpcOperation {
    const METHOD: http::Method;
    const PATH: &'static str;          // 复用现有 *_ENDPOINT 常量
    type Req: Serialize + DeserializeOwned;
    type Res: Serialize + DeserializeOwned;
}
pub struct CoreStart;   // impl IpcOperation for CoreStart { PATH = CORE_START_ENDPOINT; ... }
```

- client:`Client::call::<Op>(req)` 泛型方法;`shortcuts.rs` 现有方法改为其一行别名(对外签名不变);
- server:`register::<Op>(router, handler)` 保证挂载路径/方法来自同一常量;
- **契约测试**:遍历 op 清单断言 (a) router 挂载集合 == op 集合,(b) Req/Res 对 golden JSON 快照序列化稳定(wire 兼容的回归闸)。
- 不写宏。op 目前 7 个,手写 impl 成本低于宏的维护成本。

### 7.2 中间件与错误面

- 统一 layer 栈:`TraceLayer`(已有)+ request-id + `TimeoutLayer`(控制面操作 30s 上限,防挂死占住 IPC)+ `CatchPanicLayer`(panic → 500 R envelope 而非断管);
- 统一 fallback:404/405 也返回 `R` envelope,client 侧错误可解析;
- ws 端点不套 timeout。

## 8. 方案 E:CLI 人体工学(S5)

| 项 | 现状 | 提议 | 兼容性 |
|---|---|---|---|
| version | 手动 `--version` flag + `print_version` 内 `exit(0)` | clap 原生 `-V/--version`(短版本行);花体盒子移到 `version` 子命令 | ⚠️ 与 GUI/脚本协调 |
| verbose | `-V/--verbose` | `-v/--verbose`(count:`-vv` = trace) | ⚠️ 同上 |
| 裸调用 | eprintln "No command specified" | `arg_required_else_help = true` | ✅ |
| `rpc start-core --core-type` | JSON 解析,需 `'"mihomo"'` | `clap::ValueEnum`(`mihomo`/`mihomo-alpha`/`clash-rs`/...) | ✅(旧写法同时兼容一版) |
| `install` 参数 | 4 个必填长参 | 增加 `env` 回退(`NYANPASU_*`);保持长参优先 | ✅ 纯放宽 |
| `update` | 无参 | `--from <path>`(候选二进制;为 S6 铺路)、`--check`(dry-run 只比版本) | ✅ 纯新增 |
| 补全 | 无 | `clap_complete` 隐藏子命令 `completions <shell>` | ✅ |
| `status` 退出码 | 恒 0 | `--exit-code` opt-in:running=0 / stopped=非 0 | ✅ opt-in |

- ⚠️ 项打包为一个"CLI v2"变更,与 GUI 仓库同步 PR;其余可即刻落地。
- 测试:`Cli::command().debug_assert()` + `try_parse_from` 用例矩阵;bin 级用 `assert_cmd` 验 `status --json` 输出 schema 与退出码表。

## 9. 方案 F:免 UAC 自更新(S6)

### 9.1 原则

服务进程(SYSTEM/root)自己有权替换自己的二进制——缺的不是权限,是**信任依据**。现在的信任依据是"UAC 弹窗时用户点了是";替换为"候选二进制携带项目私钥的有效签名"。签名校验取代 UAC,成为唯一的门。

### 9.2 流程

```text
GUI(非提权)                          service(SYSTEM/root)
 1. 下载新版 zip + manifest ──IPC──▶ 2. POST /service/update { candidate_path }
                                     3. 复制候选到 staging(service_data_dir/staging/,
                                        仅 SYSTEM/root 可写)—— 之后不再触碰用户可写路径
                                     4. 在 staged 副本上验证(防 TOCTOU):
                                        a. manifest(minisign 签名):{version, target, sha256}
                                        b. sha256(staged) == manifest.sha256
                                        c. [Windows 加验] Authenticode 链 + 签名者身份钉扎
                                        d. manifest.version > 当前版本(防降级)
                                     5. spawn staged 新二进制 `update finalize`(分离进程,
                                        继承 SYSTEM/root 上下文):
                                        - SCM/systemctl 停服务
                                        - 当前 exe rename → nyanpasu-service.old(Windows
                                          允许重命名运行中映像文件)
                                        - staged rename → 正式路径(同卷原子)
                                        - 启动服务;轮询 status 端点 ≤30s
                                        - 失败 → 换回 .old 再启动(回滚)
                                     6. 新服务上线,ws 广播 updated 事件;.old 保留一代
```

- UAC 仅剩:首次 `install`、`uninstall`。
- 现有 `update` 命令保留为降级路径(签名基础设施故障时的兜底),并复用同一套 staging/验签引擎(提权跑时跳过 IPC)。

### 9.3 威胁模型

| 威胁 | 对策 |
|---|---|
| 恶意本地进程调用 update IPC | IPC 本有 ACL(Windows SID / unix nyanpasu group);且**签名是真正的门**——最坏结果 = 装上另一个官方正版版本 |
| TOCTOU(验签后换文件) | 只在 SYSTEM/root 专属 staging 目录内验证与安装,验证对象即安装对象 |
| 降级攻击(装旧版利用已修漏洞) | manifest 版本单调强制;显式降级需未来签名内 `allow_downgrade` 声明 |
| 换装中途崩溃 | staging + rename 原子性;`.old` 存在即可回滚;finalize 幂等可重入 |
| 私钥泄露 | minisign 公钥编译期内嵌;轮换走双钥过渡(新版本同时内嵌新旧公钥) |
| dev/fork 构建无签名 | 仅 `debug_assertions` 或显式 `--allow-unsigned`(且要求提权调用方)放行,release 服务端一律拒绝 |

### 9.4 实现要点

- 验签库:manifest 用 `minisign-verify`(纯 Rust、验签-only、tauri updater 同款,选型实施时终验);Authenticode 用 `WinVerifyTrust` + 证书指纹/主体钉扎(windows crate 已在依赖树)。
- 传输:IPC 只传路径不传字节(候选包 GUI 已落盘);service 从该路径**复制**进 staging——路径可为用户可写,因为后续一切以 staged 副本为准。
- `update finalize` 是隐藏子命令,由 lib 的 update 引擎实现,bin 转发;所有步骤写审计日志(service 日志 + 事件)。
- Unix:同构 helper 流程(systemd `Restart=` 不依赖);rename 覆盖天然原子。
- 引擎全程不假设自己在服务进程内 → 可在测试中以普通进程 + 临时目录 + 测试密钥对全流程演练(含回滚)。

## 10. 测试策略(总)

| 层 | 手段 | 依赖 |
|---|---|---|
| 状态映射/契约 | 纯单测:manager CoreState→IPC 映射表逐臂断言;IpcOperation golden JSON | 无 |
| route handler | `Router` + `tower::ServiceExt::oneshot`,`AppState` 用临时目录构造;core 路由后接 fake_core(复用 core-manager tests/helpers) | S2 去全局化 |
| EventHub | 多订阅者广播、慢消费 Lagged、断连清理 | S2 |
| IPC 全链路 | 现 roundtrip 扩展:pipe/uds 名参数化随机后缀,可并行;新增 update 端点用例 | 无 |
| 控制面引擎 | mock `dyn ServiceManager`(trait 对象现成);install/start/stop/status 状态机分支全覆盖 | S3 |
| CLI | `debug_assert()` + `try_parse_from` 矩阵;`assert_cmd` 验 json schema/退出码 | S5 |
| 更新引擎 | 临时目录 + 测试 minisign 密钥对:坏签名/坏哈希/降级/finalize 中断重入/健康失败回滚 | S6 |
| 等价性 | S1 前后 IPC 可见序列 golden 对照 | S1 |
| 覆盖率 | CI 加 `cargo-llvm-cov`:`nyanpasu-service-runtime`/`nyanpasu_ipc` 初始阈值 60%,只升不降(ratchet) | S3 后 |

## 11. 实施阶段(每段独立可合并)

| 阶段 | 内容 | 验收 | 依赖 |
|---|---|---|---|
| S1 | core-manager 接线,删 instance.rs 及 §4.3 清单 | 等价性 golden 绿;真核套件绿 | 无 |
| S2 | EventHub(broadcast)+ 去全局化;修 ws 碰撞 bug | EventHub/route 测试落地并绿 | S1 |
| S3 | bin/lib 拆分;控制面引擎注入化;覆盖率 CI | mock ServiceManager 测试;阈值生效 | S2 |
| S4 | IpcOperation 契约层 + 中间件栈 | 契约测试 + wire golden 绿 | 可与 S3 并行 |
| S5 | CLI 人体工学(兼容项即刻,⚠️ 项协调 GUI) | CLI 测试矩阵绿 | S3 |
| S6 | 自更新引擎 + `/service/update` 端点 + finalize | §10 更新引擎全用例绿;手动端到端(双平台) | S3(引擎入 lib)、S4(端点走契约层) |

## 12. 风险与开放问题

| # | 问题 | 影响 | 处置 |
|---|---|---|---|
| O1 | "运行中 start"是否改为隐式 switch(平滑换配置) | GUI 升级体验 vs wire 语义稳定 | S1 保持旧语义;switch 作为新端点在协议演进中单独提 |
| O2 | wire 类型 `CoreType` 目前住在 nyanpasu-utils,utils::core 废弃后归属 | GUI 依赖该类型序列化 | 倾向迁入 `nyanpasu_ipc::types` 并在 utils re-export 过渡;S1 时定 |
| O3 | 签名基础设施:CI 私钥托管(GH secrets)、Authenticode 证书有无 | S6 是否可全量启用 | 无 Authenticode 证书时 Windows 仅 minisign 单签,钉扎项降级为可配 |
| O4 | `-V`/`--verbose` 变更影响 GUI 调用脚本 | 破坏兼容 | 归入"CLI v2"与 GUI 同版本对齐发布 |
| O5 | `service-manager` crate 各平台 restart 语义差异(finalize 依赖 stop/start 可靠性) | 自更新可靠性 | S6 实施期做三平台 stop/start/status 一致性 smoke(macOS 已有 launchctl 兜底先例) |
| O6 | Managed 模式默认化时机(GUI 端点发现适配) | 平滑切换收益兑现 | 保持 Passthrough 默认;上游就绪后翻转 |
