# Release executable size

Measured on 2026-10-21 with the locked dependencies, windows-rs submodule
`75c2364d1ba98d4b1fb9a9f28e2a9b0fc067e6e6`, and
`rustc 1.100.0-nightly (a36d05efa 2026-09-09)`, targeting Windows x64 MSVC.
Sizes are the actual uncompressed EXE, including its icon and manifest.

| Configuration | Bytes | MiB | Reduction from baseline |
| --- | ---: | ---: | ---: |
| Original default release profile | 3,802,624 | 3.63 | — |
| Full LTO, one codegen unit, `s`, abort, strip | 1,773,056 | 1.69 | 53.4% |
| Full LTO, one codegen unit, `z`, abort, strip | 1,442,304 | 1.38 | 62.1% |
| Same `z` profile, size-optimized standard library | 1,302,016 | 1.24 | 65.8% |

`z` saves 330,752 bytes compared with `s` in this application. Rebuilding the
standard library saves another 140,288 bytes. These measurements select the
release profile and the release workflow's `build-std` options; they do not
establish an absolute lower bound. Compiler updates and architectures can change
the result.

The original PE contained 2,680,320 bytes of code, 894,976 bytes of read-only data,
and only 55,808 bytes of icon/manifest resources. Optimizing generated code
therefore provided much more benefit than changing the artwork. The implementation
keeps all six languages, the GUI, CLI, and existing runtime deployment model.
No executable packer is used.

Full LTO trades build time for size. Size optimization may trade throughput for
size; interactive behavior was checked, but this is not a performance benchmark.
`panic = "abort"` terminates on programming panics instead of unwinding; it does
not change ordinary `Result` error handling. Native GUI callbacks already cannot
unwind across the Windows ABI. Tests retain the test harness's unwind behavior.
Symbol stripping reduces postmortem debugging information. The additional
standard-library optimization requires nightly's unstable `build-std` support
and `rust-src`; the normal release command remains available without it.

Validation:

- All 52 application tests passed in both development and release profiles.
- `cargo clippy --all-targets --locked -- -D warnings` and application formatting
  checks passed.
- ARM64 compilation reached the final link, but this machine has only x64/x86
  MSVC build tools and no ARM64 linker. Its EXE size and runtime behavior remain
  unverified locally; the release workflow retains both architecture targets.
- The smallest x64 EXE loaded all 136 real AppContainers. Native GUI checks
  exercised checkbox edits and counter updates, refresh cancellation and refresh,
  dirty-close confirmation and cancellation, and clean close. Firewall exemptions
  were compared before/after and were unchanged; Save was never invoked.
- The repository's existing GUI smoke script has stale sort-header expectations
  and requests a SelectionItemPattern unavailable on this runtime's rows; the
  original baseline fails those checks too. A temporary adapter under
  `target/size-study` used the current header names, omitted native selection/sort
  assertions, and waited for dialog dismissal animations. Selection and sorting
  are covered by the application tests, but were not verified through native UIA.

Reproduce the smallest build with the command in the README. Intermediate x64
EXEs from this investigation are retained locally in `target/size-study/`.
