# nyanpasu-jobs

Standalone jobs with typed Rust definitions, actor-owned execution, durable Run history, Tokio scheduling, and a scoped tracing journal. No Tauri, Profiles, core process, daemon IPC, or frontend dependency.

This implements the reusable portions of P1 → P2 → P3 → P5 from the [P0 contract](https://github.com/libnyanpasu/clash-nyanpasu/blob/a4be9966cf765e430e214133e5bc7544fba4deb0/docs/design/jobs-p0-contract.md). Application wiring remains a separate change. See the [audit guide](../../docs/design/nyanpasu-jobs.md) for contract mapping and validation boundaries.

## Usage

```rust,no_run
use nyanpasu_jobs::*;

# async fn example(store: Box<dyn JobStore>) -> Result<(), Error> {
let capture = LogCapture::new(LogPolicy::default());
// Install capture.layer() in the application's subscriber before starting Jobs.
let mut service = JobsService::start(store, capture, Limits::default()).await?;
let client = service.client();
let job = TypedJob::new(JobDefinition::manual("example/double"), 2u32,
    |_context, value| async move { Ok(value * 2) })?;
client.reconcile("default", 1, vec![job.registration()]).await?;
let handle = job.bind(client.clone());
let run = handle.run_with(&21).await?;
assert_eq!(run.output().await?, 42);
assert_eq!(client.get_run(run.id()).await?.completion().unwrap().outcome,
    Outcome::Succeeded);
let report = service.shutdown().await?;
// If !report.closed, keep the service and its business dependencies alive.
# Ok(())
# }
```

`Job::new` provides an erased registration when a typed handle is unnecessary. `Job::blocking` adapts synchronous work under the same concurrency permit. `()` is a normal no-input job. `TypedJob` retains the input/output types through registration and binding; schema mismatches in retained output report `OutputUnavailable`.

The `storage` module exposes the synchronous `JobStore` port and, with the default `redb-store` feature, `RedbJobStore`. At composition time, pass `RedbJobStore::open` the full file path you want, including its directory and filename; missing parent directories are created. The store owns that file exclusively; do not open another database instance for queries. `JobsClient` exposes history and logs through the owning worker. `--no-default-features` allows an injected `JobStore` without redb. A future Turso adapter can implement this port, though an async database API would need an adapter or a revised port. The optional `specta` feature derives types for finite `dto` records; it does not expose arbitrary JSON schemas or application commands.

Run the self-contained example with:

```sh
cargo run -p nyanpasu-jobs --example manual
```

## Ownership and completion

`JobsService` is the lifetime owner. Its typed `JobsClient` hides ractor internals. `JobsActor` owns registrations, revision/generation fences, admission permits, Run status, timer ownership, and execution JoinHandles. A bounded, single blocking storage worker owns the `JobStore`; it performs no business orchestration. A Run owner performs admission, business execution, child/delegation drain, log sealing, durable finalization, and maintenance in that order.

Keep the service until `shutdown()` returns `closed: true`. A timeout returns remaining Run IDs and the journal state; it does not abort blocking work or drop dependencies. Repeated shutdown calls continue joining the same work. Client clones may outlive shutdown but cannot keep the database open. Dropping the service without shutdown is not a graceful shutdown protocol; forced process exit is recovered as Interrupted on restart.

Run IDs are caller-generated UUIDs. `submit` accepts one explicitly so an adapter can retain the identity when its response is lost. `AdmissionUnknown(id)` means query that ID; **do not retry with a new ID**. Duplicate retained IDs return `AlreadySubmitted`. A known `AdmissionFailed` has no handler effects; ambiguous storage errors are resolved by reading the same ID, without another admission transaction. Retention bounds deduplication; never reuse an expired ID.

Only a confirmed admission starts business work. Business outcome, output encoding, and journal durability are separate. A failed or timed-out final write reports the real outcome with `JournalState::Degraded`, closes new admission, and retains the result while the owner retries storage. Retries never call the handler. Backoff is 1/2/4/8/16/30 seconds, with at most one in-flight write per owner; the database worker is globally serial. A blocked original write is awaited, not duplicated. Once all faulted owners finish durable cleanup, admission reopens.

`wait` is bounded (30 seconds by default), supports multiple and late waiters, and never cancels execution. Durable completion means accepted logs and the result are queryable. Active results use watch receivers; completed lookups use the journal, without a separate stale history cache. Retention may eventually make an ID unavailable.

Cancellation defaults to `Unsupported`. A cooperative handler waits on `context.cancelled()` and returns `JobError::cancelled()` only after effects stop. A cancellation request never overrides an actual successful return. The stable JobKey permit survives definition removal/recreation. Blocking work is not aborted.

Use `context.spawn(...)` for owned child futures. Transfer `context.delegation()` tokens to external actors owning delegated effects, retaining the token until those effects stop, including on lost replies. Parent completion closes new child creation and waits for existing children/tokens. A panicked parent with outstanding delegated work remains nonterminal with `execution_uncertain` in inspection. The handler remains responsible for interpreting child return values; task tracking establishes lifetime ownership, not automatic business error aggregation. Arbitrary detached `tokio::spawn` calls cannot be tracked by the crate.

Job handlers must return safe error codes/messages and output suitable for persistence. Raw inputs are not journaled. Output serialization is bounded and panic-contained; encoding failure does not rewrite Succeeded as Failed. Domain effects that outlive an RPC must retain an ownership token or be explicitly handed to another lifecycle owner before reporting completion.

## Scheduling and scopes

A full scope snapshot is validated before any changes. Old revisions are rejected; a repeated revision cannot change contents. Identical definitions/default inputs preserve the timer anchor. Increment `JobDefinition.version` when changing handler behavior even if metadata remains the same: closures are not compared or persisted. One scope cannot overwrite or remove another scope's keys. Inspection exposes source/applied revisions and validation errors.

Manual, Once, Interval, Cron, and explicit StartupCatchUp submissions share admission. There is no manual queue or automatic business retry. Manual Busy returns an error; a scheduled collision writes Skipped if journal/global capacity permits, otherwise increments `dropped_trigger_count`. At most eight admissions/runs are in flight by default, including skipped records.

A single owned timer wakes the actor for the nearest deadline and waits for acknowledgement. Reconcile wakes it to recompute from current registrations, so no queued message carries an obsolete definition. The actor serializes due selection and admission. This provides generation isolation without one timer task per registration.

Intervals use monotonic time, first wait a full period, and retain their anchor across manual executions. Once is a delay from registration; an extra manual execution does not consume it. A Due more than five seconds late is skipped and counted without generating an unbounded series of historical records. Once is consumed when missed. `missed_trigger_count` counts aggregated missed wakeups, not every elapsed theoretical tick.

Cron uses jiff-cron 0.3.0 and jiff 0.2, with five or six fields, an explicit IANA timezone, no year field, and exclusive future occurrences. Five-field expressions receive a zero-seconds prefix; six-field expressions and supported `@hourly`/`@daily`/`@weekly`/`@monthly`/`@yearly` aliases pass directly to jiff-cron. The library searches calendar years 1970–2100. The library skips a nonexistent fixed local time in a spring gap and runs a fixed time once across a fall overlap; an hourly wildcard visits both repeated hours. Its day-of-month and day-of-week fields both have to match when constrained, and numeric weekdays are Sunday = 1 through Saturday = 7 (0 is invalid). Existing schedules should be reviewed for those calendar semantics. Wall time is rechecked at least every five seconds; clock rollback does not replay an already consumed UTC slot. There is no restart replay; application-specific catch-up must be submitted explicitly. `Clock` is injectable; interval deadlines still use Tokio's monotonic clock.

## Logging and limits

Create `LogCapture` early and install its layer explicitly. File/console filters should be per-layer; an outer global EnvFilter can discard spans/events before this layer sees them. The service captures the current tracing dispatcher at startup and propagates it through managed futures/blocking work. Across actor messages, carry `JobContext` and instrument the actual processing future.

The default policy denies every target. Configure exact target/field allowlists. Arbitrary messages, URLs, tokens, and config bodies are not automatically safe: allow `message` only for audited safe text. The layer does not log its own storage failures. It performs bounded capture only; the owner drains batches and seals the Run before finalization. Late events cannot mutate sealed history. Dropped events consume sequence numbers and increment the final dropped count.

Defaults:

| Boundary                        | Limit                                                            |
| ------------------------------- | ---------------------------------------------------------------- |
| In-flight admissions/runs       | 8; one executing handler per stable JobKey                       |
| Raw input / encoded output      | 64 KiB each                                                      |
| Global ingress queue / entry    | 1024 entries / 4 KiB                                             |
| Accepted logs per Run           | 4096 entries or 1 MiB                                            |
| Log batch                       | 128 entries, drained every 100 ms; final tail sealed immediately |
| Control RPC / finalization wait | 10 seconds each                                                  |
| Business wait / shutdown wait   | 30 seconds each                                                  |
| Query page                      | 1–200 entries; log response additionally bounded to 256 KiB      |
| Terminal retention              | 7 days, 100 per Job, 10000 total, 256 MiB logical records/logs   |

Retention keeps newest admission sequences, touches only confirmed terminal records, and deletes records/indexes/logs atomically. It runs at startup, after completion, and every minute while idle; expired records may remain visible until that maintenance pass. Limits govern logical data, not redb's physical file size. Pagination orders by durable admission sequence, so wall-clock rollback does not hide newly admitted runs. Cursor query limits apply to each page; callers own their presentation/authorization policies.

## Validation

```sh
cargo test -p nyanpasu-jobs --all-features --locked
cargo test -p nyanpasu-jobs --no-default-features --locked
cargo clippy -p nyanpasu-jobs --all-targets --all-features --locked -- -D warnings
cargo fmt -p nyanpasu-jobs --check
```

Tests use injected stores/clocks, temporary databases, explicit task acknowledgements, and virtual time. The CI matrix runs both feature configurations on Linux, macOS, and Windows. Injected transaction failures and reopen recovery are not physical ENOSPC, power-loss or OS-suspend simulations; application workflows and frontend integration remain outside this crate delivery.
