# nyanpasu-utils `process` 模块实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `crates/nyanpasu-utils` 中落地通用子进程管理模块 `process`(feature `"process"`):Command → spawn → 事件流 + 句柄 + Supervisor,以 processkit 为内部引擎。

**Architecture:** 公开 API 全部为自有类型(`Command` / `ProcessEvent` / `ProcessHandle` / `Supervisor`);processkit 只允许出现在 `src/process/engine.rs`(及 P0 探针 example)中。引擎用"单一所有者 + 控制消息"模式:一个 tokio 任务独占 `RunningProcess`,通过 `select!` 同时泵送输出事件和处理 kill/stdin 控制消息,终止后经 watch 通道广播 `TerminatedPayload`。

**Tech Stack:** Rust edition 2024、tokio、processkit(git 锁定 `2adad32`)、encoding_rs、tokio-util(CancellationToken)、thiserror 2。

**规格来源:** `docs/superpowers/specs/2026-07-16-nyanpasu-utils-process-module-design.md`(§5 模块设计、§7 测试策略、§8 验收标准)。

## Global Constraints

- **范围红线:不得修改** `crates/nyanpasu-utils/src/core/**`、`crates/nyanpasu-utils/src/os/child.rs`、`crates/nyanpasu-core-manager/**`、`nyanpasu_service/**`(core-manager 迁移是独立计划)。
- 工具链:`rustc --version` 必须 ≥ 1.88(processkit MSRV);工作区 edition 2024。
- processkit 依赖**只允许 git 锁定**:`git = "https://github.com/ZelAnton/ProcessKit-rs", rev = "2adad322f1e3d7c471f3b9c8c625e44f356e416b"`,features 含 `"tracing"`;禁止改用 crates.io(设计 §9 R4)。
- processkit 类型不得出现在任何 `pub` 签名;`use processkit` 仅允许出现在 `src/process/engine.rs` 与 `examples/containment_probe.rs`。
- `src/process/**` 内禁止出现 `std::thread::spawn`(验收 §8.3)。
- 事件顺序合同:`ProcessEvent::Terminated` 恒为通道最后一条事件。
- 默认值(设计 §5.2/§5.5):事件通道容量 **64**;`kill_grace` **5s**;输出缓冲上限 **256 KiB 环形**(`OutputBufferPolicy::unbounded().with_max_bytes(256 * 1024)`);`hide_window` 默认 **true**。
- 每个任务提交前:`cargo fmt` + `cargo clippy -p nyanpasu-utils --features process -- -D warnings` 通过。
- 测试命令基线:`cargo test -p nyanpasu-utils --features process`(本机 Windows;Unix 专属测试用 `#[cfg(unix)]` 门控,CI 上跑)。
- processkit 本地源码副本(API 查证用,与锁定 rev 一致):`C:\Users\a6320\AppData\Local\Temp\claude\G--Programs-Rust-nyanpasu-service\4376f978-6be6-4da0-a491-b6cba276ab6b\scratchpad\processkit-audit`;若不存在则 `git clone https://github.com/ZelAnton/ProcessKit-rs && git checkout 2adad32`。
- 已核验的 processkit API 锚点(文件:行号基于 rev `2adad32`):`Command::start()`;`RunningProcess` 的 `OutputEvents` 流(`src/running/stream.rs:641`,`OutputEvent::Stdout(OutputLine)/Stderr(OutputLine)`,`OutputLine::into_text()`);`Finished::stderr`(`stream.rs:51`);`Command::create_no_window()`(`src/command.rs:496`);`stdout_encoding/stderr_encoding/encoding`(`command.rs:1024/1030/1036`);`Command::output_buffer` + `OutputBufferPolicy::unbounded().with_max_bytes`(`src/buffer.rs:256`);`ProcessGroup::signal(Signal)`(`src/group.rs:376`,`Signal::Term/Kill`);`RunningProcess::shutdown(grace)`(`src/running/mod.rs:806`,仅 Unix 有优雅层);`mechanism()` 返回 JobObject/CgroupV2/ProcessGroup。
- git:所有工作在分支 `feat/utils-process-module` 上进行;逐任务提交,消息用 Conventional Commits。

---

### Task 1: Feature 接线与公共类型

**Files:**
- Modify: `crates/nyanpasu-utils/Cargo.toml`
- Modify: `crates/nyanpasu-utils/src/lib.rs`
- Create: `crates/nyanpasu-utils/src/process/mod.rs`
- Create: `crates/nyanpasu-utils/src/process/event.rs`
- Create: `crates/nyanpasu-utils/src/process/error.rs`

**Interfaces:**
- Produces(后续所有任务依赖):
  - `ProcessEvent { Stdout(String), Stderr(String), Error(String), Terminated(TerminatedPayload) }`(`#[non_exhaustive]`)
  - `TerminatedPayload { code: Option<i32>, signal: Option<i32> }`
  - `ProcessError`(thiserror 枚举,见下)
  - `ProcessOutput { code: Option<i32>, stdout: String, stderr: String }` + `fn success(&self) -> bool`

- [ ] **Step 1: 建分支并确认工具链**

```powershell
git checkout -b feat/utils-process-module
rustc --version   # 必须 >= 1.88,否则 rustup update stable 后重查
```

- [ ] **Step 2: Cargo.toml 增加依赖与 feature**

在 `crates/nyanpasu-utils/Cargo.toml` 的 `[features]` 中新增:

```toml
process = ["os", "dep:processkit", "dep:encoding_rs", "dep:tokio-util"]
```

在 `[dependencies]` 中新增(encoding_rs 已存在,无需重复):

```toml
processkit = { git = "https://github.com/ZelAnton/ProcessKit-rs", rev = "2adad322f1e3d7c471f3b9c8c625e44f356e416b", features = ["tracing"], optional = true }
tokio-util = { version = "0.7", optional = true }
```

- [ ] **Step 3: lib.rs 挂载模块**

在 `crates/nyanpasu-utils/src/lib.rs` 末尾追加:

```rust
#[cfg(feature = "process")]
pub mod process;
```

- [ ] **Step 4: 写类型与单元测试(先写测试)**

`crates/nyanpasu-utils/src/process/mod.rs`:

```rust
//! Generic child-process management: spawn, event stream, kill, supervise.
//!
//! Design: docs/superpowers/specs/2026-07-16-nyanpasu-utils-process-module-design.md

mod error;
mod event;

pub use error::{ProcessError, ProcessOutput};
pub use event::{ProcessEvent, TerminatedPayload};
```

`crates/nyanpasu-utils/src/process/event.rs`:

```rust
/// Exit information of a terminated child. Field semantics match the legacy
/// `core::TerminatedPayload` so downstream migration is a rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminatedPayload {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

/// Events delivered on the channel returned by [`crate::process::Command::spawn`].
///
/// Contract: `Terminated` is always the final event on the channel.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ProcessEvent {
    Stdout(String),
    Stderr(String),
    /// Non-fatal IO/decode error while pumping output. The process may still be alive.
    Error(String),
    Terminated(TerminatedPayload),
}
```

`crates/nyanpasu-utils/src/process/error.rs`:

```rust
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProcessError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to spawn `{program}`: {message}")]
    Spawn { program: String, message: String },
    #[error("process timed out after {after:?}")]
    Timeout { after: Duration },
    #[error("process already exited")]
    AlreadyExited,
    #[error("stdin is not piped (enable Command::pipe_stdin) or already closed")]
    StdinUnavailable,
    /// Engine-internal failures that have no dedicated variant. The engine maps
    /// processkit errors to strings here so processkit types never leak.
    #[error("process engine error: {0}")]
    Engine(String),
}

/// Result of [`crate::process::Command::output`] — one-shot capture.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl ProcessOutput {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_output_success_only_on_zero() {
        let mk = |code| ProcessOutput { code, stdout: String::new(), stderr: String::new() };
        assert!(mk(Some(0)).success());
        assert!(!mk(Some(1)).success());
        assert!(!mk(None).success());
    }

    #[test]
    fn error_display_is_stable() {
        let e = ProcessError::Spawn { program: "mihomo".into(), message: "not found".into() };
        assert_eq!(e.to_string(), "failed to spawn `mihomo`: not found");
    }
}
```

- [ ] **Step 5: 运行测试验证通过**

```powershell
cargo test -p nyanpasu-utils --features process process::
```
Expected: 2 passed(以及既有测试不受影响:`cargo check -p nyanpasu-utils` 无 feature 也要通过)。

- [ ] **Step 6: Commit**

```powershell
git add crates/nyanpasu-utils
git commit -m "feat(utils/process): add process feature wiring and public event/error types"
```

---

### Task 2: P0 探针 example(O3 关闭 + 引擎 API 侦察)

**Files:**
- Create: `crates/nyanpasu-utils/examples/containment_probe.rs`
- Modify: `crates/nyanpasu-utils/Cargo.toml`(example 声明)

**Interfaces:**
- Produces: 无代码接口。产出两件事:(1) 本机 `mechanism()` 实测结果;(2) 五个引擎接线问题的答案(写入本任务 Step 4 的核对清单,供 Task 5 使用)。

**这是决策门任务**:若 Windows 本机未报告 `JobObject`,或核对清单发现 API 形态与锚点严重不符,**停止执行本计划**,回到设计文档 §4 评估降级方案 B。

- [ ] **Step 1: Cargo.toml 声明 example**

```toml
[[example]]
name = "containment_probe"
required-features = ["process"]
```

- [ ] **Step 2: 写探针(此文件允许直接 use processkit,不算封装违例)**

`crates/nyanpasu-utils/examples/containment_probe.rs`:

```rust
//! P0 probe (design doc §6/§9 O3): print the actual containment mechanism and
//! prove kill-on-drop on this host. Run on every deployment target class.
//!
//! cargo run -p nyanpasu-utils --features process --example containment_probe

use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Long-running child. Platform-native sleeper, no helper bin needed.
    #[cfg(windows)]
    let cmd = processkit::Command::new("cmd")
        .args(["/C", "ping -n 3600 127.0.0.1 >NUL"])
        .create_no_window();
    #[cfg(unix)]
    let cmd = processkit::Command::new("sleep").args(["3600"]);

    let run = cmd.start().await?;
    // NOTE: adjust accessor per recon — mechanism may live on the RunningProcess
    // or on its process group. Print whatever the API exposes.
    println!("containment mechanism = {:?}", run.mechanism());
    let pid = run.pid();
    println!("child pid = {pid}");

    // 2) Drop the handle -> the kernel object must reap the child.
    drop(run);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let alive = {
        use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};
        let kind = RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing());
        let mut s = System::new_with_specifics(kind);
        s.refresh_specifics(kind);
        s.process(Pid::from_u32(pid)).is_some()
    };
    println!("child alive after drop = {alive} (MUST be false)");
    assert!(!alive, "kill-on-drop failed on this host");
    println!("PROBE OK");
    Ok(())
}
```

- [ ] **Step 3: 编译并运行;编译错误在此步消化**

```powershell
cargo run -p nyanpasu-utils --features process --example containment_probe
```
Expected 输出含:`containment mechanism = JobObject`、`child alive after drop = false`、`PROBE OK`。
编译报错(方法名/所有权不符)时,对照本地 processkit 源码副本修正 example——这正是本任务的侦察目的。

- [ ] **Step 4: 回答引擎接线核对清单(答案写进本文件此处,Task 5 依赖)**

对照 processkit 源码副本逐条确认并**把答案追加到本步骤下方**:

1. `mechanism()` 挂在哪个类型上(`RunningProcess` / `ProcessGroup`),返回类型叫什么?
2. `RunningProcess` 的输出事件流:`output_events()` 的确切方法名与返回类型;返回值是否**不携带**对 `RunningProcess` 的借用(即可移入另一任务)?
3. kill / 优雅关停的控制面:`RunningProcess::shutdown(grace)` 与 `kill()` 的签名(`self` / `&mut self` / `&self`);是否存在可克隆/共享的组句柄(如 `run.group()`)使控制与泵送可分离?
4. stdin:`keep_stdin_open()` 的调用位置(builder 还是 RunningProcess)与 writer 类型。
5. 一次性捕获 verb:`output_string()`/`output_bytes()` 的返回结构字段名(code / stdout / stderr / timed_out)。

> 答案记录区(执行时填写):
> 1. …
> 2. …
> 3. …
> 4. …
> 5. …

- [ ] **Step 5: 决策门判定 + Commit**

若 `JobObject` + kill-on-drop 通过 → 继续;否则停止并报告。

```powershell
git add crates/nyanpasu-utils docs/superpowers/plans/2026-07-17-nyanpasu-utils-process-module.md
git commit -m "chore(utils/process): add P0 containment probe example (O3) with recon notes"
```

---

### Task 3: Command builder(纯配置,不 spawn)

**Files:**
- Create: `crates/nyanpasu-utils/src/process/command.rs`
- Modify: `crates/nyanpasu-utils/src/process/mod.rs`

**Interfaces:**
- Produces(Task 5/7/9/10/12 依赖,签名以此为准):

```rust
pub struct Command { /* 字段 pub(crate),见下 */ }
impl Command {
    pub fn new(program: impl AsRef<std::ffi::OsStr>) -> Self;
    pub fn arg(self, a: impl AsRef<std::ffi::OsStr>) -> Self;
    pub fn args<I, S>(self, args: I) -> Self where I: IntoIterator<Item = S>, S: AsRef<std::ffi::OsStr>;
    pub fn env(self, k: impl AsRef<std::ffi::OsStr>, v: impl AsRef<std::ffi::OsStr>) -> Self;
    pub fn current_dir(self, dir: impl Into<std::path::PathBuf>) -> Self;
    pub fn encoding(self, enc: Option<&'static encoding_rs::Encoding>) -> Self; // None = UTF-8
    pub fn hide_window(self, hide: bool) -> Self;          // default true; Unix no-op
    pub fn kill_grace(self, d: std::time::Duration) -> Self; // default 5s; Windows 无优雅阶段
    pub fn event_channel_capacity(self, cap: usize) -> Self; // default 64(细化设计草图中的 stdout_channel_capacity 命名)
    pub fn timeout(self, d: std::time::Duration) -> Self;   // 到期整树终止
    pub fn pipe_stdin(self, pipe: bool) -> Self;            // default false
    pub fn pid_file(self, path: impl Into<std::path::PathBuf>) -> Self;
    // spawn()/output() 由 Task 5/7 追加
}
```

- [ ] **Step 1: 写失败测试(builder 默认值与链式设置)**

`command.rs` 底部:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn defaults_match_design() {
        let c = Command::new("prog");
        assert_eq!(c.event_channel_capacity, 64);
        assert_eq!(c.kill_grace, Duration::from_secs(5));
        assert!(c.hide_window);
        assert!(!c.pipe_stdin);
        assert!(c.encoding.is_none());
        assert!(c.pid_file.is_none());
        assert!(c.timeout.is_none());
    }

    #[test]
    fn builder_chain_sets_fields() {
        let c = Command::new("prog")
            .arg("-v")
            .args(["a", "b"])
            .env("K", "V")
            .current_dir("C:/tmp")
            .kill_grace(Duration::from_secs(1))
            .event_channel_capacity(8)
            .pipe_stdin(true)
            .hide_window(false);
        assert_eq!(c.args.len(), 3);
        assert_eq!(c.envs.len(), 1);
        assert_eq!(c.event_channel_capacity, 8);
        assert!(c.pipe_stdin);
        assert!(!c.hide_window);
    }
}
```

- [ ] **Step 2: 跑测试确认编译失败(类型不存在)**

```powershell
cargo test -p nyanpasu-utils --features process command
```
Expected: FAIL(`Command` 未定义)。

- [ ] **Step 3: 最小实现**

`command.rs`:

```rust
use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
    time::Duration,
};

/// Builder for spawning a managed child process. See module docs for the contract.
pub struct Command {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) envs: Vec<(OsString, OsString)>,
    pub(crate) current_dir: Option<PathBuf>,
    pub(crate) encoding: Option<&'static encoding_rs::Encoding>,
    pub(crate) hide_window: bool,
    pub(crate) kill_grace: Duration,
    pub(crate) event_channel_capacity: usize,
    pub(crate) timeout: Option<Duration>,
    pub(crate) pipe_stdin: bool,
    pub(crate) pid_file: Option<PathBuf>,
}

impl Command {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            envs: Vec::new(),
            current_dir: None,
            encoding: None,
            hide_window: true,
            kill_grace: Duration::from_secs(5),
            event_channel_capacity: 64,
            timeout: None,
            pipe_stdin: false,
            pid_file: None,
        }
    }

    pub fn arg(mut self, a: impl AsRef<OsStr>) -> Self {
        self.args.push(a.as_ref().to_os_string());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args.extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
        self
    }

    pub fn env(mut self, k: impl AsRef<OsStr>, v: impl AsRef<OsStr>) -> Self {
        self.envs.push((k.as_ref().to_os_string(), v.as_ref().to_os_string()));
        self
    }

    pub fn current_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(dir.into());
        self
    }

    pub fn encoding(mut self, enc: Option<&'static encoding_rs::Encoding>) -> Self {
        self.encoding = enc;
        self
    }

    pub fn hide_window(mut self, hide: bool) -> Self {
        self.hide_window = hide;
        self
    }

    pub fn kill_grace(mut self, d: Duration) -> Self {
        self.kill_grace = d;
        self
    }

    pub fn event_channel_capacity(mut self, cap: usize) -> Self {
        self.event_channel_capacity = cap.max(1);
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = Some(d);
        self
    }

    pub fn pipe_stdin(mut self, pipe: bool) -> Self {
        self.pipe_stdin = pipe;
        self
    }

    pub fn pid_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.pid_file = Some(path.into());
        self
    }
}
```

`mod.rs` 增加:

```rust
mod command;
pub use command::Command;
```

- [ ] **Step 4: 跑测试确认通过**

```powershell
cargo test -p nyanpasu-utils --features process command
```
Expected: 2 passed。

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-utils/src/process
git commit -m "feat(utils/process): add Command builder with design defaults"
```

---

### Task 4: 测试辅助子进程 `nyanpasu-test-child`

**Files:**
- Modify: `crates/nyanpasu-utils/Cargo.toml`([[bin]])
- Create: `crates/nyanpasu-utils/tests/helpers/test_child.rs`
- Create: `crates/nyanpasu-utils/tests/process_helper_smoke.rs`

**Interfaces:**
- Produces(后续集成测试依赖):bin `nyanpasu-test-child`,模式:
  `exit-with <code>` / `echo-lines <a> <b> ...`(stdout 逐行 + stderr 一行 `stderr-marker`)/ `spam-stdout <n>`(`line-0..line-{n-1}`)/ `sleep-forever`(先打印 `ready`)/ `trap-term`(Unix:捕 SIGTERM 后打印 `got-term` 退出 0;Windows:等同 sleep-forever)/ `spawn-grandchild`(打印 `grandchild-pid:<pid>` 后长眠)/ `gbk-stdout`(输出 GBK 编码的"中文"+换行)/ `echo-stdin`(读一行,回显 `echo:<line>`)。
- 测试内取路径:`env!("CARGO_BIN_EXE_nyanpasu-test-child")`。

- [ ] **Step 1: Cargo.toml 声明 bin(无 required-features,保证 CARGO_BIN_EXE 可靠生成;仅依赖 std+tokio)**

```toml
[[bin]]
name = "nyanpasu-test-child"
path = "tests/helpers/test_child.rs"
doc = false
```

- [ ] **Step 2: 写 smoke 测试(先失败)**

`crates/nyanpasu-utils/tests/process_helper_smoke.rs`:

```rust
//! Sanity checks for the test helper binary itself (std::process, no process feature needed).

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

#[test]
fn exit_with_propagates_code() {
    let st = std::process::Command::new(child()).args(["exit-with", "3"]).status().unwrap();
    assert_eq!(st.code(), Some(3));
}

#[test]
fn echo_lines_writes_stdout_and_stderr() {
    let out = std::process::Command::new(child()).args(["echo-lines", "a", "b"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["a", "b"]);
    assert!(String::from_utf8(out.stderr).unwrap().contains("stderr-marker"));
}

#[test]
fn gbk_stdout_emits_expected_bytes() {
    let out = std::process::Command::new(child()).args(["gbk-stdout"]).output().unwrap();
    assert_eq!(&out.stdout[..4], &[0xD6, 0xD0, 0xCE, 0xC4]);
}
```

- [ ] **Step 3: 跑测试确认失败**

```powershell
cargo test -p nyanpasu-utils --test process_helper_smoke
```
Expected: FAIL(bin 不存在,编译错误)。

- [ ] **Step 4: 实现 helper bin**

`crates/nyanpasu-utils/tests/helpers/test_child.rs`:

```rust
//! Test helper child for nyanpasu-utils process-module integration tests.
//! Not a production binary. Modes documented in the implementation plan.

use std::io::Write;
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_default();
    match mode.as_str() {
        "exit-with" => {
            let code: i32 = args.next().expect("code").parse().expect("i32");
            std::process::exit(code);
        }
        "echo-lines" => {
            for a in args {
                println!("{a}");
            }
            eprintln!("stderr-marker");
        }
        "spam-stdout" => {
            let n: usize = args.next().expect("n").parse().expect("usize");
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            for i in 0..n {
                writeln!(lock, "line-{i}").expect("write");
            }
        }
        "sleep-forever" => {
            println!("ready");
            sleep_forever().await;
        }
        "trap-term" => {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{SignalKind, signal};
                let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
                println!("ready");
                term.recv().await;
                println!("got-term");
                std::process::exit(0);
            }
            #[cfg(not(unix))]
            {
                println!("ready");
                sleep_forever().await;
            }
        }
        "spawn-grandchild" => {
            let exe = std::env::current_exe().expect("current_exe");
            let child = std::process::Command::new(exe)
                .arg("sleep-forever")
                .stdout(std::process::Stdio::null())
                .spawn()
                .expect("spawn grandchild");
            println!("grandchild-pid:{}", child.id());
            sleep_forever().await;
        }
        "gbk-stdout" => {
            // "中文" encoded as GBK, plus newline
            let bytes = [0xD6u8, 0xD0, 0xCE, 0xC4, b'\n'];
            std::io::stdout().write_all(&bytes).expect("write gbk");
        }
        "echo-stdin" => {
            let mut line = String::new();
            std::io::stdin().read_line(&mut line).expect("read line");
            print!("echo:{line}");
        }
        other => {
            eprintln!("unknown mode: {other}");
            std::process::exit(2);
        }
    }
}

async fn sleep_forever() -> ! {
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
```

- [ ] **Step 5: 跑测试确认通过**

```powershell
cargo test -p nyanpasu-utils --test process_helper_smoke
```
Expected: 3 passed。

- [ ] **Step 6: Commit**

```powershell
git add crates/nyanpasu-utils
git commit -m "test(utils/process): add nyanpasu-test-child helper binary with smoke tests"
```

---

### Task 5: 引擎 spawn + 事件泵 + `Command::spawn()` + `ProcessHandle`(pid/wait/containment)

**Files:**
- Create: `crates/nyanpasu-utils/src/process/engine.rs`
- Create: `crates/nyanpasu-utils/src/process/handle.rs`
- Modify: `crates/nyanpasu-utils/src/process/command.rs`(追加 `spawn`)
- Modify: `crates/nyanpasu-utils/src/process/mod.rs`
- Create: `crates/nyanpasu-utils/tests/process_spawn.rs`

**Interfaces:**
- Consumes: Task 1 类型、Task 3 `Command` 字段、Task 2 侦察答案、Task 4 helper。
- Produces:

```rust
// handle.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Containment { JobObject, CgroupV2, ProcessGroup }

#[derive(Clone)]
pub struct ProcessHandle { /* private */ }
impl ProcessHandle {
    pub fn pid(&self) -> u32;
    pub fn containment(&self) -> Containment;
    pub async fn wait(&self) -> Result<TerminatedPayload, ProcessError>;
    // graceful_kill/kill 在 Task 6 追加,write_stdin 在 Task 8 追加
}

// command.rs
impl Command {
    pub async fn spawn(self) -> Result<(ProcessHandle, tokio::sync::mpsc::Receiver<ProcessEvent>), ProcessError>;
}

// engine.rs(crate 内部)
pub(crate) enum Ctrl {
    GracefulKill(tokio::sync::oneshot::Sender<Result<(), ProcessError>>),
    Kill(tokio::sync::oneshot::Sender<Result<(), ProcessError>>),
    WriteStdin(Vec<u8>, tokio::sync::oneshot::Sender<Result<(), ProcessError>>),
}
pub(crate) struct SpawnParts {
    pub pid: u32,
    pub containment: Containment,
    pub ctrl_tx: tokio::sync::mpsc::Sender<Ctrl>,
    pub terminated_rx: tokio::sync::watch::Receiver<Option<TerminatedPayload>>,
    pub events_rx: tokio::sync::mpsc::Receiver<ProcessEvent>,
}
pub(crate) async fn spawn(cmd: Command) -> Result<SpawnParts, ProcessError>;
```

**引擎接线规则**:公开 API 与本任务/后续任务的集成测试是合同;`engine.rs` 内部与 processkit 的对接方式(流是否可分离、控制句柄形态)按 Task 2 侦察答案调整,允许偏离下面的示意代码,但**不允许**改公开签名、事件顺序合同或引入 OS 线程。

- [ ] **Step 1: 写失败的集成测试**

`crates/nyanpasu-utils/tests/process_spawn.rs`:

```rust
#![cfg(feature = "process")]

use nyanpasu_utils::process::{Command, ProcessEvent};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

async fn collect_all(mut rx: tokio::sync::mpsc::Receiver<ProcessEvent>) -> Vec<ProcessEvent> {
    let mut evs = Vec::new();
    while let Some(e) = rx.recv().await {
        evs.push(e);
    }
    evs
}

#[tokio::test]
async fn stdout_stderr_then_terminated_last() {
    let (handle, rx) = Command::new(child())
        .args(["echo-lines", "hello", "world"])
        .spawn()
        .await
        .unwrap();
    assert!(handle.pid() > 0);
    let evs = collect_all(rx).await;

    let stdout: Vec<_> = evs
        .iter()
        .filter_map(|e| match e {
            ProcessEvent::Stdout(l) => Some(l.trim_end().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(stdout, vec!["hello", "world"]);
    assert!(evs.iter().any(|e| matches!(e, ProcessEvent::Stderr(l) if l.contains("stderr-marker"))));
    // contract: Terminated is the FINAL event
    assert!(matches!(evs.last().unwrap(), ProcessEvent::Terminated(p) if p.code == Some(0)));
    let payload = handle.wait().await.unwrap();
    assert_eq!(payload.code, Some(0));
}

#[tokio::test]
async fn nonzero_exit_code_is_reported() {
    let (handle, rx) = Command::new(child()).args(["exit-with", "3"]).spawn().await.unwrap();
    let evs = collect_all(rx).await;
    assert!(matches!(evs.last().unwrap(), ProcessEvent::Terminated(p) if p.code == Some(3)));
    assert_eq!(handle.wait().await.unwrap().code, Some(3));
}

#[tokio::test]
async fn spam_10k_lines_no_loss_with_default_capacity() {
    let (_handle, rx) = Command::new(child()).args(["spam-stdout", "10000"]).spawn().await.unwrap();
    let evs = collect_all(rx).await;
    let n = evs.iter().filter(|e| matches!(e, ProcessEvent::Stdout(_))).count();
    assert_eq!(n, 10000);
    assert!(matches!(evs.last().unwrap(), ProcessEvent::Terminated(_)));
}

#[tokio::test]
async fn spawn_missing_program_is_error() {
    let err = Command::new("definitely-not-a-real-binary-42").spawn().await.err().unwrap();
    let msg = err.to_string();
    assert!(!msg.is_empty());
}

#[tokio::test]
async fn containment_matches_platform() {
    use nyanpasu_utils::process::Containment;
    let (handle, rx) = Command::new(child()).args(["exit-with", "0"]).spawn().await.unwrap();
    let c = handle.containment();
    #[cfg(windows)]
    assert_eq!(c, Containment::JobObject);
    #[cfg(target_os = "linux")]
    assert!(matches!(c, Containment::CgroupV2 | Containment::ProcessGroup));
    #[cfg(all(unix, not(target_os = "linux")))]
    assert_eq!(c, Containment::ProcessGroup);
    collect_all(rx).await;
}
```

- [ ] **Step 2: 跑测试确认编译失败**

```powershell
cargo test -p nyanpasu-utils --features process --test process_spawn
```
Expected: FAIL(`spawn`/`ProcessHandle` 不存在)。

- [ ] **Step 3: 实现 handle.rs**

```rust
use tokio::sync::{mpsc, oneshot, watch};

use super::{error::ProcessError, event::TerminatedPayload};

/// The kernel containment mechanism actually in effect (mirrors the engine's report).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Containment {
    JobObject,
    CgroupV2,
    ProcessGroup,
}

pub(crate) enum Ctrl {
    GracefulKill(oneshot::Sender<Result<(), ProcessError>>),
    Kill(oneshot::Sender<Result<(), ProcessError>>),
    WriteStdin(Vec<u8>, oneshot::Sender<Result<(), ProcessError>>),
}

/// Cloneable handle to a spawned child. Dropping ALL handles and the event
/// receiver kills the whole process tree (kill-on-drop, see design §5.6).
#[derive(Clone)]
pub struct ProcessHandle {
    pub(crate) pid: u32,
    pub(crate) containment: Containment,
    pub(crate) ctrl: mpsc::Sender<Ctrl>,
    pub(crate) terminated: watch::Receiver<Option<TerminatedPayload>>,
}

impl ProcessHandle {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn containment(&self) -> Containment {
        self.containment
    }

    /// Waits until the child terminates; returns immediately if it already has.
    pub async fn wait(&self) -> Result<TerminatedPayload, ProcessError> {
        let mut rx = self.terminated.clone();
        loop {
            if let Some(p) = rx.borrow().clone() {
                return Ok(p);
            }
            rx.changed()
                .await
                .map_err(|_| ProcessError::Engine("process pump task dropped".into()))?;
        }
    }

    pub(crate) async fn send_ctrl(
        &self,
        make: impl FnOnce(oneshot::Sender<Result<(), ProcessError>>) -> Ctrl,
    ) -> Result<(), ProcessError> {
        let (tx, rx) = oneshot::channel();
        if self.ctrl.send(make(tx)).await.is_err() {
            // Pump ended: the process is already gone. Kill-type ops are idempotent-Ok.
            return if self.terminated.borrow().is_some() {
                Ok(())
            } else {
                Err(ProcessError::AlreadyExited)
            };
        }
        rx.await.map_err(|_| ProcessError::AlreadyExited)?
    }
}
```

- [ ] **Step 4: 实现 engine.rs(示意代码;按 Task 2 侦察答案微调对接点)**

```rust
//! processkit adapter. The ONLY file in src/process allowed to `use processkit`.

use std::time::Duration;

use tokio::sync::{mpsc, watch};

use super::{
    command::Command,
    error::ProcessError,
    event::{ProcessEvent, TerminatedPayload},
    handle::{Containment, Ctrl},
};

pub(crate) struct SpawnParts {
    pub pid: u32,
    pub containment: Containment,
    pub ctrl_tx: mpsc::Sender<Ctrl>,
    pub terminated_rx: watch::Receiver<Option<TerminatedPayload>>,
    pub events_rx: mpsc::Receiver<ProcessEvent>,
}

fn build_pk(cmd: &Command) -> processkit::Command {
    let mut pk = processkit::Command::new(&cmd.program);
    pk = pk.args(&cmd.args);
    for (k, v) in &cmd.envs {
        pk = pk.env(k, v);
    }
    if let Some(dir) = &cmd.current_dir {
        pk = pk.current_dir(dir);
    }
    if let Some(enc) = cmd.encoding {
        pk = pk.encoding(enc);
    }
    if let Some(t) = cmd.timeout {
        pk = pk.timeout(t);
    }
    #[cfg(windows)]
    if cmd.hide_window {
        pk = pk.create_no_window();
    }
    // Bounded ring buffer — hard requirement from design §5.5 (audit A-05).
    pk = pk.output_buffer(processkit::OutputBufferPolicy::unbounded().with_max_bytes(256 * 1024));
    if cmd.pipe_stdin {
        pk = pk.keep_stdin_open();
    }
    pk
}

fn map_containment(m: processkit::Mechanism) -> Containment {
    match m {
        processkit::Mechanism::JobObject => Containment::JobObject,
        processkit::Mechanism::CgroupV2 => Containment::CgroupV2,
        _ => Containment::ProcessGroup,
    }
}

pub(crate) async fn spawn(cmd: Command) -> Result<SpawnParts, ProcessError> {
    let program_label = cmd.program.to_string_lossy().into_owned();
    let kill_grace = cmd.kill_grace;
    let capacity = cmd.event_channel_capacity;

    let mut run = build_pk(&cmd).start().await.map_err(|e| ProcessError::Spawn {
        program: program_label,
        message: e.to_string(),
    })?;

    let pid = run.pid();
    let containment = map_containment(run.mechanism());

    let (ev_tx, ev_rx) = mpsc::channel::<ProcessEvent>(capacity);
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<Ctrl>(8);
    let (term_tx, term_rx) = watch::channel::<Option<TerminatedPayload>>(None);

    tokio::spawn(async move {
        let mut events = match run.output_events() {
            Ok(s) => s,
            Err(e) => {
                let _ = ev_tx.send(ProcessEvent::Error(e.to_string())).await;
                return;
            }
        };
        let mut hard_kill_at: Option<tokio::time::Instant> = None;

        loop {
            tokio::select! {
                biased;
                Some(c) = ctrl_rx.recv() => match c {
                    Ctrl::Kill(reply) => {
                        let _ = reply.send(run.kill().await.map_err(|e| ProcessError::Engine(e.to_string())));
                    }
                    Ctrl::GracefulKill(reply) => {
                        #[cfg(unix)]
                        {
                            // SIGTERM now; escalate to hard kill after the grace window
                            let r = run.signal(processkit::Signal::Term).map_err(|e| ProcessError::Engine(e.to_string()));
                            if r.is_ok() {
                                hard_kill_at = Some(tokio::time::Instant::now() + kill_grace);
                            }
                            let _ = reply.send(r);
                        }
                        #[cfg(windows)]
                        {
                            // Design §5.6: no graceful tier on Windows — equivalent to kill().
                            let _ = kill_grace; // documented no-op on this platform
                            let _ = reply.send(run.kill().await.map_err(|e| ProcessError::Engine(e.to_string())));
                        }
                    }
                    Ctrl::WriteStdin(data, reply) => {
                        let _ = reply.send(run.write_stdin(&data).await.map_err(|_| ProcessError::StdinUnavailable));
                    }
                },
                _ = async {
                    match hard_kill_at {
                        Some(at) => tokio::time::sleep_until(at).await,
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    hard_kill_at = None;
                    let _ = run.kill().await;
                }
                maybe_ev = events.next() => match maybe_ev {
                    Some(processkit::OutputEvent::Stdout(l)) => {
                        let _ = ev_tx.send(ProcessEvent::Stdout(l.into_text())).await;
                    }
                    Some(processkit::OutputEvent::Stderr(l)) => {
                        let _ = ev_tx.send(ProcessEvent::Stderr(l.into_text())).await;
                    }
                    None => break, // both streams closed -> child is exiting
                },
            }
        }

        // Streams drained; now settle the exit status. Terminated is ALWAYS last.
        let payload = match run.finish().await {
            Ok(fin) => finished_to_payload(&fin),
            Err(e) => {
                let _ = ev_tx.send(ProcessEvent::Error(e.to_string())).await;
                TerminatedPayload { code: None, signal: None }
            }
        };
        let _ = term_tx.send(Some(payload.clone()));
        let _ = ev_tx.send(ProcessEvent::Terminated(payload)).await;
        // ctrl_rx dropped here -> pending/late ctrl calls resolve via ProcessHandle::send_ctrl fallback
    });

    Ok(SpawnParts { pid, containment, ctrl_tx, terminated_rx: term_rx, events_rx: ev_rx })
}

fn finished_to_payload(fin: &processkit::Finished) -> TerminatedPayload {
    // Map processkit Outcome to (code, signal). Adjust per recon: Outcome variants
    // are Exited(i32) / Signaled(i32) / Killed / TimedOut-like.
    match &fin.outcome {
        processkit::Outcome::Exited(code) => TerminatedPayload { code: Some(*code), signal: None },
        other => TerminatedPayload { code: None, signal: outcome_signal(other) },
    }
}

#[cfg(unix)]
fn outcome_signal(o: &processkit::Outcome) -> Option<i32> {
    // recon: extract signal number if the variant carries one
    let _ = o;
    Some(9)
}

#[cfg(windows)]
fn outcome_signal(_: &processkit::Outcome) -> Option<i32> {
    None
}
```

`command.rs` 追加:

```rust
impl Command {
    /// Spawns the child. Returns a cloneable handle plus the event channel.
    /// `ProcessEvent::Terminated` is guaranteed to be the final event.
    pub async fn spawn(
        self,
    ) -> Result<(super::handle::ProcessHandle, tokio::sync::mpsc::Receiver<super::event::ProcessEvent>), super::error::ProcessError>
    {
        let parts = super::engine::spawn(self).await?;
        let handle = super::handle::ProcessHandle {
            pid: parts.pid,
            containment: parts.containment,
            ctrl: parts.ctrl_tx,
            terminated: parts.terminated_rx,
        };
        Ok((handle, parts.events_rx))
    }
}
```

`mod.rs` 增加:

```rust
mod engine;
mod handle;
pub use handle::{Containment, ProcessHandle};
```

- [ ] **Step 5: 跑测试直到通过(此步吸收 recon 修正)**

```powershell
cargo test -p nyanpasu-utils --features process --test process_spawn
```
Expected: 5 passed。若 processkit 方法名/所有权与示意不符,按 Task 2 答案修 engine.rs,公开 API 不动。

- [ ] **Step 6: 验收扫描 + Commit**

```powershell
# 封装红线:processkit 只出现在 engine.rs 与 example
Select-String -Path crates/nyanpasu-utils/src/process/*.rs -Pattern "processkit" | Where-Object { $_.Filename -ne "engine.rs" }
# 期望:无输出
Select-String -Path crates/nyanpasu-utils/src/process/*.rs -Pattern "std::thread::spawn"
# 期望:无输出
git add crates/nyanpasu-utils/src/process crates/nyanpasu-utils/tests
git commit -m "feat(utils/process): engine spawn + event pump + ProcessHandle (pid/wait/containment)"
```

---

### Task 6: `kill()` / `graceful_kill()` + 整树清理测试

**Files:**
- Modify: `crates/nyanpasu-utils/src/process/handle.rs`
- Create: `crates/nyanpasu-utils/tests/process_kill.rs`

**Interfaces:**
- Consumes: Task 5 的 `Ctrl` / `send_ctrl` / engine 控制分支(已实现)。
- Produces:

```rust
impl ProcessHandle {
    /// Unix: SIGTERM → kill_grace → SIGKILL (whole tree). Windows: identical to kill() (design §5.6).
    /// Returns after the process has fully terminated. Idempotent on dead processes.
    pub async fn graceful_kill(&self) -> Result<(), ProcessError>;
    /// Hard-kills the whole tree. Returns after termination. Idempotent.
    pub async fn kill(&self) -> Result<(), ProcessError>;
}
```

- [ ] **Step 1: 写失败测试**

`crates/nyanpasu-utils/tests/process_kill.rs`:

```rust
#![cfg(feature = "process")]

use std::time::Duration;

use nyanpasu_utils::process::{Command, ProcessEvent};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

fn pid_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};
    let kind = RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing());
    let mut s = System::new_with_specifics(kind);
    s.refresh_specifics(kind);
    s.process(Pid::from_u32(pid)).is_some()
}

#[tokio::test]
async fn kill_terminates_and_wait_returns() {
    let (handle, mut rx) = Command::new(child()).args(["sleep-forever"]).spawn().await.unwrap();
    // wait for "ready" so we know it's running
    loop {
        match rx.recv().await.unwrap() {
            ProcessEvent::Stdout(l) if l.contains("ready") => break,
            ProcessEvent::Terminated(_) => panic!("exited early"),
            _ => {}
        }
    }
    handle.kill().await.unwrap();
    let payload = handle.wait().await.unwrap();
    assert_ne!(payload.code, Some(0)); // hard kill is never a clean exit
    // killing again is idempotent-Ok
    handle.kill().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn graceful_kill_delivers_sigterm_first() {
    let (handle, mut rx) = Command::new(child()).args(["trap-term"]).spawn().await.unwrap();
    loop {
        match rx.recv().await.unwrap() {
            ProcessEvent::Stdout(l) if l.contains("ready") => break,
            ProcessEvent::Terminated(_) => panic!("exited early"),
            _ => {}
        }
    }
    handle.graceful_kill().await.unwrap();
    // trap-term exits 0 on SIGTERM -> proves the graceful tier was delivered
    assert_eq!(handle.wait().await.unwrap().code, Some(0));
}

#[cfg(windows)]
#[tokio::test]
async fn graceful_kill_equals_kill_on_windows() {
    let (handle, _rx) = Command::new(child()).args(["sleep-forever"]).spawn().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    handle.graceful_kill().await.unwrap();
    assert_ne!(handle.wait().await.unwrap().code, Some(0));
}

#[tokio::test]
async fn whole_tree_is_reaped() {
    let (handle, mut rx) = Command::new(child()).args(["spawn-grandchild"]).spawn().await.unwrap();
    let grandchild_pid: u32 = loop {
        match rx.recv().await.unwrap() {
            ProcessEvent::Stdout(l) if l.contains("grandchild-pid:") => {
                break l.trim().trim_start_matches("grandchild-pid:").parse().unwrap();
            }
            ProcessEvent::Terminated(_) => panic!("exited early"),
            _ => {}
        }
    };
    assert!(pid_alive(grandchild_pid));
    handle.kill().await.unwrap();
    handle.wait().await.unwrap();
    // the kernel object must have reaped the grandchild too
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!pid_alive(grandchild_pid), "grandchild survived tree kill");
}
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
cargo test -p nyanpasu-utils --features process --test process_kill
```
Expected: FAIL(方法不存在)。

- [ ] **Step 3: handle.rs 追加实现**

```rust
impl ProcessHandle {
    pub async fn graceful_kill(&self) -> Result<(), ProcessError> {
        self.send_ctrl(Ctrl::GracefulKill).await?;
        self.wait().await?;
        Ok(())
    }

    pub async fn kill(&self) -> Result<(), ProcessError> {
        self.send_ctrl(Ctrl::Kill).await?;
        self.wait().await?;
        Ok(())
    }
}
```

注意:`send_ctrl` 在泵任务已结束(进程已死)时返回 `Ok(())`(Task 5 已实现该回退),满足幂等语义。

- [ ] **Step 4: 跑测试确认通过(Windows 本机跑 3 个,unix 用例由 CI 覆盖)**

```powershell
cargo test -p nyanpasu-utils --features process --test process_kill
```
Expected: 本机 3 passed(`graceful_kill_delivers_sigterm_first` 被 cfg 排除)。

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-utils
git commit -m "feat(utils/process): kill and graceful_kill with whole-tree reap tests"
```

---

### Task 7: `Command::output()` 一次性捕获

**Files:**
- Modify: `crates/nyanpasu-utils/src/process/engine.rs`(追加 `run_capture`)
- Modify: `crates/nyanpasu-utils/src/process/command.rs`(追加 `output`)
- Create: `crates/nyanpasu-utils/tests/process_output.rs`

**Interfaces:**
- Produces: `impl Command { pub async fn output(self) -> Result<ProcessOutput, ProcessError>; }`
  语义:非零退出是**数据**不是错误;超时(设置了 `timeout` 且到期)→ `Err(ProcessError::Timeout { after })`。

- [ ] **Step 1: 写失败测试**

`crates/nyanpasu-utils/tests/process_output.rs`:

```rust
#![cfg(feature = "process")]

use std::time::Duration;

use nyanpasu_utils::process::{Command, ProcessError};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

#[tokio::test]
async fn output_captures_streams_and_code() {
    let out = Command::new(child()).args(["echo-lines", "x"]).output().await.unwrap();
    assert!(out.success());
    assert_eq!(out.stdout.trim(), "x");
    assert!(out.stderr.contains("stderr-marker"));
}

#[tokio::test]
async fn output_nonzero_is_data_not_error() {
    let out = Command::new(child()).args(["exit-with", "5"]).output().await.unwrap();
    assert!(!out.success());
    assert_eq!(out.code, Some(5));
}

#[tokio::test]
async fn output_timeout_is_error() {
    let err = Command::new(child())
        .args(["sleep-forever"])
        .timeout(Duration::from_millis(300))
        .output()
        .await
        .err()
        .expect("must time out");
    assert!(matches!(err, ProcessError::Timeout { .. }));
}
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
cargo test -p nyanpasu-utils --features process --test process_output
```
Expected: FAIL(`output` 不存在)。

- [ ] **Step 3: 实现**

`engine.rs` 追加(用 processkit 的一次性捕获 verb,字段名按 Task 2 侦察答案第 5 条校正):

```rust
pub(crate) async fn run_capture(cmd: Command) -> Result<super::error::ProcessOutput, ProcessError> {
    let program_label = cmd.program.to_string_lossy().into_owned();
    let timeout = cmd.timeout;
    let res = build_pk(&cmd).output_string().await.map_err(|e| ProcessError::Spawn {
        program: program_label,
        message: e.to_string(),
    })?;
    if res.timed_out {
        return Err(ProcessError::Timeout { after: timeout.unwrap_or_default() });
    }
    Ok(super::error::ProcessOutput { code: res.code, stdout: res.stdout, stderr: res.stderr })
}
```

`command.rs` 追加:

```rust
impl Command {
    /// One-shot run capturing stdout/stderr. A non-zero exit is data, not an error;
    /// only spawn failures and timeouts are `Err`.
    pub async fn output(self) -> Result<super::error::ProcessOutput, super::error::ProcessError> {
        super::engine::run_capture(self).await
    }
}
```

- [ ] **Step 4: 跑测试确认通过**

```powershell
cargo test -p nyanpasu-utils --features process --test process_output
```
Expected: 3 passed。

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-utils
git commit -m "feat(utils/process): one-shot Command::output with timeout-as-error semantics"
```

---

### Task 8: `write_stdin`

**Files:**
- Modify: `crates/nyanpasu-utils/src/process/handle.rs`
- Create: `crates/nyanpasu-utils/tests/process_stdin.rs`

**Interfaces:**
- Produces: `impl ProcessHandle { pub async fn write_stdin(&self, data: &[u8]) -> Result<(), ProcessError>; }`
  前置:`Command::pipe_stdin(true)`;否则 `Err(StdinUnavailable)`。

- [ ] **Step 1: 写失败测试**

`crates/nyanpasu-utils/tests/process_stdin.rs`:

```rust
#![cfg(feature = "process")]

use nyanpasu_utils::process::{Command, ProcessError, ProcessEvent};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

#[tokio::test]
async fn write_stdin_roundtrip() {
    let (handle, mut rx) = Command::new(child())
        .args(["echo-stdin"])
        .pipe_stdin(true)
        .spawn()
        .await
        .unwrap();
    handle.write_stdin(b"ping\n").await.unwrap();
    let mut echoed = None;
    while let Some(e) = rx.recv().await {
        if let ProcessEvent::Stdout(l) = e {
            echoed = Some(l);
            break;
        }
    }
    assert_eq!(echoed.unwrap().trim(), "echo:ping");
}

#[tokio::test]
async fn write_stdin_without_pipe_is_error() {
    let (handle, _rx) = Command::new(child()).args(["sleep-forever"]).spawn().await.unwrap();
    let err = handle.write_stdin(b"x").await.err().unwrap();
    assert!(matches!(err, ProcessError::StdinUnavailable));
    handle.kill().await.unwrap();
}
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
cargo test -p nyanpasu-utils --features process --test process_stdin
```
Expected: FAIL。

- [ ] **Step 3: 实现**

`handle.rs` 追加(engine 的 `Ctrl::WriteStdin` 分支 Task 5 已备好;未启用 pipe_stdin 时 engine 分支返回 `StdinUnavailable`——在 engine.rs 的 WriteStdin 分支加一个 `cmd.pipe_stdin` 快照判断,spawn 时把该布尔存入泵任务):

```rust
impl ProcessHandle {
    pub async fn write_stdin(&self, data: &[u8]) -> Result<(), ProcessError> {
        let data = data.to_vec();
        self.send_ctrl(move |reply| Ctrl::WriteStdin(data, reply)).await
    }
}
```

注意 `send_ctrl` 的闭包签名已兼容(Task 5 定义为 `impl FnOnce(...) -> Ctrl`)。

- [ ] **Step 4: 跑测试确认通过**

```powershell
cargo test -p nyanpasu-utils --features process --test process_stdin
```
Expected: 2 passed。

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-utils
git commit -m "feat(utils/process): stdin piping via ProcessHandle::write_stdin"
```

---

### Task 9: 编码支持(GBK)

**Files:**
- Create: `crates/nyanpasu-utils/tests/process_encoding.rs`
- (如 recon 无出入,engine.rs 无需改动——`encoding` 已在 Task 5 的 `build_pk` 中接线)

**Interfaces:**
- Consumes: `Command::encoding(Some(encoding_rs::GBK))`。

- [ ] **Step 1: 写测试**

`crates/nyanpasu-utils/tests/process_encoding.rs`:

```rust
#![cfg(feature = "process")]

use nyanpasu_utils::process::{Command, ProcessEvent};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

#[tokio::test]
async fn gbk_stdout_decodes_correctly() {
    let (_handle, mut rx) = Command::new(child())
        .args(["gbk-stdout"])
        .encoding(Some(encoding_rs::GBK))
        .spawn()
        .await
        .unwrap();
    let mut decoded = None;
    while let Some(e) = rx.recv().await {
        if let ProcessEvent::Stdout(l) = e {
            decoded = Some(l);
            break;
        }
    }
    assert_eq!(decoded.unwrap().trim(), "中文");
}
```

说明:`encoding_rs` 无需加进 dev-dependencies——它是 process feature 激活的可选依赖,包内测试目标可直接 `use encoding_rs`。

- [ ] **Step 2: 跑测试;若失败按 recon 修 `build_pk` 的 encoding 接线(`encoding` vs `stdout_encoding`+`stderr_encoding`)**

```powershell
cargo test -p nyanpasu-utils --features process --test process_encoding
```
Expected: 1 passed。

- [ ] **Step 3: Commit**

```powershell
git add crates/nyanpasu-utils
git commit -m "test(utils/process): GBK output decoding via Command::encoding"
```

---

### Task 10: pid 文件生命周期

**Files:**
- Create: `crates/nyanpasu-utils/src/process/pid_file.rs`
- Modify: `crates/nyanpasu-utils/src/process/engine.rs`(spawn 前 prepare、spawn 后 write、泵尾 cleanup)
- Modify: `crates/nyanpasu-utils/src/process/mod.rs`(`mod pid_file;`)
- Create: `crates/nyanpasu-utils/tests/process_pid_file.rs`

**Interfaces:**
- Consumes: `crate::os::{create_pid_file, kill_by_pid_file}`(feature `process` 已含 `os`)。
- Produces(crate 内部):

```rust
pub(crate) struct PidFileGuard { path: std::path::PathBuf }
impl PidFileGuard {
    /// Kill any residual process recorded in the pid file (validated against
    /// `expected_exe`, the spawned program's file name), then take ownership of the path.
    pub(crate) async fn prepare(path: std::path::PathBuf, expected_exe: Option<String>) -> std::io::Result<Self>;
    pub(crate) async fn write(&self, pid: u32) -> std::io::Result<()>;
    /// Best-effort removal; never fails the pump.
    pub(crate) async fn cleanup(&self);
}
```

- [ ] **Step 1: 写失败测试**

`crates/nyanpasu-utils/tests/process_pid_file.rs`:

```rust
#![cfg(feature = "process")]

use nyanpasu_utils::process::{Command, ProcessEvent};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

fn pid_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};
    let kind = RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing());
    let mut s = System::new_with_specifics(kind);
    s.refresh_specifics(kind);
    s.process(Pid::from_u32(pid)).is_some()
}

#[tokio::test]
async fn pid_file_written_and_cleaned_up() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core.pid");
    let (handle, rx) = Command::new(child())
        .args(["exit-with", "0"])
        .pid_file(&pid_path)
        .spawn()
        .await
        .unwrap();
    // pid file exists right after spawn and contains the pid
    let content: u32 = std::fs::read_to_string(&pid_path).unwrap().trim().parse().unwrap();
    assert_eq!(content, handle.pid());
    // drain to termination -> file removed
    let mut rx = rx;
    while rx.recv().await.is_some() {}
    assert!(!pid_path.exists());
}

#[tokio::test]
async fn residual_process_is_killed_before_spawn() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core.pid");

    let (h1, mut rx1) = Command::new(child())
        .args(["sleep-forever"])
        .pid_file(&pid_path)
        .spawn()
        .await
        .unwrap();
    loop {
        match rx1.recv().await.unwrap() {
            ProcessEvent::Stdout(l) if l.contains("ready") => break,
            ProcessEvent::Terminated(_) => panic!("exited early"),
            _ => {}
        }
    }
    let old_pid = h1.pid();

    // second spawn with the same pid file must kill the residual first
    let (h2, _rx2) = Command::new(child())
        .args(["sleep-forever"])
        .pid_file(&pid_path)
        .spawn()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(!pid_alive(old_pid), "residual process not killed");
    let content: u32 = std::fs::read_to_string(&pid_path).unwrap().trim().parse().unwrap();
    assert_eq!(content, h2.pid());
    h2.kill().await.unwrap();
}
```

`[dev-dependencies]` 加 `tempfile = "3"`(已是可选依赖,dev 下直接声明)。

- [ ] **Step 2: 跑测试确认失败**

```powershell
cargo test -p nyanpasu-utils --features process --test process_pid_file
```
Expected: FAIL(pid 文件不存在——尚未接线)。

- [ ] **Step 3: 实现 pid_file.rs**

```rust
use std::path::PathBuf;

/// Owns a pid file's lifecycle around one spawned child (design §5.4).
pub(crate) struct PidFileGuard {
    path: PathBuf,
}

impl PidFileGuard {
    pub(crate) async fn prepare(path: PathBuf, expected_exe: Option<String>) -> std::io::Result<Self> {
        let validator: Option<Vec<String>> = expected_exe.map(|e| vec![e.to_lowercase()]);
        match crate::os::kill_by_pid_file(&path, validator.as_deref()).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!("failed to kill residual process from pid file: {e}"),
        }
        Ok(Self { path })
    }

    pub(crate) async fn write(&self, pid: u32) -> std::io::Result<()> {
        crate::os::create_pid_file(&self.path, pid).await
    }

    pub(crate) async fn cleanup(&self) {
        if let Err(e) = tokio::fs::remove_file(&self.path).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("failed to remove pid file {:?}: {e}", self.path);
        }
    }
}
```

- [ ] **Step 4: engine.rs 接线**

在 `engine::spawn` 中、`build_pk(...).start()` **之前**:

```rust
    let pid_guard = match &cmd.pid_file {
        Some(path) => {
            let expected_exe = std::path::Path::new(&cmd.program)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned());
            Some(super::pid_file::PidFileGuard::prepare(path.clone(), expected_exe).await?)
        }
        None => None,
    };
```

spawn 成功取得 `pid` 后:

```rust
    if let Some(g) = &pid_guard {
        if let Err(e) = g.write(pid).await {
            tracing::warn!("failed to write pid file: {e}");
        }
    }
```

把 `pid_guard` move 进泵任务,在发送 `Terminated` 之后:

```rust
        if let Some(g) = pid_guard {
            g.cleanup().await;
        }
```

`mod.rs` 增加 `mod pid_file;`。

- [ ] **Step 5: 跑测试确认通过**

```powershell
cargo test -p nyanpasu-utils --features process --test process_pid_file
```
Expected: 2 passed。

- [ ] **Step 6: Commit**

```powershell
git add crates/nyanpasu-utils
git commit -m "feat(utils/process): pid file lifecycle (residual kill, write, cleanup)"
```

---

### Task 11: Supervisor 策略纯逻辑(RestartPolicy / Backoff / ReadinessProbe / SupervisorEvent)

**Files:**
- Create: `crates/nyanpasu-utils/src/process/supervisor.rs`
- Modify: `crates/nyanpasu-utils/src/process/mod.rs`

**Interfaces:**
- Produces(Task 12 依赖):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy { Never, OnFailure { max_restarts: u32 } }

#[derive(Debug, Clone, Copy)]
pub struct Backoff { /* private */ }
impl Backoff {
    pub fn exponential(initial: std::time::Duration, max: std::time::Duration) -> Self;
    pub fn with_jitter(self) -> Self;
    pub(crate) fn delay_for(&self, attempt: u32) -> std::time::Duration; // attempt 从 0 起
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum ReadinessProbe { AliveAfter(std::time::Duration) }

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SupervisorEvent {
    Started { pid: u32 },
    Ready,
    Exited(super::event::TerminatedPayload),
    Restarting { attempt: u32, delay: std::time::Duration },
    GaveUp,
    Stopped,
}
```

- [ ] **Step 1: 写失败单元测试(文件内 `#[cfg(test)]`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn backoff_doubles_and_caps() {
        let b = Backoff::exponential(Duration::from_secs(1), Duration::from_secs(30));
        assert_eq!(b.delay_for(0), Duration::from_secs(1));
        assert_eq!(b.delay_for(1), Duration::from_secs(2));
        assert_eq!(b.delay_for(4), Duration::from_secs(16));
        assert_eq!(b.delay_for(10), Duration::from_secs(30)); // capped
    }

    #[test]
    fn jitter_stays_within_25_percent() {
        let b = Backoff::exponential(Duration::from_secs(4), Duration::from_secs(60)).with_jitter();
        for _ in 0..100 {
            let d = b.delay_for(0);
            assert!(d >= Duration::from_secs(3) && d <= Duration::from_secs(5), "{d:?}");
        }
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
cargo test -p nyanpasu-utils --features process supervisor
```
Expected: FAIL。

- [ ] **Step 3: 实现类型与 Backoff**

```rust
use std::time::Duration;

use super::event::TerminatedPayload;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    Never,
    OnFailure { max_restarts: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct Backoff {
    initial: Duration,
    max: Duration,
    jitter: bool,
}

impl Backoff {
    pub fn exponential(initial: Duration, max: Duration) -> Self {
        Self { initial, max, jitter: false }
    }

    pub fn with_jitter(mut self) -> Self {
        self.jitter = true;
        self
    }

    pub(crate) fn delay_for(&self, attempt: u32) -> Duration {
        let base = self
            .initial
            .saturating_mul(2u32.saturating_pow(attempt.min(16)))
            .min(self.max);
        if !self.jitter {
            return base;
        }
        // Cheap deterministic-free jitter in [-25%, +25%] without a rand dependency.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0) as u64;
        let base_ns = base.as_nanos().max(1) as u64;
        let span = base_ns / 2; // total width 50% => ±25%
        let offset = nanos % span.max(1);
        Duration::from_nanos(base_ns - span / 2 + offset)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum ReadinessProbe {
    /// Emit `Ready` if the child is still alive after this delay
    /// (successor of the legacy 1.5s `DelayCheckpointPass`, design §5.3).
    AliveAfter(Duration),
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SupervisorEvent {
    Started { pid: u32 },
    Ready,
    Exited(TerminatedPayload),
    Restarting { attempt: u32, delay: Duration },
    GaveUp,
    Stopped,
}
```

`mod.rs` 增加:

```rust
mod supervisor;
pub use supervisor::{Backoff, ReadinessProbe, RestartPolicy, SupervisorEvent};
```

- [ ] **Step 4: 跑测试确认通过 + Commit**

```powershell
cargo test -p nyanpasu-utils --features process supervisor
git add crates/nyanpasu-utils
git commit -m "feat(utils/process): supervisor policy types with exponential backoff and jitter"
```

---

### Task 12: Supervisor 运行时(spawn loop / 就绪探测 / 事件回调 / stop / CancellationToken)

**Files:**
- Modify: `crates/nyanpasu-utils/src/process/supervisor.rs`
- Modify: `crates/nyanpasu-utils/src/process/mod.rs`(导出 `Supervisor, SupervisorBuilder`)
- Create: `crates/nyanpasu-utils/tests/process_supervisor.rs`

**Interfaces:**
- Consumes: Task 5 `Command::spawn` / `ProcessHandle`、Task 11 策略类型。
- Produces:

```rust
pub struct Supervisor { /* private */ }
impl Supervisor {
    pub fn builder<F>(factory: F) -> SupervisorBuilder
    where F: Fn() -> super::command::Command + Send + Sync + 'static;
    /// Cancels restarts, gracefully kills the current child, waits for the loop to end.
    pub async fn stop(self) -> Result<(), ProcessError>;
}

pub struct SupervisorBuilder { /* private */ }
impl SupervisorBuilder {
    pub fn restart_policy(self, p: RestartPolicy) -> Self;              // default OnFailure { max_restarts: 5 }
    pub fn backoff(self, b: Backoff) -> Self;                           // default exponential(1s, 30s).with_jitter()
    pub fn readiness(self, r: ReadinessProbe) -> Self;                  // default AliveAfter(1500ms)
    pub fn on_event(self, f: impl Fn(SupervisorEvent) + Send + Sync + 'static) -> Self;
    pub fn on_process_event(self, f: impl Fn(ProcessEvent) + Send + Sync + 'static) -> Self;
    pub fn cancel_token(self, t: tokio_util::sync::CancellationToken) -> Self;
    /// Errors if the FIRST spawn fails; later failures go through the restart policy.
    pub async fn spawn(self) -> Result<Supervisor, ProcessError>;
}
```

**循环语义(实现依据,也写进 rustdoc):**
- 每轮:factory() → spawn → 发 `Started{pid}`;转发子进程 `ProcessEvent` 给 `on_process_event`。
- 就绪:spawn 后若 `AliveAfter(d)` 期间未退出 → 发 `Ready` 且 **attempt 归零**;期间退出视为一次失败。
- 退出:发 `Exited(payload)`。`code == Some(0)` 视为干净退出 → 结束循环(不重启);cancel 触发 → 结束循环并发 `Stopped`。
- 失败重启:`attempt += 1`;超过 `max_restarts` → 发 `GaveUp` 结束;否则发 `Restarting{attempt, delay}` 后等待 backoff(等待期间可被 cancel 打断 → `Stopped`)。
- `stop()`:cancel 内部 token → 当前子进程 `graceful_kill` → 等循环任务结束。

- [ ] **Step 1: 写失败集成测试**

`crates/nyanpasu-utils/tests/process_supervisor.rs`:

```rust
#![cfg(feature = "process")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use nyanpasu_utils::process::{
    Backoff, Command, ReadinessProbe, RestartPolicy, Supervisor, SupervisorEvent,
};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

#[derive(Clone, Default)]
struct EventLog(Arc<Mutex<Vec<SupervisorEvent>>>);

impl EventLog {
    fn push(&self, e: SupervisorEvent) {
        self.0.lock().unwrap().push(e);
    }
    fn snapshot(&self) -> Vec<SupervisorEvent> {
        self.0.lock().unwrap().clone()
    }
    async fn wait_for(&self, pred: impl Fn(&[SupervisorEvent]) -> bool, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if pred(&self.snapshot()) {
                return;
            }
            assert!(tokio::time::Instant::now() < deadline, "timeout; log = {:?}", self.snapshot());
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

#[tokio::test]
async fn restarts_on_failure_then_gives_up() {
    let log = EventLog::default();
    let log2 = log.clone();
    let _sup = Supervisor::builder(|| Command::new(child()).args(["exit-with", "1"]))
        .restart_policy(RestartPolicy::OnFailure { max_restarts: 2 })
        .backoff(Backoff::exponential(Duration::from_millis(10), Duration::from_millis(40)))
        .readiness(ReadinessProbe::AliveAfter(Duration::from_millis(5000))) // never ready
        .on_event(move |e| log2.push(e))
        .spawn()
        .await
        .unwrap();

    log.wait_for(|evs| evs.iter().any(|e| matches!(e, SupervisorEvent::GaveUp)), Duration::from_secs(10)).await;
    let evs = log.snapshot();
    let starts = evs.iter().filter(|e| matches!(e, SupervisorEvent::Started { .. })).count();
    let restarts = evs.iter().filter(|e| matches!(e, SupervisorEvent::Restarting { .. })).count();
    assert_eq!(starts, 3, "initial + 2 restarts, log = {evs:?}");
    assert_eq!(restarts, 2);
    assert!(!evs.iter().any(|e| matches!(e, SupervisorEvent::Ready)));
}

#[tokio::test]
async fn ready_emitted_and_stop_is_clean() {
    let log = EventLog::default();
    let log2 = log.clone();
    let sup = Supervisor::builder(|| Command::new(child()).args(["sleep-forever"]))
        .readiness(ReadinessProbe::AliveAfter(Duration::from_millis(100)))
        .on_event(move |e| log2.push(e))
        .spawn()
        .await
        .unwrap();

    log.wait_for(|evs| evs.iter().any(|e| matches!(e, SupervisorEvent::Ready)), Duration::from_secs(5)).await;
    sup.stop().await.unwrap();
    let evs = log.snapshot();
    assert!(matches!(evs.last().unwrap(), SupervisorEvent::Stopped), "log = {evs:?}");
    // no restart after stop
    assert!(!evs.iter().any(|e| matches!(e, SupervisorEvent::Restarting { .. })));
}

#[tokio::test]
async fn clean_exit_does_not_restart() {
    let log = EventLog::default();
    let log2 = log.clone();
    let _sup = Supervisor::builder(|| Command::new(child()).args(["exit-with", "0"]))
        .restart_policy(RestartPolicy::OnFailure { max_restarts: 3 })
        .on_event(move |e| log2.push(e))
        .spawn()
        .await
        .unwrap();
    log.wait_for(|evs| evs.iter().any(|e| matches!(e, SupervisorEvent::Exited(_))), Duration::from_secs(5)).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let evs = log.snapshot();
    assert_eq!(evs.iter().filter(|e| matches!(e, SupervisorEvent::Started { .. })).count(), 1);
    assert!(!evs.iter().any(|e| matches!(e, SupervisorEvent::Restarting { .. })));
}

#[tokio::test]
async fn first_spawn_failure_is_error() {
    let r = Supervisor::builder(|| Command::new("definitely-not-a-real-binary-42"))
        .spawn()
        .await;
    assert!(r.is_err());
}
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
cargo test -p nyanpasu-utils --features process --test process_supervisor
```
Expected: FAIL(`Supervisor` 无 builder/runtime)。

- [ ] **Step 3: 实现运行时**

`supervisor.rs` 追加:

```rust
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::{
    command::Command,
    error::ProcessError,
    event::ProcessEvent,
    handle::ProcessHandle,
};

type Factory = Arc<dyn Fn() -> Command + Send + Sync>;
type EventHook = Arc<dyn Fn(SupervisorEvent) + Send + Sync>;
type ProcessEventHook = Arc<dyn Fn(ProcessEvent) + Send + Sync>;

pub struct SupervisorBuilder {
    factory: Factory,
    policy: RestartPolicy,
    backoff: Backoff,
    readiness: ReadinessProbe,
    on_event: Option<EventHook>,
    on_process_event: Option<ProcessEventHook>,
    cancel_token: Option<CancellationToken>,
}

pub struct Supervisor {
    token: CancellationToken,
    current: Arc<tokio::sync::Mutex<Option<ProcessHandle>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Supervisor {
    pub fn builder<F>(factory: F) -> SupervisorBuilder
    where
        F: Fn() -> Command + Send + Sync + 'static,
    {
        SupervisorBuilder {
            factory: Arc::new(factory),
            policy: RestartPolicy::OnFailure { max_restarts: 5 },
            backoff: Backoff::exponential(std::time::Duration::from_secs(1), std::time::Duration::from_secs(30)).with_jitter(),
            readiness: ReadinessProbe::AliveAfter(std::time::Duration::from_millis(1500)),
            on_event: None,
            on_process_event: None,
            cancel_token: None,
        }
    }

    pub async fn stop(self) -> Result<(), ProcessError> {
        self.token.cancel();
        if let Some(h) = self.current.lock().await.clone() {
            let _ = h.graceful_kill().await;
        }
        let _ = self.task.await;
        Ok(())
    }
}

impl SupervisorBuilder {
    pub fn restart_policy(mut self, p: RestartPolicy) -> Self { self.policy = p; self }
    pub fn backoff(mut self, b: Backoff) -> Self { self.backoff = b; self }
    pub fn readiness(mut self, r: ReadinessProbe) -> Self { self.readiness = r; self }
    pub fn on_event(mut self, f: impl Fn(SupervisorEvent) + Send + Sync + 'static) -> Self {
        self.on_event = Some(Arc::new(f));
        self
    }
    pub fn on_process_event(mut self, f: impl Fn(ProcessEvent) + Send + Sync + 'static) -> Self {
        self.on_process_event = Some(Arc::new(f));
        self
    }
    pub fn cancel_token(mut self, t: CancellationToken) -> Self { self.cancel_token = Some(t); self }

    pub async fn spawn(self) -> Result<Supervisor, ProcessError> {
        let token = self.cancel_token.unwrap_or_default().child_token();
        let current: Arc<tokio::sync::Mutex<Option<ProcessHandle>>> = Arc::default();
        let emit = {
            let hook = self.on_event.clone();
            move |e: SupervisorEvent| {
                if let Some(h) = &hook {
                    h(e);
                }
            }
        };

        // First spawn happens HERE so a broken command fails fast.
        let (first_handle, first_rx) = (self.factory)().spawn().await?;
        emit(SupervisorEvent::Started { pid: first_handle.pid() });
        *current.lock().await = Some(first_handle.clone());

        let factory = self.factory;
        let policy = self.policy;
        let backoff = self.backoff;
        let readiness = self.readiness;
        let on_process_event = self.on_process_event;
        let token_ = token.clone();
        let current_ = current.clone();

        let task = tokio::spawn(async move {
            let mut attempt: u32 = 0;
            let mut handle = first_handle;
            let mut rx = first_rx;
            loop {
                // ---- readiness phase ----
                let ReadinessProbe::AliveAfter(delay) = readiness;
                let ready_at = tokio::time::Instant::now() + delay;
                let mut became_ready = false;
                // ---- run phase: pump events until termination ----
                let payload = loop {
                    tokio::select! {
                        _ = tokio::time::sleep_until(ready_at), if !became_ready => {
                            became_ready = true;
                            attempt = 0;
                            emit(SupervisorEvent::Ready);
                        }
                        maybe_ev = rx.recv() => match maybe_ev {
                            Some(ProcessEvent::Terminated(p)) => break Some(p),
                            Some(e) => {
                                if let Some(h) = &on_process_event {
                                    h(e);
                                }
                            }
                            None => break None,
                        },
                    }
                };
                let payload = payload.unwrap_or(super::event::TerminatedPayload { code: None, signal: None });
                let clean_exit = payload.code == Some(0);
                emit(SupervisorEvent::Exited(payload));
                *current_.lock().await = None;

                if token_.is_cancelled() {
                    emit(SupervisorEvent::Stopped);
                    return;
                }
                if clean_exit || matches!(policy, RestartPolicy::Never) {
                    return;
                }
                attempt += 1;
                let RestartPolicy::OnFailure { max_restarts } = policy else { return };
                if attempt > max_restarts {
                    emit(SupervisorEvent::GaveUp);
                    return;
                }
                let delay = backoff.delay_for(attempt - 1);
                emit(SupervisorEvent::Restarting { attempt, delay });
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = token_.cancelled() => {
                        emit(SupervisorEvent::Stopped);
                        return;
                    }
                }
                // ---- respawn ----
                match (factory)().spawn().await {
                    Ok((h, r)) => {
                        emit(SupervisorEvent::Started { pid: h.pid() });
                        *current_.lock().await = Some(h.clone());
                        handle = h;
                        rx = r;
                    }
                    Err(e) => {
                        tracing::error!("supervisor respawn failed: {e}");
                        // treat as an immediate failure of this attempt; loop continues
                        continue;
                    }
                }
                let _ = &handle; // handle kept for parity; current_ is the source of truth
            }
        });

        Ok(Supervisor { token, current, task })
    }
}
```

`mod.rs` 的 supervisor 导出行改为:

```rust
pub use supervisor::{
    Backoff, ReadinessProbe, RestartPolicy, Supervisor, SupervisorBuilder, SupervisorEvent,
};
```

- [ ] **Step 4: 跑测试确认通过**

```powershell
cargo test -p nyanpasu-utils --features process --test process_supervisor
```
Expected: 4 passed。注意 `respawn 失败 continue` 分支会跳过 readiness/run 阶段直接进入下一轮判定——确认 `restarts_on_failure_then_gives_up` 中 3 次 Started 计数正确。

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-utils
git commit -m "feat(utils/process): Supervisor runtime with readiness probe, backoff restarts and stop"
```

---

### Task 13: 模块文档、全量验收与收尾

**Files:**
- Modify: `crates/nyanpasu-utils/src/process/mod.rs`(rustdoc)
- Modify: `docs/superpowers/specs/2026-07-16-nyanpasu-utils-process-module-design.md`(§6 P0/P1/P2 勾选与 O3 结果回填)

**Interfaces:** 无新接口。

- [ ] **Step 1: mod.rs 顶部补完整 rustdoc(含 no_run 示例)**

```rust
//! Generic child-process management: spawn, event stream, kill, supervise.
//!
//! processkit is the internal engine and MUST NOT appear in any public signature
//! (only `engine.rs` may import it). Event contract: [`ProcessEvent::Terminated`]
//! is always the final event on the channel.
//!
//! ```no_run
//! use nyanpasu_utils::process::{Command, ProcessEvent};
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let (handle, mut events) = Command::new("mihomo")
//!     .args(["-d", "/etc/mihomo"])
//!     .pid_file("/run/mihomo.pid")
//!     .spawn()
//!     .await?;
//! while let Some(ev) = events.recv().await {
//!     match ev {
//!         ProcessEvent::Stdout(line) => tracing::info!("{line}"),
//!         ProcessEvent::Stderr(line) => tracing::error!("{line}"),
//!         ProcessEvent::Error(e) => tracing::warn!("pump: {e}"),
//!         ProcessEvent::Terminated(p) => {
//!             tracing::info!("exited: {p:?}");
//!             break;
//!         }
//!     }
//! }
//! handle.graceful_kill().await.ok();
//! # Ok(()) }
//! ```
//!
//! Design: docs/superpowers/specs/2026-07-16-nyanpasu-utils-process-module-design.md
```

- [ ] **Step 2: 全量验收命令**

```powershell
cargo fmt --check
cargo clippy -p nyanpasu-utils --features process -- -D warnings
cargo test -p nyanpasu-utils --features process
cargo check -p nyanpasu-utils            # 默认 feature 集不受影响
cargo check -p nyanpasu-service          # 工作区其余成员不受影响
cargo doc -p nyanpasu-utils --features process --no-deps
```
Expected: 全部通过,tests ≥ 20 passed。

- [ ] **Step 3: 对照设计文档 §8 验收标准核对**

- §8.2 强杀整树 + 孙进程回收测试:`process_kill.rs::whole_tree_is_reaped` ✔
- §8.3 `src/process/**` 无 `std::thread::spawn`:`Select-String` 复查 ✔
- §8.1/§8.4/§8.5(消费方迁移、TODO 移除)属 core-manager 计划,明确不在本计划范围。

- [ ] **Step 4: 设计文档回填 + Commit**

在设计文档 §6 表格 P0/P1/P2 行尾追加"✅ 完成于 feat/utils-process-module";O3 的本机结果(JobObject)写入 §9 O3 行。

```powershell
git add crates/nyanpasu-utils docs/superpowers
git commit -m "docs(utils/process): module rustdoc and design-doc P0-P2 closure"
```

---

## 计划外(明确不做)

- `nyanpasu-utils::core` 的改造 / `#[deprecated]` 标注、`os/child.rs` 删除 —— core-manager 迁移计划(设计 §6 P3/P4)。
- `nyanpasu-core-manager` crate 的任何代码。
- `nyanpasu_service` 的接入与 `recover_core` 删除。
- Linux/macOS CI 矩阵调整(unix 门控测试随现有 CI 自然生效;若 CI 工具链 < 1.88 需在独立 PR 处理)。
