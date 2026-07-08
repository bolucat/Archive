---
name: clouddrive-cli
description: Use when an agent needs to operate cloud-drive accounts through clouddrive-cli or clouddrive-mcp: discover logged-in accounts, inspect/list/walk/search cloud files, read local documents as context, generate or apply safe media rename/move/trash/upload/organize plans, and roll back supported operations.
allowed-tools:
  - Bash(clouddrive-cli:*)
  - Bash(clouddrive-mcp:*)
  - Read
---

# clouddrive-cli

`clouddrive-cli` turns BoxPlayer cloud-drive accounts into a deterministic command surface for humans and AI agents. It supports Aliyun Drive, OneDrive, Dropbox, Box, 123Pan, 115 Drive, Baidu Netdisk, and PikPak.

This skill is the orientation layer. Discover the live CLI surface first, then choose the smallest safe command path for the user's goal.

## The three pillars

- **Account and provider discovery** — `auth list`, `settings show`, and `providers capabilities` tell you which accounts exist and which operations each provider supports.
- **Cloud file operations** — `files list|walk|tree|stats|info|download|search|mkdir|rename-apply|move-apply|trash-apply` provide the stable boundary for reading and changing cloud files.
- **Agent workflows** — `docs read`, `media *`, `upload *`, `organize *`, and `ops *` let agents combine local instructions, cloud inventory, dry-run plans, execution, and rollback.

## Install and locate

```bash
which clouddrive-cli
clouddrive-cli --help
clouddrive-cli list --format json
```

If the command is missing, ask the user to install it from BoxPlayer account settings or with `npm install -g clouddrive-cli`.

There are two install modes:

- **BoxPlayer app mode** — the Electron app registers `clouddrive-cli` and exports logged-in account tokens.
- **Standalone CLI mode** — install with npm and use `auth login` or `auth import-token` to add accounts.

## Discover first

Do not treat this file as the complete command registry. Start with live discovery:

```bash
clouddrive-cli list --format json
clouddrive-cli schema commands
clouddrive-cli auth list --format json
clouddrive-cli settings show --format json
clouddrive-cli providers capabilities --format json
```

`clouddrive-cli list --format json` is the source of truth for command metadata such as `access`, `requiresDryRun`, `destructive`, and `undoable`. `providers capabilities` is the source of truth for provider-specific support such as search, mkdir, move, rename, trash/delete, upload, recursive walk, path addressing, and file-id addressing.

## Universal conventions

| convention | meaning |
|------------|---------|
| `--format json` / `--json` | Agent-friendly JSON output. Prefer this by default. |
| JSON errors | Failures return `{ ok: false, error: { code, message, exitCode } }` in JSON mode. Branch on `error.code`, not message text. |
| dry-run first | Commands marked `requiresDryRun` must be previewed before execution. |
| capability check | Before mkdir, move, rename, trash, upload, or organize apply, check `providers capabilities`. |
| large output | Commands with `largeOutput: true` should use `--output <file.json>` so stdout stays a compact summary. |
| rationale | Write commands accept `--rationale <reason>`; use it to preserve why the Agent is acting. |
| no invented IDs | Get file IDs from `files list`, `files walk`, `files search`, `files tree`, or `files info`. |

Common error codes:

| code | meaning | response |
|------|---------|----------|
| `AUTH_ERROR` | Missing/expired account or login failure. | Run `auth list`, `auth check`, `auth login`, or ask for token import. |
| `PROVIDER_API_ERROR` | Provider returned an API/HTTP error. | Retry only if transient; otherwise record provider limitation. |
| `UNSUPPORTED_CAPABILITY` | Provider or command capability is unavailable. | Stop or choose a supported fallback. |
| `VALIDATION_ERROR` | Bad args, missing files, invalid plan, unknown operation. | Fix input before retrying. |

## Read workflows

Use bounded reads before large traversals:

```bash
clouddrive-cli files stats --provider aliyun --account default --file-id root --depth 2 --format json
clouddrive-cli files stats --provider aliyun --account default --file-id root --output stats.json --format json
clouddrive-cli files tree --provider aliyun --account default --file-id root --depth 1
clouddrive-cli files list --provider aliyun --account default --file-id root --format json
clouddrive-cli files list --provider aliyun --account default --file-id root --limit 100 --format json
clouddrive-cli files list --provider aliyun --account default --file-id root --limit 100 --cursor <nextCursor> --format json
clouddrive-cli files download --provider aliyun --account default --file-id <file-id> --output ./download.bin --format json
clouddrive-cli files search --provider aliyun --account default --name "movie" --limit 50 --format json
```

Rules of thumb:

- Use `stats` or shallow `tree` to understand scale.
- Use `search` for known keywords.
- Use `walk` only after narrowing scope or when a full inventory is explicitly needed.
- Avoid raw `files tree --json` and broad `walk` on large roots unless you will save and summarize the output locally.

Provider root defaults when `--file-id` is omitted:

| provider | default root |
|----------|--------------|
| aliyun | `root` |
| cloud123 | `0` |
| 115 | `0` |
| baidu | `/` |
| pikpak | `*` |
| dropbox | empty string |
| onedrive | `onedrive_root` |
| box | `box_root` |

## Write workflows

Never apply a write plan first. The safe sequence is:

1. Discover command metadata with `clouddrive-cli list --format json`.
2. Check provider capability with `clouddrive-cli providers capabilities --format json`.
3. Generate or read a plan.
4. Run the dry-run or preview command.
5. Show the user a concise summary of planned changes.
6. Execute only after explicit user approval.
7. Record the operation ID and rollback path when available.

Include `--rationale` on write commands, for example:

```bash
clouddrive-cli upload apply upload-plan.json --dry-run --rationale "User asked to back up this folder" --format json
```

Common write previews:

```bash
clouddrive-cli files rename-apply rename-plan.json --current files.json --dry-run --format json
clouddrive-cli files move-apply move-plan.json --dry-run --format json
clouddrive-cli files trash-apply trash-plan.json --format json
clouddrive-cli upload apply upload-plan.json --dry-run --format json
clouddrive-cli organize apply organize-plan.json --dry-run --summary --format json
```

Notes:

- `trash-apply` defaults to preview; execution requires `--apply`.
- Rename and move operations are undoable through `ops undo`; trash is not.
- Dropbox and Baidu move/trash plans may require path fields in addition to file IDs.
- Upload planning exists for all providers. Real byte upload is wired for providers whose `providers capabilities` entry has `uploadFile: true` (`aliyun`, `cloud123`, `115`, `baidu`, `dropbox`, `onedrive`, `box` in this build); `pikpak` remains plan/dry-run only.

## Media organization

Recommended flow:

```bash
clouddrive-cli docs read ./rules.md --max-chars 50000 --format json
clouddrive-cli docs read ./rules.pdf --pdf-format markdown --pdf-pages 1-3 --format json
clouddrive-cli docs convert ./pdf-folder --output ./out --pdf-format json,html,pdf,markdown,tagged-pdf,text --format json
clouddrive-cli files stats --provider aliyun --account default --file-id <folder-id> --depth 2 --format json
clouddrive-cli files walk --provider aliyun --account default --file-id <folder-id> --output files.json --format json
clouddrive-cli media match --input files.json --format json
# Generate rename-plan.json yourself from rules, inventory, and match output.
clouddrive-cli files rename-apply rename-plan.json --dry-run --format json
```

`media match` returns media fields under `match`, for example `{ fileId, name, match: { type, title, year, season, episode, confidence, jellyfin_name } }`.

For full-drive organization, treat broad plans as risky even if dry-run succeeds. Sample suspicious moves, check whether existing meaningful folders would be flattened into generic `Movies` or `TV Shows`, and prefer applying one folder/category at a time.

## Rollback

```bash
clouddrive-cli ops list --format json
clouddrive-cli ops show <operation-id> --format json
clouddrive-cli ops undo <operation-id> --dry-run --format json
clouddrive-cli ops undo <operation-id> --format json
```

Always run `ops undo --dry-run` first. Only rename and move operations have CLI undo.

## MCP

`clouddrive-mcp` exposes the CLI manifest as MCP tools. Tool names and input schemas are generated from the same command manifest used by `clouddrive-cli list --format json`, so CLI/MCP discovery should stay aligned. Prefer direct CLI commands when:

- You need file output redirection or plan files.
- You are testing CLI behavior during skill/CLI iteration.

## Known provider limitations

- OneDrive `files search` may intermittently fail with Microsoft Graph `generalException` / HTTP 500. Use `files walk` plus local filtering as fallback.
- PikPak `files search --name` may ignore the name query and return root items. Use `files walk` plus local filtering.
- Baidu `files info` handles `filemetas` under either `list` or `info`, but `errno=12` can still indicate an API/token limitation. Fall back to `files walk` or `files search`.
- 115 Open API folder fields may map folders as `type: file`. Confirm folder-ness by trying `files list --file-id <fileId>` before planning folder operations.

## Don't

- Don't paste this skill's command list into a plan; call `clouddrive-cli list --format json`.
- Don't run non-dry-run writes without explicit user approval.
- Don't assume root path tokens are portable across providers.
- Don't continue after a structured JSON error without branching on `error.code`.
- Don't emit huge raw inventories to the user; save, summarize, and sample.
