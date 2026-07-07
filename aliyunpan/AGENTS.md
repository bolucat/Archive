# AGENTS.md — BoxPlayer (aliyunpan)

## Quick reference

```bash
pnpm install          # install deps (pnpm only, never npm/yarn)
pnpm dev              # hot-reload Electron dev server
pnpm run build        # typecheck + bundle (vue-tsc --noEmit && vite build)
pnpm run test         # focused Vitest suite (not full repo)
pnpm run test:clouddrive-cli  # clouddrive-cli tests only
```

## Package manager: pnpm only

Lockfile is `pnpm-lock.yaml`. `package-lock.json` and `yarn.lock` are in `.gitignore`.
Never use `npm install` or `yarn`.

## Node version

`engines.node >= 18.0.0` in package.json.

## Build pipeline order matters

`pnpm run build` runs **version bump → typecheck → vite bundle**. The version bump (`version.mjs`) auto-increments the patch version in package.json before every build. The build removes `dist/` and `release/` directories on start.

`pnpm run build:electron` calls `build` first, then runs `electron-builder`.

For platform-specific packaging:
```bash
pnpm run build:mac       # unsigned macOS
pnpm run build:mac:signed  # signed + notarized
pnpm run build:linux
pnpm run build:windows
pnpm run build:all       # cross-platform, sequential
```

## Sensitive config: clean/restore cycle

API keys live in `src/config.ts`. Committing real keys is prevented by a pre-commit hook (`nano-staged`) that calls `scripts/clean-config.js` to blank the values. After commit, restore with:

```bash
pnpm run config:restore
```

Before building for distribution, ensure `src/config.ts` has real keys — never build from the cleaned version.

`scripts/` directory has 10 scripts. Only `scripts/koodo-reader-smoke.mjs` is tracked in git (see `.gitignore`).

## Architecture: Electron + Vue 3 + Vite

| Directory | Purpose |
|---|---|
| `electron/main/` | Electron main process (entry: `electron/main/index.ts`) |
| `electron/preload/` | Preload scripts |
| `src/` | Vue 3 renderer (components, views, stores, API modules) |
| `shared/` | Code shared between main + renderer |
| `scripts/` | Build, CLI entry points, config management |
| `clouddrive-cli/` | Standalone CLI package (published to npm as `clouddrive-cli`) |

Path aliases (set in `tsconfig.json`, `vite.config.ts`, `vitest.config.ts`):
- `@shared/*` → `shared/*`
- `@main/*` → `electron/main/*`

Cloud provider API modules in `src/`: `aliapi/`, `cloudbaidu/`, `cloud123/`, `cloud115/`, `pikpak/`, `onedrive/`, `box/`, `dropbox/`, `quark/`, `cloud139/`, `cloud189/`.

## Testing: Vitest, Node environment, selective includes

Tests run in Node environment (not jsdom). The `vitest.config.ts` lists explicit test directories — not a glob over `**/*.test.ts`. Tests outside these paths are not picked up:

- `electron/main/core/__tests__/`, `electron/main/aria/__tests__/`
- `shared/__tests__/`, `scripts/__tests__/`
- `src/down/motrix-integration/`, `src/media-server/__tests__/`
- `src/utils/__tests__/`, `src/aliapi/__tests__/`
- `src/pikpak/__tests__/`, `src/quark/__tests__/`
- `src/dropbox/__tests__/`, `src/onedrive/__tests__/`, `src/box/__tests__/`
- `clouddrive-cli/__tests__/`

Default `pnpm run test` runs only a subset (aria2 tests, motrix integration, a couple utils files).

When adding tests in a new directory, update `vitest.config.ts`.

## Pre-commit hooks (nano-staged)

Defined in `nano-staged.mjs`:
- JS/TS: `prettier --write` + `eslint --cache --fix`
- Vue files: `stylelint --fix` + `prettier --write` + `eslint --cache --fix`
- CSS/Less: `stylelint --fix` + `prettier --write`
- Typecheck (`npm run typecheck`) runs on changed files under `src/`

## Formatting conventions

From `.prettierrc`:
- Single quotes, no semicolons
- 260 printWidth (very wide)
- No trailing commas
- LF line endings
- `sortAttributes: true` on HTML/Vue

## TypeScript

- `tsconfig.json` targets ESNext, moduleResolution node, strict mode, `@shared/*` and `@main/*` path aliases
- Includes `src/` and `shared/`, excludes `shared/__tests__`
- References `tsconfig.node.json` for Electron-side code

## provider integration checklist

When adding new cloud-drive providers, follow the 15-step checklist in `AGENT.md`. This file is in `.gitignore`, thus local-only. Key steps: OAuth flow, provider detection, file listing, download/playback, search, thumbnails, file operations, sharing, upload, media scanning, folder picker, capability boundaries, properties/recycle bin, tests, and verification.

## clouddrive-cli

Standalone CLI + MCP server for agent-driven cloud-drive operations. Docs: `clouddrive-cli/README.md`. The CLI supports 8 providers and has its own npm package (`clouddrive-cli`). Separate test command: `pnpm run test:clouddrive-cli`.

## CI

Manual trigger only (`workflow_dispatch`) via `.github/workflows/release.yml`. Builds on `windows-latest` + `ubuntu-latest`, publishes draft GitHub Release. No automatic CI on push/PR.

## Subprojects (gitignored, referenced locally)

`Motrix/`, `koodo-reader/`, `CloudServiceKit/`, `XbyVideoHub/`, `OpenCLI/`, `QuarkPan/`, `PikPakAPI/` — these are local-only and not part of the main repo build.
