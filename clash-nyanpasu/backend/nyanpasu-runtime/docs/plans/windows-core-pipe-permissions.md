# Windows 内核管道权限计划

- [x] 抽取 `nyanpasu-windows-security`，迁移现有 IPC 的 SID/SDDL 实现，保持原 IPC 权限语义。
- [x] 新增 `NamedPipeSecurityDescriptor` Feature：支持的内核在创建时设置权限，其余内核由服务设置；使用同一安装 SID 列表，并验证管道服务端 PID 和 DACL。
- [x] 覆盖权限拒绝、错误原生设置、运行中权限重置、自动重启及真实内核 REST/WebSocket，接入 CI。

授权逻辑通过 `ControllerAccess` 注入，创建参数属于单个实例，自动重启沿用；readiness 和 liveness 都检查授权。没有修改进程全局环境，也没有保留旧 ACL API 包装层。

能力依据：Mihomo [v1.18.9 的实现](https://github.com/MetaCubeX/mihomo/blob/v1.18.9/adapter/inbound/listen_windows.go) 已读取 `LISTEN_NAMEDPIPE_SDDL`；clash-rs [v0.9.7 的实现](https://github.com/ibigbug/clash-rs/blob/v0.9.7/clash-lib/src/app/api/ipc.rs) 使用固定描述符，走服务设置路径。

验证：Windows 普通权限下，真实 Mihomo 和 clash-rs 的 REST/WebSocket、自动重启测试通过；受限令牌被 Windows 拒绝访问。完整相关测试串行通过（410 通过、27 忽略，真实内核两项另行执行通过）；Clippy 与 Linux 目标的相关 crate/tests 编译检查通过（保留原有警告）。曾有一个原有配置补偿测试在并发执行时超时，单独与串行复查均通过。

限制：服务事后设置存在从创建到授权的窗口，不能撤销此前已打开的句柄。当前终端没有提升权限，尚未执行 LocalSystem 服务与普通 GUI 的双身份验收。变更保留在 runtime 子模块独立 worktree，主仓库应在新服务版本发布后再更新发布标签 pin。
