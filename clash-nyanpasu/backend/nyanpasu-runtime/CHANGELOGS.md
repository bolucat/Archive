
## [2.0.0] - 2026-10-15

### 💥 Breaking Changes

- **clash-api:** Select proxies through a named selection by @greenhat616 

- **clash-api:** Take retry delay bounds by name by @greenhat616 

- **core-log:** Unify frame types across manager and IPC by @greenhat616 

- **core-manager:** Take HealthPolicy inputs by name by @greenhat616 

- **core-manager:** Type the epochs the manager issues by @greenhat616 

- **core-manager:** Delete ControllerMode, make LocalIpcPolicy the only knob (S10) by @greenhat616 

- **ipc:** Drop the v1 /core/restart wire by @greenhat616 

- **ipc:** Drop the v1 /core/recover wire by @greenhat616 

- **ipc:** Drop the v1 /core/apply wire by @greenhat616 

- **ipc:** Replace the error_kind string constants with CoreErrorKind by @greenhat616 

- **ipc:** One unversioned /ws/events stream (S11) by @greenhat616 

- **ipc,service:** Retire Event::Log, publish the log directories (L3) by @greenhat616 



### ⚡ Performance Improvements

- **logging:** Use gxhash for indexes and viewer sessions (#420) by @greenhat616  in [#420](https://github.com/libnyanpasu/nyanpasu-runtime/pull/420)

- **logging:** Use gxhash for indexes and viewer sessions by @greenhat616 


### ✅ Testing

- **core-manager:** Prove the runtime boundary and quarantine contract with an in-memory backend by @greenhat616 

- **core-manager:** Real-core integration suite + clash-rs ipc flag, meow profile, noop classify (#385) by @greenhat616  in [#385](https://github.com/libnyanpasu/nyanpasu-runtime/pull/385)

- **core-manager:** Retry the whole proxy roundtrip, not just the connect by @greenhat616 

- **core-manager:** Serialize real-core tests by @greenhat616 

- **core-manager:** Real-core integration suite with socks5 harness by @greenhat616 

- **jobs:** Verify lifecycle and journal integration (#412) by @greenhat616  in [#412](https://github.com/libnyanpasu/nyanpasu-runtime/pull/412)

- **routing:** Probe the v2 mounts and pin outcomes by @greenhat616 

- **service:** Deterministic bridge-test handshake; docs: precise R9 Lagged wording by @greenhat616 


### ✨ Features

- **clash-api:** Serialize unknown connection fields under _extra (#431) by @greenhat616  in [#431](https://github.com/libnyanpasu/nyanpasu-runtime/pull/431)

- **clash-api:** Return owned typed WebSocket subscriptions (#406) by @greenhat616  in [#406](https://github.com/libnyanpasu/nyanpasu-runtime/pull/406)

- **cli:** Backward-compatible ergonomics batch (S5) by @greenhat616 

- **core:** Reconcile controller policies and report effective commits (#414) by @greenhat616  in [#414](https://github.com/libnyanpasu/nyanpasu-runtime/pull/414)

- **core:** Expose process-bound API connection metadata by @greenhat616 

- **core-manager:** Add the long-poll operation wait for RPC hosts by @greenhat616 

- **core-manager:** Own the host DNS override at fixed transaction phases by @greenhat616 

- **core-manager:** Add the CoreControl plane with a cancellation-isolated executor by @greenhat616 

- **core-manager:** Add the unified reconcile convergence transaction by @greenhat616 

- **core-manager:** Add the portable control-plane command surface by @greenhat616 

- **core-manager:** Classify errors with Error::kind() by @greenhat616 

- **core-manager:** Archive core logs as rotating JSONL (L1) by @greenhat616 

- **core-manager:** Meow launch profile, clash-rs ipc flag, noop classify by @greenhat616 

- **core-manager:** Gate the managed controller on core capability (#384) by @greenhat616  in [#384](https://github.com/libnyanpasu/nyanpasu-runtime/pull/384)

- **core-metadata:** Extend the wire table with the control-plane admission kinds by @greenhat616 

- **core-metadata:** Add CoreErrorKind, the single error-kind wire table by @greenhat616 

- **core-metadata:** Add crate for core kinds and feature version floors by @greenhat616 

- **ipc:** Carry retryable on the response envelope by @greenhat616 

- **ipc:** Add the additive v2 control-plane wire by @greenhat616 

- **ipc:** Expose the typed error kind on the client error by @greenhat616 

- **ipc:** Event protocol v2 - full status snapshots over /ws/events (S9) by @greenhat616 

- **ipc:** /core/apply, /core/check and /core/recover (S8) by @greenhat616 

- **ipc:** Enrich the status payload with controller, health, revision and state detail (S7) by @greenhat616 

- **ipc,service:** Push core logs on their own event ring (L2) by @greenhat616 

- **jobs:** Allow disabling per-job log capture (#432) by @greenhat616  in [#432](https://github.com/libnyanpasu/nyanpasu-runtime/pull/432)

- **jobs:** Implement actor-owned execution and scheduling (#411) by @greenhat616  in [#411](https://github.com/libnyanpasu/nyanpasu-runtime/pull/411)

- **jobs:** Add owned contexts and scoped log capture (#410) by @greenhat616  in [#410](https://github.com/libnyanpasu/nyanpasu-runtime/pull/410)

- **jobs:** Add a transactional redb journal (#409) by @greenhat616  in [#409](https://github.com/libnyanpasu/nyanpasu-runtime/pull/409)

- **jobs:** Define run contracts and scheduling types (#408) by @greenhat616  in [#408](https://github.com/libnyanpasu/nyanpasu-runtime/pull/408)

- **logging:** List flexi_logger-named log files (#428) by @greenhat616  in [#428](https://github.com/libnyanpasu/nyanpasu-runtime/pull/428)

- **logging:** Own bounded viewer sessions in a logs actor by @greenhat616 

- **logging:** Add bounded owned file index by @greenhat616 

- **runtime:** Support the independent Meow Alpha channel (#442) by @keiko233  in [#442](https://github.com/libnyanpasu/nyanpasu-runtime/pull/442)

- **service:** Authorize Windows core named pipes (#424) by @greenhat616  in [#424](https://github.com/libnyanpasu/nyanpasu-runtime/pull/424)

- **service:** Query service log files through viewer sessions by @greenhat616 

- **service:** Host the v2 control plane behind the additive wire by @greenhat616 


### 🐛 Bug Fixes

- **clash-api:** Read proxy groups from Mihomo and Meow (#441) by @greenhat616  in [#441](https://github.com/libnyanpasu/nyanpasu-runtime/pull/441)

- **clash-api:** Accept base proxy and provider responses (#405) by @greenhat616  in [#405](https://github.com/libnyanpasu/nyanpasu-runtime/pull/405)

- **clash-api:** Preserve partial runtime config responses (#404) by @greenhat616  in [#404](https://github.com/libnyanpasu/nyanpasu-runtime/pull/404)

- **clash-api:** Preserve optional and unknown provider metadata (#403) by @greenhat616  in [#403](https://github.com/libnyanpasu/nyanpasu-runtime/pull/403)

- **clash-api:** Accept rules without optional core metadata by @greenhat616 

- **clash-api:** Default missing meta in /version for legacy cores by @greenhat616 

- **control:** Give every operation a terminal state when the executor dies by @greenhat616 

- **control:** Atomic admission, hard live capacity, digest in identity, per-call check files by @greenhat616 

- **core:** Manage native storage ownership and lifecycle (#430) by @4o3F  in [#430](https://github.com/libnyanpasu/nyanpasu-runtime/pull/430)

- **core-manager:** Keep the source mapping order in runtime configs (#427) by @greenhat616  in [#427](https://github.com/libnyanpasu/nyanpasu-runtime/pull/427)

- **core-manager:** Hold ctrl through sink finalization in shutdown (review R3) by @greenhat616 

- **core-manager:** Compare effective configs before noop by @greenhat616 

- **core-manager,service:** Prove sink termination, finalize on error paths, protect the active file (review R2) by @greenhat616 

- **core-manager,service:** Bound the log sink's lifecycle, batches and records (review R1) by @greenhat616 

- **dns:** Durable fail-closed record, stable previous baseline, bounded controller calls by @greenhat616 

- **ipc:** Identify service requests by hostname (#423) by @greenhat616  in [#423](https://github.com/libnyanpasu/nyanpasu-runtime/pull/423)

- **release:** Let Publish advance the service to rc.10 by @4o3F 

- **scripts:** Treat DirEntry type flags as properties, not methods by @greenhat616 

- **scripts:** Overwrite the existing raw binary under --force by @greenhat616 

- **service:** Stop the core before exiting on a signal (#435) by @greenhat616  in [#435](https://github.com/libnyanpasu/nyanpasu-runtime/pull/435)

- **service:** Commit the v2 type echo only for a newly admitted reconcile that stuck by @greenhat616 

- **service:** Route daemon shutdown through the executor and supervise its exit by @greenhat616 

- **service:** Do not log environment variable values by @greenhat616 

- **service,core-manager:** Publish v2 echo snapshots, classify two error kinds, bound config checks by @greenhat616 

- Point clash-rs downloads at the new upstream (#425) by @greenhat616  in [#425](https://github.com/libnyanpasu/nyanpasu-runtime/pull/425)

- Recover IPC clients from named-pipe contention (#422) by @greenhat616  in [#422](https://github.com/libnyanpasu/nyanpasu-runtime/pull/422)


### 💅 Styling

- **core-manager:** Use as_chunks for the operation-id hex parse by @greenhat616 


### 📚 Documentation

- **core-manager:** Document ipc modes and the real-core suite by @greenhat616 

- **ipc:** Retire the deleted v1 core endpoints from the docs by @greenhat616 

- **ipc:** Record the log-directory readability question as open by @greenhat616 

- **service:** Service evolution design spec (S1-S6) by @greenhat616 

- Correct the graceful-switch claim and the Switched rustdoc by @greenhat616 

- IPC protocol evolution report (S7-S10 proposal) by @greenhat616 


### 🔨 Refactor

- **core-manager:** Name captured stdout and stderr by @greenhat616 

- **core-manager:** Pass core launch paths as one view by @greenhat616 

- **core-manager:** Reconcile probe observations through one state view by @greenhat616 

- **core-manager:** Classify config changes from two plan views by @greenhat616 

- **core-manager:** Allocate epochs under the control lock by @greenhat616 

- **core-manager:** Share the epoch retirement mechanics by @greenhat616 

- **core-manager:** Install every epoch through one path by @greenhat616 

- **core-manager:** Carry the epoch plan whole through Active by @greenhat616 

- **core-manager:** Use zerocopy instead of hand written bytes transmute by @greenhat616 

- **core-manager:** Front the process mechanics with the RuntimeBackend boundary by @greenhat616 

- **core-manager:** Split per-core config rules into clash and mihomo modules by @greenhat616 

- **core-manager:** Group src into config, health and manager modules by @greenhat616 

- **core-metadata:** Split local IPC features by transport by @greenhat616 

- **ipc:** Single-source operation contract + server middleware (S4) by @greenhat616 

- **service:** Read the error kind from the manager instead of remapping it by @greenhat616 

- **service:** Split the service into a runtime lib crate + thin bin (S3) by @greenhat616 

- **service:** EventHub broadcast fan-out + de-globalized server state by @greenhat616 

- **service:** Wire server onto nyanpasu-core-manager by @greenhat616 

- **service-runtime:** Name install program and working directory by @greenhat616 

- **service-runtime:** Name runtime and data directories by @greenhat616 


### 🧹 Miscellaneous Tasks

- **metadata:** Add specta and json schema cap for Feature by @greenhat616 

- **scripts:** Add prepare-cores.ts to download the four cores by @greenhat616 

- **serena:** Migrate project config to `language_servers` key by @greenhat616 

- **submodule:** Bump nyanpasu-utils onto its named-input signatures by @greenhat616 

- Apply linting fixes with rustfmt by @github-actions[bot] 

- Update docs by @greenhat616 

-----------------



## New Contributors
* @4o3F made their first contribution

**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-runtime/compare/v1.4.5...v2.0.0




## [1.4.5] - 2026-07-22


### ✅ Testing

- **clash-api:** Add mihomo end-to-end integration tests by @greenhat616 

- **clash-api:** Cover maintenance routes and queries by @greenhat616 

- **nyanpasu-core-manager:** Add real-mihomo smoke tests by @greenhat616 

- **nyanpasu-core-manager:** Cover graceful-switch rollback and patch fallback by @greenhat616 

- **nyanpasu-core-manager:** Serve the fake core over local transports by @greenhat616 

- **nyanpasu-core-manager:** Make the equivalence gate teardown deterministic by @greenhat616 

- **nyanpasu-core-manager:** Add the legacy lifecycle equivalence gate by @greenhat616 

- **nyanpasu-core-manager:** Cover user stop semantics by @greenhat616 

- **nyanpasu-core-manager:** Cover crash recovery and budget exhaustion by @greenhat616 

- **nyanpasu-core-manager:** Cover immediate-exit startup failure by @greenhat616 

- **nyanpasu-core-manager:** Cover the startup-timeout kill path by @greenhat616 

- **nyanpasu-core-manager:** Add the fake-core simulator and shared test utilities by @greenhat616 

- **nyanpasu-ipc:** Cover every client interface over pipe and unix socket by @greenhat616 


### ✨ Features

- **clash-api:** Add specta types and ordered maps by @greenhat616 

- **clash-api:** Implement typed mihomo client by @greenhat616 

- **clash-api:** Add transport-aware client by @greenhat616 

- **nyanpasu-core-manager:** Customizable health probes with runtime liveness by @greenhat616 

- **nyanpasu-core-manager:** Write-ahead graceful switch + end-to-end orphan recovery by @greenhat616 

- **nyanpasu-core-manager:** Strict diff classification + apply_config compensating saga by @greenhat616 

- **nyanpasu-core-manager:** Manager-owned runtime artifacts + immutable config snapshots by @greenhat616 

- **nyanpasu-core-manager:** Add graceful switching with a degradation matrix by @greenhat616 

- **nyanpasu-core-manager:** Add managed controller mode with derived configs by @greenhat616 

- **nyanpasu-core-manager:** Add derived-config generation with a listener restore plan by @greenhat616 

- **nyanpasu-core-manager:** Add restart and hard switching by @greenhat616 

- **nyanpasu-core-manager:** Add CoreManager start/stop with status publication by @greenhat616 

- **nyanpasu-core-manager:** Add the health-probed single-epoch instance by @greenhat616 

- **nyanpasu-core-manager:** Add the version health probe by @greenhat616 

- **nyanpasu-core-manager:** Add one-shot config checking by @greenhat616 

- **nyanpasu-core-manager:** Add runtime-config introspection and controller resolution by @greenhat616 

- **nyanpasu-core-manager:** Add error, state, and spec types by @greenhat616 

- **nyanpasu-core-manager:** Migrate parse_check_output and add error_summary by @greenhat616 

- **nyanpasu-core-manager:** Add core kinds and launch profiles by @greenhat616 

- **utils:** Land process module via submodule branch; backfill design doc P0-P2/O3 closure by @greenhat616 

- **utils/process:** Add process feature wiring and public event/error types by @greenhat616 


### 🐛 Bug Fixes

- **ci:** Collapse inline tables to single line for cross compatibility by @greenhat616 

- **ci:** Publish ci by @greenhat616 

- **deps:** Update whoami to v2 by @greenhat616 

- **nyanpasu-core-manager:** Avoid never-loop lint on non-Windows by @greenhat616 

- **nyanpasu-core-manager:** Gate test hooks, structure durability errors, keep quarantine status by @greenhat616 

- **nyanpasu-core-manager:** Retryable quarantine recovery + durability surfacing by @greenhat616 

- **nyanpasu-core-manager:** Quarantine unconfirmed stops + drop-safety + durability-aware commits by @greenhat616 

- **nyanpasu-core-manager:** Review fixes — runtime-dir ownership, unconfirmed-stop safety, socket cleanup, dns classification by @greenhat616 

- **nyanpasu-core-manager:** Plug config leaks and stale rollback state by @greenhat616 

- **nyanpasu-core-manager:** Publish terminal states on stop and switch error paths by @greenhat616 

- **nyanpasu-core-manager:** Make managed pipe templates unique per test by @greenhat616 

- **nyanpasu-core-manager:** Give restart/switch tests startup-timeout headroom by @greenhat616 

- **nyanpasu-core-manager:** Cancel supervision when an instance is dropped by @greenhat616 

- **nyanpasu-core-manager:** Restore the crate-level docs dropped in the lib.rs rewrite by @greenhat616 

- **nyanpasu-ipc:** Make the default-pipe SDDL test alias-aware by @greenhat616 


### 💅 Styling

- **nyanpasu-core-manager:** Apply rustfmt to the remaining modules by @greenhat616 

- **nyanpasu-core-manager:** Apply rustfmt to the config module by @greenhat616 


### 📚 Documentation

- Update README by @greenhat616 

- Add README by @greenhat616 

- Mark the core-manager design spec as implemented by @greenhat616 

- Add the nyanpasu-core-manager implementation plan by @greenhat616 

- Add the nyanpasu-core-manager design spec by @greenhat616 

- Add serena dir by @greenhat616 

- Add AGENT guideline by @greenhat616 

- Add process module design spec and implementation plan by @greenhat616 


### 🔨 Refactor

- **core-manager:** Use atomic-fs in nyanpasu-utils by @greenhat616 

- **nyanpasu-ipc:** Rewrite the client with reqwest and reqwest-websocket by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.4.4...v1.4.5




## [1.4.4] - 2026-07-14


### 🐛 Bug Fixes

- **service:** Restore macOS lifecycle operations by @keiko233 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.4.3...v1.4.4




## [1.4.3] - 2026-07-13


### 🐛 Bug Fixes

- **ipc:** Avoid unsupported socket mode on macOS by @keiko233 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.4.2...v1.4.3




## [1.4.2] - 2026-02-07


### 🐛 Bug Fixes

- **deps:** Linux 32bit build by @keiko233 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.4.1...v1.4.2




## [1.4.1] - 2025-07-19


### 🐛 Bug Fixes

- Create service config dir while service install by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.4.0...v1.4.1




## [1.4.0] - 2025-07-18


### ✨ Features

- **acl:** Finish windows acl by @greenhat616 


### 🐛 Bug Fixes

- **core:** Ws report panic and remove UNC prefix by @greenhat616 

- **service:** Add a force kill logic by @greenhat616 

- **service:** Use acl file directly by @greenhat616 

- Example by @greenhat616 


### 🔨 Refactor

- **instance:** Improve instance management by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.3.1...v1.4.0




## [1.3.1] - 2025-05-20


### 🐛 Bug Fixes

- Follow SAFE_PATHS windows styles by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.3.0...v1.3.1




## [1.3.0] - 2025-05-20


### ✨ Features

- Support SAFE_PATHS by @greenhat616 


### 🐛 Bug Fixes

- Ci by @greenhat616 

- Provide service stop pending ttl by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.2.0...v1.3.0




## [1.2.0] - 2025-02-23


### ✨ Features

- **ws:** Support log notify by @greenhat616 

- **ws:** Support core state changed notify by @greenhat616 

- Static crt for windows by @greenhat616 

- Ws events layer and refactor core manager handle to owned by app state by @greenhat616 

- Support win service gracefully shutdown by @greenhat616 

- Specta support by @greenhat616 


### 🐛 Bug Fixes

- Linter by @greenhat616 


### 🔨 Refactor

- Use deno script to get version by @greenhat616 

- Use axum listener trait and axum gracefully shutdown by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.1.3...v1.2.0




## [1.1.3] - 2025-01-09


### 🐛 Bug Fixes

- **server:** Use merge instead of nest for axum by @greenhat616 

- Clippy by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.1.2...v1.1.3




## [1.1.2] - 2025-01-09


### 🐛 Bug Fixes

- **macos:** Support set dns by @greenhat616 

- Lint by @greenhat616 

- Correct exit code usage by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.1.1...v1.1.2




## [1.1.1] - 2025-01-08


### 🐛 Bug Fixes

- Do not replace binary if src and dst are same by @greenhat616 

- Use /usr/bin on linux by @greenhat616 

- Bump axum to 0.8 by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.1.0...v1.1.1




## [1.1.0] - 2024-12-27


### 🐛 Bug Fixes

- **ci:** Try to fix release ci by @greenhat616 

- **lifecycle:** Make service exit gracefully by @greenhat616 

- **macos:** Fix a status check condition by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.0.7...v1.1.0




## [1.0.7] - 2024-10-09


### ✨ Features

- Support clash rs alpha by @greenhat616 


### 🐛 Bug Fixes

- Handle service shutdown signal by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.0.6...v1.0.7




## [1.0.6] - 2024-09-10


### 🐛 Bug Fixes

- Bump simd-json to fix x86 build by @greenhat616 

-----------------



**Full Changelog**: https://github.com/libnyanpasu/nyanpasu-service/compare/v1.0.5...v1.0.6



## [1.0.0] - 2024-07-24

### ✨ Features

- Better version print by @greenhat616 

- Add a logs cmd to get server logs by @greenhat616 

- Add core rpc calls by @greenhat616 

- Add slef update command by @greenhat616 

- Cleanup socket file and cleanup zombie instance before startup by @greenhat616 

- Add deadlock detection and status skip-service-check flag by @greenhat616 

- Add status client rpc check by @greenhat616 

- Add acl for server by @greenhat616 

- Core restart & stop rpc api by @greenhat616 

- Core start rpc api by @greenhat616 

- Service server startup and status inspect rpc by @greenhat616 

- Status command by @greenhat616 

- Restart command by @greenhat616 

- Stop command by @greenhat616 

- Stop command by @greenhat616 

- Start command by @greenhat616 

- Unstall command by @greenhat616 

- Install command by @greenhat616 

- Add core manager util by @greenhat616 

- Draft http client ipc by @greenhat616 

- Draft server ipc by @greenhat616 

- Add config file by @zzzgydi 

- Update by @zzzgydi 

- Add install and uninstall bin by @zzzgydi 


### 🐛 Bug Fixes

- Publish ci by @greenhat616 

- Lint by @greenhat616 

- Macos user ops by @greenhat616 

- Lint by @greenhat616 

- Issues by @greenhat616 

- Ci by @greenhat616 

- Lint by @greenhat616 

- Ci by @greenhat616 

- Ci by @greenhat616 

- Refresh process table before kill process by @greenhat616 

- Check pid whether is exist before killing zombie server by @greenhat616 

- Publish version ctx by @greenhat616 

- Lint by @greenhat616 

- Rpc inspect logs by @greenhat616 

- Process service stop signal by @greenhat616 

- Missing PathBuf mod import by @keiko233 

- Lint by @greenhat616 

- Socket file permission is not changed by @greenhat616 

- Mark socket not execuable by @greenhat616 

- Lint by @greenhat616 

- Lint by @greenhat616 

- Mark unix socket group rw able by @greenhat616 

- State match by @greenhat616 

- Lint by @greenhat616 

- The status query for launchd by @greenhat616 

- Setup windows service manager by @greenhat616 

- Setup windows service manager by @greenhat616 

- Correct macOS group creation command in create_nyanpasu_group function by @keiko233 

- Logging guard is dropped too early by @greenhat616 

- Service manager encoding issue by @greenhat616 

- Lint by @greenhat616 

- Missing use by @greenhat616 

- Issue by @greenhat616 

- Correct server args by @greenhat616 

- Error handling in `check_and_create_nyanpasu_group` function by @keiko233 

- Correct return type for is_nyanpasu_group_exists function by @keiko233 

- Upstream status check by @greenhat616 

- Update dependencies by @greenhat616 

- Rename meta to mihomo and support clash-rs, mihomo-alpha by @greenhat616 


### 📚 Documentation

- Update readme by @greenhat616 


### 🧹 Miscellaneous Tasks

- Bump crates by @greenhat616 

- Apply linting fixes with rustfmt by @github-actions[bot] 

- Use tracing-panic to better capture panic info by @greenhat616 

- Add debug info for os operations by @greenhat616 

- Cleanup deps by @greenhat616 

- Enable tokio-console for debug by @greenhat616 

- Version info use table output by @greenhat616 

- Add editorconfig by @greenhat616 

- Fmt by @greenhat616 

- Add debug print by @greenhat616 

- Apply linting fixes with rustfmt by @github-actions[bot] 

- Add stop advice by @greenhat616 

- Update actions/checkout action to v4 (#24) by @renovate[bot]  in [#24](https://github.com/LibNyanpasu/nyanpasu-service/pull/24)

- Apply linting fixes with rustfmt by @github-actions[bot] 

- Apply linting fixes with rustfmt by @github-actions[bot] 

- Apply linting fixes with clippy by @github-actions[bot] 

- Apply linting fixes with rustfmt by @github-actions[bot] 

- Apply linting fixes with clippy by @github-actions[bot] 

- Commit workspace by @greenhat616 

- Draft api ctx definition by @greenhat616 

- Rename --debug to --verbose by @greenhat616 

- Commit workspace by @greenhat616 

- Commit workspace by @greenhat616 

- Update actions/checkout action to v4 (#3) by @renovate[bot]  in [#3](https://github.com/LibNyanpasu/nyanpasu-service/pull/3)

- Add renovate.json (#2) by @renovate[bot]  in [#2](https://github.com/LibNyanpasu/nyanpasu-service/pull/2)

- Code format by @zzzgydi 

- Ci by @zzzgydi 

- Init by @zzzgydi 

-----------------



## New Contributors
* @github-actions[bot] made their first contribution
* @keiko233 made their first contribution
* @renovate[bot] made their first contribution in [#24](https://github.com/LibNyanpasu/nyanpasu-service/pull/24)
* @zzzgydi made their first contribution



## [1.0.1] - 2024-07-24

### 🐛 Bug Fixes

- Replace dscl to dseditgroup by @keiko233 

- Update rust crate clap to v4.5.10 (#29) by @renovate[bot]  in [#29](https://github.com/LibNyanpasu/nyanpasu-service/pull/29)


### 🧹 Miscellaneous Tasks

- Up by @greenhat616 

- Update rust crate tokio to v1.39.1 (#30) by @renovate[bot]  in [#30](https://github.com/LibNyanpasu/nyanpasu-service/pull/30)

-----------------



**Full Changelog**: https://github.com/LibNyanpasu/nyanpasu-service/compare/v1.0.0...v1.0.1



## [1.0.2] - 2024-07-26

### 🐛 Bug Fixes

- Update rust crate interprocess to 2.2.1 by @renovate[bot]  in [#34](https://github.com/LibNyanpasu/nyanpasu-service/pull/34)


### 🧹 Miscellaneous Tasks

- Update rust crate parking_lot to 0.12.3 by @renovate[bot]  in [#33](https://github.com/LibNyanpasu/nyanpasu-service/pull/33)

- Update rust crate clap to 4.5.10 by @renovate[bot]  in [#32](https://github.com/LibNyanpasu/nyanpasu-service/pull/32)

- Update rust crate axum to 0.7.5 by @renovate[bot]  in [#31](https://github.com/LibNyanpasu/nyanpasu-service/pull/31)

-----------------



**Full Changelog**: https://github.com/LibNyanpasu/nyanpasu-service/compare/v1.0.1...v1.0.2



## [1.0.3] - 2024-07-26

### ✨ Features

- Support sidecar path search and share the status type with ui by @greenhat616 

-----------------



**Full Changelog**: https://github.com/LibNyanpasu/nyanpasu-service/compare/v1.0.2...v1.0.3



## [1.0.4] - 2024-07-28

### 🐛 Bug Fixes

- Should start service after updated by @greenhat616 


### 🔨 Refactor

- Use atomic cell to hold flag and state, and add a recover core logic by @greenhat616 


### 🧹 Miscellaneous Tasks

- Sync latest nyanpasu-utils by @greenhat616 

-----------------



**Full Changelog**: https://github.com/LibNyanpasu/nyanpasu-service/compare/v1.0.3...v1.0.4



## [1.0.5] - 2024-07-28

### 🐛 Bug Fixes

- Fetch status deadlock by @greenhat616 

- Up by @greenhat616 


### 🧹 Miscellaneous Tasks

- Add a error log for deadlock debug use by @greenhat616 

- Add a timeout seq for status by @greenhat616 

- Update rust crate clap to 4.5.11 by @renovate[bot]  in [#35](https://github.com/LibNyanpasu/nyanpasu-service/pull/35)

- Apply linting fixes with rustfmt by @github-actions[bot] 

- Mark start req as Cow by @greenhat616 

-----------------



**Full Changelog**: https://github.com/LibNyanpasu/nyanpasu-service/compare/v1.0.4...v1.0.5



