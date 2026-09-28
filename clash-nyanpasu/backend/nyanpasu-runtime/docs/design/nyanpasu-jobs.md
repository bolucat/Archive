# nyanpasu-jobs 独立交付审计指引

范围：按应用侧 [P0 契约](https://github.com/libnyanpasu/clash-nyanpasu/blob/a4be9966cf765e430e214133e5bc7544fba4deb0/docs/design/jobs-p0-contract.md)，完成 P1 → P2 → P3 → P5 通用日志与查询能力。runtime 基线为 `5d157fb`。不包含 P4 Profiles、P5 Tauri IPC、P6 前端或 P7 旧框架切换。

## 阶段与代码边界

| 阶段        | 实现                                                                                   | 主要验收                                                                                                |
| ----------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| P1          | `job.rs` / `typed.rs` 定义与强类型 handle，`actor.rs` 控制面，`runner.rs` 单 Run owner | 多等待者、等待超时、同键删除重建、取消确认、panic/delegation、blocking 归属、无 redb 的 fake store      |
| P2          | `storage/mod.rs` 窄端口、`journal.rs` 单个有界 blocking worker，`storage/redb.rs` 事务/索引/恢复/retention  | 接纳失败零副作用、接纳不确定读回、保存失败/阻塞保留真实结果、只重试 journal、重开恢复、索引清理         |
| P3          | `schedule.rs` 的 jiff-cron 日历计算、jiff `Timestamp` 与 `Clock` 墙钟端口，actor 中的单调 deadline/scope reconcile | 首次完整间隔、手动不改 anchor、no-op、删改 fence、旧 revision、scope 隔离、休眠跳过、墙钟前跳/回退、DST |
| P5 通用部分 | `logging.rs` 有界 ingress/layer，Run owner 批次/封闭/终态屏障，`dto.rs` 有限 DTO       | 子任务及 blocking 日志关联、允许列表、队列溢出、游标、封闭后的迟到事件、写阻塞、Specta 大整数字符串导出 |

## 实施中收敛的选择

- 使用一个有确认的 timer 唤醒任务，而非每个 Job 独立 timer。Tick 不携带旧定义；actor 同时读取当前 registration、计算 Due 和接纳。reconcile 唤醒 timer 重算，因此不需要在外部排队旧 generation 的 Due。
- JobsActor 拥有业务生命周期状态；`storage` 模块公开同步 `JobStore` 端口与可选 redb adapter，`journal.rs` 的单个 blocking worker 通过有界通道接收存储操作。redb 文件完整路径由应用指定，缺失父目录自动创建。未来接入 Turso 时，同步端口与 async API 的适配仍需设计。没有第二份共享可变 Run 注册表，也不在 actor handle 内等待数据库。
- Run 归属使用受管理子任务及显式 delegation token。父 handler 返回后封闭新子任务入口，再等已有工作完成。库无法追踪调用方自行创建的 detached task；跨 actor 委托必须遵循 token 协议。
- 完成后的查询直接读 journal；仅活跃/待持久化的结果使用 watch 保留，不另建可能违反 retention 的历史缓存。内存结果不会在保存故障时因缓存容量被驱逐。
- 墙钟时间使用 jiff `Timestamp`；Cron 使用 jiff-cron 原生 DST 与日历语义，外部仍支持 5/6 字段及常见 `@` 别名。单调间隔仍使用 Tokio `Instant`。
- 调度 Busy 记录也占全局在途上限。容量耗尽或 journal 降级时聚合 dropped trigger 数，不为历史记录建立无限队列。
- 接纳错误区分保证未提交的 AdmissionFailed 与结果未知。后者只按原 ID 读回，不再执行 admission；确定存在记录后由原 owner 继续一次执行。终态写入不确定时也保留原 write，禁止堆积重复 blocking 调用。
- shutdown 的等待范围包含 actor、Run、journal 队列和 blocking worker join。超时返回报告，下一次调用继续同一组资源的 drain，不将 abort 当停止。

## 审计重点

1. `State::admit` 的稳定 JobKey 与全局名额在接纳待定时已预留；删改定义不会释放旧名额。
2. `RunExecution::run` 只有持久化接纳确认才进入 handler，且重试循环不含 handler 调用。
3. 终态发布时间分 durable/degraded；Degraded 的实际 writer 继续被 owner 持有。不得将等待超时或结果编码失败解释为业务失败。
4. `JobContext::drain` 先封闭新子任务再等待已有任务/token。发生 panic 且仍有委托时 inspect 保持非终态。
5. `RedbJobStore` 在同一事务里写记录/索引、写尾部日志/终态、删记录/索引/日志；分页不依赖墙钟排序。
6. 日志默认 deny-all，仅按精确 target/字段允许列表采集。泛型业务错误和输出仍需 handler 先转换为可安全持久化的领域报告。
7. 应用集成时必须先装独立过滤的 layer、注入唯一 store，最后显式 shutdown。不能增加 global accessor 或让 handler 回调再次触发同一个 Job 的 facade。

## 验证边界

测试覆盖执行状态竞争、注入的接纳/日志/终态失败与写阻塞、临时 redb 重开恢复、虚拟调度时间、注入墙钟和有限 DTO 导出。`portable.rs` 用独立 fake store 验证关闭 redb feature 后仍可执行强类型 Job。三平台 CI 新增两种 feature 配置的测试。

未声称完成真实 ENOSPC、断电、OS suspend 或应用核心协调验证。泛型 crate 也不定义 Profiles 的 Reconciled/Unknown 报告；那属于 P4 workflow。后续应用接线必须保留 P0 中的提交与核心应用边界，不因 crate 已交付而跳过集成验收。

## 本地交付验证（2026-09-28）

| 检查                                                            | 结果                                                           |
| --------------------------------------------------------------- | -------------------------------------------------------------- |
| `cargo test -p nyanpasu-jobs --all-features --locked`           | 41 项集成测试及 1 项文档编译测试通过                           |
| `cargo test -p nyanpasu-jobs --no-default-features --locked`    | 12 项集成测试及 1 项文档编译测试通过                            |
| 新 crate 的 all-targets/all-features Clippy `-D warnings`       | 通过                                                           |
| workspace rustfmt 检查                                          | 通过                                                           |
| `cargo check --workspace --all-targets --all-features --locked` | 通过；保留未修改代码的既有 warnings                              |
| `manual` 示例                                                   | 实际执行得到 42，并完成显式 shutdown                           |

本地验证环境为 macOS；Linux/Windows 由本 PR 的 CI 矩阵运行，不能将本地通过写成三平台均已通过。Cargo.lock 保留当前主线已有包版本，仅加入 jobs 所需依赖和多版本消歧。
