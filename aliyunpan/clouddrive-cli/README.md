# clouddrive-cli

> BoxPlayer 的多网盘命令行工具。只开放网盘文件管理能力，不开放媒体库整理、文档读取、AI 阅读、媒体服务器或播放相关能力。

[中文](#中文) | [English](./README.en.md)

---

## 中文

`clouddrive-cli` 把 BoxPlayer 中的网盘账号变成适合终端和 AI Agent 调用的确定性命令接口。它的边界很清晰：只处理云盘账号、目录、文件、上传、下载、移动、重命名、删除、能力发现和操作记录。

不包含：

- 媒体库扫描 / 媒体命名识别
- 影视整理计划 / 媒体库整理
- 本地文档读取 / PDF 转换
- AI 搜索、AI 阅读、媒体获取、追更
- 媒体服务器、播放、字幕、弹幕

## 支持 provider

```text
aliyun · cloud123 · 115 · 139 · 189 · quark · pikpak · dropbox · onedrive · box · baidu
```

可用能力以实时输出为准：

```bash
clouddrive-cli providers capabilities --json
```

## 安装

跟随 BoxPlayer App 安装：

```text
BoxPlayer → 账户设置 → 安装命令行工具
```

独立安装：

```bash
npm install -g clouddrive-cli
```

开发源码运行：

```bash
node clouddrive-cli/bin/cli.mjs --help
```

## 命令发现

不要硬编码 README。CLI 自带机器可读命令清单：

```bash
clouddrive-cli list --json
clouddrive-cli schema commands
clouddrive-cli schema plans
```

当前保留的命令组：

| 组 | 用途 |
|---|---|
| `auth` | 查看账号、设置默认账号、导入 token、登录 |
| `settings` | 查看配置目录、账号统计、默认账号 |
| `providers` | 查看 provider 能力 |
| `files` | 列表、递归遍历、树、统计、详情、搜索、下载、新建目录、移动、重命名、删除 |
| `upload` | 生成上传计划、dry-run、执行上传 |
| `ops` | 查看操作记录、撤销支持的移动/重命名操作 |
| `schema` / `list` | 输出命令和 plan schema，供 AI / MCP 使用 |

## 常用命令

### 账号与能力

```bash
clouddrive-cli auth list --json
clouddrive-cli settings show --json
clouddrive-cli providers capabilities --json
```

### 读取目录

```bash
clouddrive-cli files list --provider aliyun --account default --file-id root --json
clouddrive-cli files walk --provider aliyun --account default --file-id root --output files.json --json
clouddrive-cli files tree --provider aliyun --file-id root --depth 3 --json
clouddrive-cli files stats --provider aliyun --file-id root --depth 2 --json
```

### 搜索与详情

```bash
clouddrive-cli files search --provider aliyun --name "report" --json
clouddrive-cli files info --provider aliyun --file-id <file-id> --json
```

### 下载

```bash
clouddrive-cli files download --provider aliyun --file-id <file-id> --output ./download.bin --json
```

### 新建目录

```bash
clouddrive-cli files mkdir --provider aliyun --parent root --name "New Folder" --json
```

### 上传

```bash
clouddrive-cli upload plan --local ./backup --provider aliyun --remote-parent root --output upload-plan.json --json
clouddrive-cli upload apply upload-plan.json --dry-run --json
clouddrive-cli upload apply upload-plan.json --rationale "User requested backup" --json
```

### 移动 / 重命名 / 删除

写操作建议由 AI 或脚本先生成 plan，再 dry-run。

先用机器可读 schema 确认 plan 格式：

```bash
clouddrive-cli schema plans --json
clouddrive-cli schema plans --name rename --json
```

```bash
clouddrive-cli files move-apply move-plan.json --dry-run --json
clouddrive-cli files move-apply move-plan.json --rationale "User approved moving these files" --json

clouddrive-cli files rename-apply rename-plan.json --dry-run --json
clouddrive-cli files rename-apply rename-plan.json --rationale "User approved renaming these files" --json

clouddrive-cli files trash-apply trash-plan.json --json
clouddrive-cli files trash-apply trash-plan.json --apply --rationale "User confirmed deleting these files" --json
```

### 操作记录与撤销

```bash
clouddrive-cli ops list --json
clouddrive-cli ops show <operation-id> --json
clouddrive-cli ops undo <operation-id> --dry-run --json
clouddrive-cli ops undo <operation-id> --rationale "User approved undoing this operation" --json
```

## AI Agent 安全规则

| 规则 | 说明 |
|---|---|
| 先发现 | 先运行 `clouddrive-cli list --json`，不要猜命令。 |
| 先看 plan schema | 写 plan 前运行 `clouddrive-cli schema plans --json`。 |
| 先能力 | 写操作前运行 `providers capabilities --json`。 |
| 不编造 ID | `file-id` 必须来自 `files list`、`files walk`、`files search`、`files tree` 或 `files info`。 |
| 大输出落盘 | `largeOutput: true` 的命令优先使用 `--output <file.json>`。 |
| 先 dry-run | `requiresDryRun` 的命令必须先预览。 |
| 写入理由 | 真实执行 apply 时必须带 `--rationale <reason>` 记录原因。 |

## MCP

启动 MCP Server：

```bash
clouddrive-mcp
```

MCP 工具由 `clouddrive-cli schema commands` 生成，因此只会暴露当前保留的网盘命令。对 `requiresDryRun` 的工具，MCP 默认执行 dry-run；如果要真实执行，必须同时传 `dry_run: false`、`confirm_apply: true` 和 `rationale`。

## 配置目录

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

覆盖配置目录：

```bash
CLOUDDRIVE_CLI_CONFIG_DIR=/path/to/config clouddrive-cli auth list --json
```
