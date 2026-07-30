# clouddrive-cli

> BoxPlayer's multi-cloud-drive CLI. It exposes cloud-drive file management only. Media-library organization, document reading, AI reading, media-server, and playback features are intentionally not exposed.

[中文](./README.md) | English

## Scope

`clouddrive-cli` turns BoxPlayer cloud-drive accounts into deterministic terminal and AI-agent commands.

Included:

- account discovery and defaults
- provider capability discovery
- file list / walk / tree / stats / info
- provider-side search
- download
- folder creation
- upload plan / dry-run / apply
- move / rename / trash plan execution
- operation history and undo for supported operations
- MCP schema for the same cloud-drive commands

Not included:

- media scan / media naming
- media-library organization
- local document reading / PDF conversion
- AI search, AI reader, media acquisition, follow-up tracking
- media servers, playback, subtitles, danmaku

## Providers

```text
aliyun · cloud123 · 115 · 139 · 189 · quark · pikpak · dropbox · onedrive · box · baidu
```

Check live capabilities:

```bash
clouddrive-cli providers capabilities --json
```

## Install

From BoxPlayer:

```text
BoxPlayer → Account Settings → Install CLI
```

Standalone:

```bash
npm install -g clouddrive-cli
```

From source:

```bash
node clouddrive-cli/bin/cli.mjs --help
```

## Command discovery

Do not hard-code README examples. The CLI exposes a machine-readable command list:

```bash
clouddrive-cli list --json
clouddrive-cli schema commands
clouddrive-cli schema plans
```

Command groups:

| Group | Purpose |
|---|---|
| `auth` | list accounts, set defaults, import tokens, login |
| `settings` | show config directory, account summary, defaults |
| `providers` | list provider capabilities |
| `files` | list, walk, tree, stats, info, search, download, mkdir, move, rename, trash |
| `upload` | upload plan, dry-run, apply |
| `ops` | operation history and undo |
| `schema` / `list` | machine-readable command and plan schemas |

## Examples

### Accounts and capabilities

```bash
clouddrive-cli auth list --json
clouddrive-cli settings show --json
clouddrive-cli providers capabilities --json
```

### Read directories

```bash
clouddrive-cli files list --provider aliyun --account default --file-id root --json
clouddrive-cli files walk --provider aliyun --account default --file-id root --output files.json --json
clouddrive-cli files tree --provider aliyun --file-id root --depth 3 --json
clouddrive-cli files stats --provider aliyun --file-id root --depth 2 --json
```

### Search and info

```bash
clouddrive-cli files search --provider aliyun --name "report" --json
clouddrive-cli files info --provider aliyun --file-id <file-id> --json
```

### Download

```bash
clouddrive-cli files download --provider aliyun --file-id <file-id> --output ./download.bin --json
```

### Create folder

```bash
clouddrive-cli files mkdir --provider aliyun --parent root --name "New Folder" --json
```

### Upload

```bash
clouddrive-cli upload plan --local ./backup --provider aliyun --remote-parent root --output upload-plan.json --json
clouddrive-cli upload apply upload-plan.json --dry-run --json
clouddrive-cli upload apply upload-plan.json --rationale "User requested backup" --json
```

### Move / rename / trash

Inspect the machine-readable plan schema before writing plan JSON:

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

### Operations and undo

```bash
clouddrive-cli ops list --json
clouddrive-cli ops show <operation-id> --json
clouddrive-cli ops undo <operation-id> --dry-run --json
clouddrive-cli ops undo <operation-id> --rationale "User approved undoing this operation" --json
```

## AI-agent safety rules

| Rule | Description |
|---|---|
| Discover first | Run `clouddrive-cli list --json`; do not guess commands. |
| Inspect plan schemas | Run `clouddrive-cli schema plans --json` before writing plan JSON. |
| Check capabilities | Run `providers capabilities --json` before writes. |
| Do not invent IDs | `file-id` must come from `files list`, `files walk`, `files search`, `files tree`, or `files info`. |
| Save large output | Prefer `--output <file.json>` for commands marked `largeOutput: true`. |
| Dry-run first | Commands marked `requiresDryRun` must be previewed first. |
| Record rationale | Real apply operations require `--rationale <reason>`. |

## MCP

Start the MCP server:

```bash
clouddrive-mcp
```

MCP tools are generated from `clouddrive-cli schema commands`, so only the retained cloud-drive commands are exposed. For tools marked `requiresDryRun`, MCP defaults to dry-run; real apply requires `dry_run: false`, `confirm_apply: true`, and `rationale`.

## Config directory

Default:

```bash
~/.clouddrive-cli
```

Files:

```text
tokens.json
config.json
operations/
```

Override:

```bash
CLOUDDRIVE_CLI_CONFIG_DIR=/path/to/config clouddrive-cli auth list --json
```
