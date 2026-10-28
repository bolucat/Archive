# Nyanpasu Utils

Shared utilities for Nyanpasu UI and Nyanpasu Service.

The repository contains a single `nyanpasu-utils` crate. Utilities are organized as feature-gated
modules so the crate can be embedded in a larger monorepo without nested Cargo workspaces:

- `core` — proxy-core process lifecycle management (`core_manager` feature)
- `dirs` — platform-aware application directories (`dirs` feature)
- `io` and `runtime` — shared IO and Tokio runtime helpers
- `network` — platform-specific network configuration (`network` feature)
- `os` — operating-system and process helpers (`os` feature)
- `process` — supervised children and versioned per-epoch PID records. Orphan
  termination uses the same validated process handle on Windows and a pidfd on
  supported Linux kernels. Other Unix targets immediately revalidate before a
  PID signal, with a documented residual PID-reuse window. Reaping captures a
  live descendant tree before killing the root, then identity-validates and
  confirms every captured descendant independently. A descendant that reparents
  before either capture snapshot observes it cannot be safely attributed and
  may remain. A live PID whose start token differs from the record was reused
  by another process: the record is retired and nothing is signaled. A
  matching start token with a different or unreadable executable stays an
  error. macOS reads start tokens through `sysctl(KERN_PROC_PID)` so that
  processes of other users are compared too.

Epoch record staging files are swept on the next manager startup. A narrow gap
remains between process creation and identity-record publication; an orphan
from that interval is deliberately not killed without authoritative identity.

Default features preserve the existing public API. Consumers that need a smaller dependency surface
can disable default features and enable only the modules they use.

## `nyanpasu-log-preview`

`log-preview/` holds a second, dependency-light crate that formats a byte or text payload — an
image, a profile, a script — as its length, CRC-32 and first and last bytes, so it can be logged
without printing it whole. It depends only on `crc32fast`, so crates that must not pull in Tokio,
such as the statistic widget's protocol, can use it directly. `nyanpasu-utils` re-exports it as
`log_preview`. It is a path dependency rather than a workspace member, which keeps this repository
free of a nested workspace.
