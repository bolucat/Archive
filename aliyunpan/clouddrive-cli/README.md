# clouddrive-cli

> **Turn BoxPlayer cloud-drive accounts into an agent-friendly CLI.**  
> List, search, organize, rename, upload-plan, and roll back files across multiple cloud-drive providers through one deterministic command surface.

[中文](#中文) | [English](#english)

---

# 中文

`clouddrive-cli` 是 BoxPlayer 面向终端、人类用户和 AI Agent 暴露的多网盘自动化入口。它把 Electron App 里已登录的网盘账号，或独立 CLI 登录得到的账号，统一变成可发现、可 dry-run、可回滚的命令接口。

支持 provider：

`aliyun` · `cloud123` · `115` · `baidu` · `pikpak` · `onedrive` · `box` · `dropbox`

## 快速开始

### 1. 安装 CLI

方式 A：跟随 BoxPlayer Electron App 安装。

```bash
clouddrive-cli --help
clouddrive-cli auth list --format json
```

方式 B：独立 npm 安装。

```bash
npm install -g clouddrive-cli
clouddrive-cli --help
```

开发源码入口：

```bash
node clouddrive-cli/bin/cli.mjs --help
node clouddrive-cli/bin/cli.mjs list --format json
```

### 2. 发现命令面

不要把 README 当作完整命令注册表。实时命令清单来自 CLI 自己：

```bash
clouddrive-cli list --format json
clouddrive-cli schema commands
```

`list --format json` 会返回每个命令的 `group`、`name`、`command`、`access`、`args`、`options`、`requiresDryRun`、`destructive`、`undoable` 和 `output`。

### 3. 查看账号与能力

```bash
clouddrive-cli auth list --format json
clouddrive-cli settings show --format json
clouddrive-cli providers capabilities --format json
```

AI Agent 在任何写操作前都应先读取 provider 能力矩阵。

### 4. 跑第一个只读命令

```bash
clouddrive-cli files stats --provider aliyun --account default --file-id root --depth 1 --format json
clouddrive-cli files list --provider aliyun --account default --file-id root --format json
```

## 给人类用户

常用命令：

- `clouddrive-cli auth list` 查看已登录账号。
- `clouddrive-cli files list` 列出目录。
- `clouddrive-cli files search` 搜索文件。
- `clouddrive-cli files download` 下载单个文件到本地。
- `clouddrive-cli files tree` 快速看目录结构。
- `clouddrive-cli docs read` 读取本地规则文档。
- `clouddrive-cli ops list` 查看历史操作。

如果只是想确认当前 CLI 有什么能力：

```bash
clouddrive-cli list
clouddrive-cli providers capabilities
```

## 给 AI Agent

AI Agent 不应该猜命令、猜 file id、猜 provider 能力。推荐起手式：

```bash
clouddrive-cli list --format json
clouddrive-cli auth list --format json
clouddrive-cli providers capabilities --format json
```

JSON 模式下，错误也会返回结构化 envelope：

```json
{
  "ok": false,
  "error": {
    "code": "UNSUPPORTED_CAPABILITY",
    "message": "Unknown provider: demo",
    "exitCode": 5
  }
}
```

Agent 应该根据 `error.code` 分支，而不是解析自然语言错误文本。

### 安装 skill

如果通过 npm 包安装：

```bash
npx skills add boxplayer/clouddrive-cli -g
```

如果从本机包安装，可读取：

```bash
$(npm root -g)/clouddrive-cli/skill/SKILL.md
```

BoxPlayer App 内置安装模式下，skill 文件位于 app resources 的 `clouddrive-cli/skill/SKILL.md`。

### Agent 安全规则

| 规则 | 说明 |
|------|------|
| 先发现 | 先跑 `list --format json`，不要硬编码 README 中的命令列表。 |
| 先能力 | 写操作前跑 `providers capabilities --format json`。 |
| 先 dry-run | 标记 `requiresDryRun` 的命令必须先预览。 |
| 不编造 ID | file id 必须来自 `files list`、`files walk`、`files search`、`files tree` 或 `files info`。 |
| 大输出落盘 | `largeOutput: true` 的命令优先使用 `--output <file.json>`，CLI 会写完整 JSON，并在 stdout 返回摘要。 |
| 写入理由 | 写操作使用 `--rationale <reason>` 记录 Agent 为什么执行该动作。 |

## 安装模式

### 模式 1：跟随 Electron App

BoxPlayer App 会注册：

```text
clouddrive-cli
clouddrive-mcp
```

macOS / Linux 默认写入：

```bash
~/.local/bin/clouddrive-cli
~/.local/bin/clouddrive-mcp
```

Windows 默认写入：

```text
%LOCALAPPDATA%\BoxPlayer\bin\clouddrive-cli.cmd
%LOCALAPPDATA%\BoxPlayer\bin\clouddrive-mcp.cmd
```

这种模式会复用 App 导出的账号 token：

```bash
~/.clouddrive-cli/tokens.json
```

### 模式 2：独立安装

独立模式适合服务器、CI 或只需要 CLI 的用户。

```bash
npm install -g clouddrive-cli
clouddrive-cli auth login aliyun --format json
clouddrive-cli auth login 115 --format json
clouddrive-cli auth login dropbox --browser chrome --format json
clouddrive-cli auth login box --browser chrome --format json
clouddrive-cli auth login 123 --browser chrome --format json
```

`aliyun` 和 `115` 使用终端二维码。`dropbox`、`box`、`123`、`onedrive` 使用浏览器 OAuth loopback。若 provider 限制回调地址，使用已登记的 `http://127.0.0.1:<port>/callback`，或从 Electron App 导入 token。

## 配置

默认配置目录：

```bash
~/.clouddrive-cli
```

主要文件：

```text
tokens.json
config.json
operations/
```

环境变量：

| 变量 | 说明 |
|------|------|
| `CLOUDDRIVE_CLI_CONFIG_DIR` | 覆盖默认配置目录。 |

## 输出与错误

`--json` 与 `--format json` 等价。建议 Agent 始终使用 JSON。

大输出命令支持 `--output <file.json>`。使用后完整结果写入文件，stdout 只返回适合 Agent 读取的摘要：

```bash
clouddrive-cli files stats --provider aliyun --account default --file-id root --output stats.json --format json
```

常见退出码：

| 退出码 | 错误码 | 含义 |
|--------|--------|------|
| `0` | - | 成功 |
| `1` | `VALIDATION_ERROR` | 参数、输入文件或计划校验错误 |
| `2` | `AUTH_ERROR` | 账号缺失、过期或认证失败 |
| `3` | `PROVIDER_API_ERROR` | provider API/HTTP 错误 |
| `4` | `PARTIAL_SUCCESS` | 部分成功 |
| `5` | `UNSUPPORTED_CAPABILITY` | provider 或命令能力不支持 |

## 命令发现

```bash
clouddrive-cli list --format json
clouddrive-cli list --group files --format json
clouddrive-cli schema commands
```

命令 manifest 会包含 Agent 契约字段：`examples`、`largeOutput`、`safety`、`providerRequirements`。MCP tool schema 也从同一份 manifest 生成。

顶层命令组：

| 组 | 用途 |
|----|------|
| `auth` | 账号发现、默认账号、登录、token 导入 |
| `settings` | 配置目录、账号和 provider 摘要 |
| `providers` | provider 能力矩阵 |
| `files` | 文件读取、搜索、目录、重命名、移动、回收站 |
| `media` | 媒体扫描、匹配、命名计划 |
| `docs` | 本地文档读取，提供给 AI 作为上下文 |
| `upload` | 本地上传计划与 dry-run |
| `organize` | 网盘目录分析、整理计划与 dry-run |
| `ops` | 操作日志与撤销 |
| `schema` | 命令 schema/manifest |

## 典型工作流

### 读取目录

```bash
clouddrive-cli files stats --provider aliyun --account default --file-id root --depth 2 --format json
clouddrive-cli files tree --provider aliyun --account default --file-id root --depth 1
clouddrive-cli files list --provider aliyun --account default --file-id root --format json
clouddrive-cli files list --provider aliyun --account default --file-id root --limit 50 --format json
clouddrive-cli files list --provider aliyun --account default --file-id root --limit 50 --cursor <nextCursor> --format json
clouddrive-cli files download --provider aliyun --account default --file-id <file-id> --output ./download.bin --format json
```

### 媒体重命名

```bash
clouddrive-cli docs read ./rename-rules.md --max-chars 50000 --format json
clouddrive-cli docs read ./rules.pdf --pdf-format markdown --pdf-pages 1-3 --format json
clouddrive-cli docs convert ./pdf-folder --output ./out --pdf-format json,html,pdf,markdown,tagged-pdf,text --format json
clouddrive-cli files walk --provider aliyun --account default --file-id <folder-id> --output files.json --format json
clouddrive-cli media match --input files.json --format json
# AI 基于规则、文件清单和 match 结果生成 rename-plan.json
clouddrive-cli files rename-apply rename-plan.json --dry-run --format json
```

`docs read` 会通过 OpenDataLoader 读取 PDF；`docs convert` 会把 OpenDataLoader 的 PDF 转换能力完整暴露出来，支持输出 `json`、`text`、`html`、`pdf`、`markdown`、`tagged-pdf`，并支持 `--pdf-content-safety-off`、`--pdf-sanitize`、`--pdf-keep-line-breaks`、`--pdf-use-struct-tree`、`--pdf-table-method`、`--pdf-reading-order`、页面分隔符、图片输出、`--pdf-pages`、header/footer、strikethrough、hybrid backend、Hancom AI 参数和 `--pdf-threads`。运行 PDF 转换需要 Node.js 20+，并且系统 `PATH` 中可用 Java 11+。

用户确认后再执行：

```bash
clouddrive-cli files rename-apply rename-plan.json --rationale "Normalize media filenames for metadata scraping" --format json
```

### 上传计划

```bash
clouddrive-cli upload plan --local ./Media --provider aliyun --account default --remote-parent <folder-id> --output upload-plan.json --format json
clouddrive-cli upload apply upload-plan.json --dry-run --format json
clouddrive-cli upload apply upload-plan.json --format json
```

真实上传要求 provider 的 `uploadFile` 能力为 `true`。当前 CLI 已接入 `aliyun`、`cloud123`、`115`、`baidu`、`dropbox`、`onedrive`、`box` 的本地文件上传；`pikpak` 暂不声明本地字节上传能力。

### 网盘目录整理

```bash
clouddrive-cli organize analyze --provider aliyun --account default --file-id <folder-id> --depth 5 --output analysis.json --summary --format json
clouddrive-cli organize plan --analysis analysis.json --rules ./organize-rules.md --output organize-plan.json --summary --format json
clouddrive-cli organize apply organize-plan.json --dry-run --summary --format json
```

`--rules` 当前会写入计划作为审计上下文；内置 planner 仍使用确定性的 `Movies` / `TV Shows` 分类策略。
全盘整理风险高。即使 dry-run 成功，也应抽样检查移动目标，避免把已有结构打平成 `Movies` / `TV Shows`。

### 回滚

```bash
clouddrive-cli ops list --format json
clouddrive-cli ops show <operation-id> --format json
clouddrive-cli ops undo <operation-id> --dry-run --format json
clouddrive-cli ops undo <operation-id> --format json
```

只有 rename 和 move 支持 CLI undo。trash 不支持 undo。

## MCP

`clouddrive-mcp` 是可选 MCP Server，适合支持 MCP 的 AI 客户端直接调用工具。MCP tool 名称和 input schema 从同一个 CLI command manifest 自动生成，也就是 `clouddrive-cli list --format json` / `schema commands` 的接口源。

```bash
clouddrive-mcp
```

需要 plan 文件、输出重定向、细粒度本地处理或调试 provider 行为时，优先使用 CLI。

## Provider 注意事项

- OneDrive `files search` 可能因 Microsoft Graph `generalException` / HTTP 500 间歇失败，可回退到 `files walk` 后本地过滤。
- PikPak `files search --name` 可能忽略查询并返回 root items，可回退到 `files walk` 后本地过滤。
- Baidu `files info` 已兼容 `filemetas` 的 `list` / `info` 响应；若仍返回 `errno=12`，按 API/token 限制处理。
- 115 部分文件夹可能被 API 字段映射成 `type: file`，可通过 `files list --file-id <fileId>` 验证是否可作为目录读取。

---

# English

`clouddrive-cli` is the BoxPlayer automation interface for terminals, humans, and AI agents. It exposes logged-in cloud-drive accounts from the Electron app or standalone CLI auth as one discoverable command surface with dry-run planning and rollback support.

Supported providers:

`aliyun` · `cloud123` · `115` · `baidu` · `pikpak` · `onedrive` · `box` · `dropbox`

## Quick Start

### 1. Install

Install through the BoxPlayer Electron app, or install the standalone package:

```bash
npm install -g clouddrive-cli
clouddrive-cli --help
```

From source:

```bash
node clouddrive-cli/bin/cli.mjs --help
node clouddrive-cli/bin/cli.mjs list --format json
```

### 2. Discover commands

Do not treat this README as the complete command registry. Ask the CLI:

```bash
clouddrive-cli list --format json
clouddrive-cli schema commands
```

### 3. Inspect accounts and capabilities

```bash
clouddrive-cli auth list --format json
clouddrive-cli settings show --format json
clouddrive-cli providers capabilities --format json
```

### 4. Run a read command

```bash
clouddrive-cli files stats --provider aliyun --account default --file-id root --depth 1 --format json
clouddrive-cli files list --provider aliyun --account default --file-id root --format json
```

## For Humans

Use the CLI directly when you need repeatable cloud-drive operations:

- `clouddrive-cli auth list` lists configured accounts.
- `clouddrive-cli files list` lists a directory.
- `clouddrive-cli files search` searches files.
- `clouddrive-cli files download` downloads one file to a local path.
- `clouddrive-cli files tree` previews structure.
- `clouddrive-cli docs read` reads a local rules document.
- `clouddrive-cli ops list` shows operation history.

## For AI Agents

Start every session with:

```bash
clouddrive-cli list --format json
clouddrive-cli auth list --format json
clouddrive-cli providers capabilities --format json
```

JSON-mode failures return:

```json
{
  "ok": false,
  "error": {
    "code": "UNSUPPORTED_CAPABILITY",
    "message": "Unknown provider: demo",
    "exitCode": 5
  }
}
```

Branch on `error.code`, not natural-language text.

### Install the skill

```bash
npx skills add boxplayer/clouddrive-cli -g
```

Or read the packaged skill from:

```bash
$(npm root -g)/clouddrive-cli/skill/SKILL.md
```

## Install Modes

### Mode 1: BoxPlayer App

The Electron app registers:

```text
clouddrive-cli
clouddrive-mcp
```

It also exports app accounts into:

```bash
~/.clouddrive-cli/tokens.json
```

### Mode 2: Standalone CLI

Use `auth login` or `auth import-token`:

```bash
clouddrive-cli auth login aliyun --format json
clouddrive-cli auth login 115 --format json
clouddrive-cli auth login dropbox --browser chrome --format json
clouddrive-cli auth import-token --provider aliyun --account <id> --token token.json --default --format json
```

Aliyun and 115 render QR codes in the terminal. Dropbox, Box, 123Pan, and OneDrive use browser OAuth loopback.

## Configuration

Default config directory:

```bash
~/.clouddrive-cli
```

Main files:

```text
tokens.json
config.json
operations/
```

Environment:

| Variable | Purpose |
|----------|---------|
| `CLOUDDRIVE_CLI_CONFIG_DIR` | Override the default config directory. |

## Output and Errors

`--json` and `--format json` are equivalent.

Large-output commands support `--output <file.json>`. The complete result is written to disk and stdout returns a compact summary:

```bash
clouddrive-cli files stats --provider aliyun --account default --file-id root --output stats.json --format json
```

| Exit code | Error code | Meaning |
|-----------|------------|---------|
| `0` | - | Success |
| `1` | `VALIDATION_ERROR` | Invalid args, input, or plan |
| `2` | `AUTH_ERROR` | Missing or expired account |
| `3` | `PROVIDER_API_ERROR` | Provider API/HTTP failure |
| `4` | `PARTIAL_SUCCESS` | Partial success |
| `5` | `UNSUPPORTED_CAPABILITY` | Unsupported provider or capability |

## Command Discovery

```bash
clouddrive-cli list --format json
clouddrive-cli list --group files --format json
clouddrive-cli schema commands
```

The manifest includes agent contract fields: `examples`, `largeOutput`, `safety`, and `providerRequirements`. MCP tool schemas are generated from the same manifest.

Top-level groups:

| Group | Purpose |
|-------|---------|
| `auth` | Accounts, defaults, login, token import |
| `settings` | Config/account/provider summary |
| `providers` | Provider capability matrix |
| `files` | File reads, search, mkdir, rename, move, trash |
| `media` | Media scan, match, rename plans |
| `docs` | Local document context |
| `upload` | Upload planning and dry-run |
| `organize` | Cloud directory analysis and organization plans |
| `ops` | Operation history and undo |
| `schema` | Command schema/manifest |

## Common Workflows

### Read a directory

```bash
clouddrive-cli files stats --provider aliyun --account default --file-id root --depth 2 --format json
clouddrive-cli files tree --provider aliyun --account default --file-id root --depth 1
clouddrive-cli files list --provider aliyun --account default --file-id root --format json
clouddrive-cli files list --provider aliyun --account default --file-id root --limit 50 --format json
clouddrive-cli files list --provider aliyun --account default --file-id root --limit 50 --cursor <nextCursor> --format json
clouddrive-cli files download --provider aliyun --account default --file-id <file-id> --output ./download.bin --format json
```

### Rename media

```bash
clouddrive-cli docs read ./rename-rules.md --max-chars 50000 --format json
clouddrive-cli docs read ./rules.pdf --pdf-format markdown --pdf-pages 1-3 --format json
clouddrive-cli docs convert ./pdf-folder --output ./out --pdf-format json,html,pdf,markdown,tagged-pdf,text --format json
clouddrive-cli files walk --provider aliyun --account default --file-id <folder-id> --output files.json --format json
clouddrive-cli media match --input files.json --format json
# Let the AI generate rename-plan.json from the rules, inventory, and match output.
clouddrive-cli files rename-apply rename-plan.json --dry-run --format json
```

`docs read` uses OpenDataLoader for PDF files. `docs convert` exposes the full OpenDataLoader PDF conversion surface, including `json`, `text`, `html`, `pdf`, `markdown`, and `tagged-pdf` outputs plus content safety, sanitization, structure tree, table/reading-order, page separator, image output, page selection, header/footer, strikethrough, hybrid backend, Hancom AI, and thread options. PDF conversion requires Node.js 20+ and Java 11+ available on `PATH`.

### Plan uploads

```bash
clouddrive-cli upload plan --local ./Media --provider aliyun --account default --remote-parent <folder-id> --output upload-plan.json --format json
clouddrive-cli upload apply upload-plan.json --dry-run --format json
clouddrive-cli upload apply upload-plan.json --format json
```

Real upload requires provider capability `uploadFile: true`. The CLI currently wires real local-byte upload for `aliyun`, `cloud123`, `115`, `baidu`, `dropbox`, `onedrive`, and `box`; `pikpak` does not advertise local-byte upload yet.

### Organize a cloud directory

```bash
clouddrive-cli organize analyze --provider aliyun --account default --file-id <folder-id> --depth 5 --output analysis.json --summary --format json
clouddrive-cli organize plan --analysis analysis.json --rules ./organize-rules.md --output organize-plan.json --summary --format json
clouddrive-cli organize apply organize-plan.json --dry-run --summary --format json
```

`--rules` is recorded in the plan as audit context. The built-in planner still uses deterministic `Movies` / `TV Shows` placement.

### Roll back

```bash
clouddrive-cli ops list --format json
clouddrive-cli ops show <operation-id> --format json
clouddrive-cli ops undo <operation-id> --dry-run --format json
clouddrive-cli ops undo <operation-id> --format json
```

Only rename and move operations support CLI undo.

## MCP

`clouddrive-mcp` exposes the CLI command manifest as MCP tools. Tool names and input schemas are generated from the same source used by `clouddrive-cli list --format json` and `schema commands`:

```bash
clouddrive-mcp
```

Use direct CLI commands for plan files, output redirection, local post-processing, and provider debugging.

## Provider Notes

- OneDrive search may intermittently fail with Microsoft Graph HTTP 500. Fall back to walk plus local filtering.
- PikPak search by name may be ignored by the API. Fall back to walk plus local filtering.
- Baidu file info handles `filemetas` under `list` or `info`; `errno=12` remains a possible API/token limitation.
- 115 folder fields may be ambiguous; confirm by listing the item path before folder operations.
