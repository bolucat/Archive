//! processkit adapter. This is the only file in `src/process` that uses processkit.

use std::{collections::VecDeque, future::pending, sync::Arc, time::Duration};

use processkit::prelude::StreamExt;
use tokio::sync::{mpsc, oneshot, watch};

use super::{
    command::{Command, PidFile},
    error::ProcessError,
    event::{ProcessEvent, TerminatedPayload},
    handle::{Containment, Ctrl},
    pid_file::PidFileGuard,
};

pub(crate) struct SpawnParts {
    pub pid: u32,
    pub containment: Containment,
    pub ctrl_tx: mpsc::Sender<Ctrl>,
    pub terminated_rx: watch::Receiver<Option<Result<TerminatedPayload, String>>>,
    pub events_rx: mpsc::Receiver<ProcessEvent>,
}

struct PumpParts {
    run: processkit::RunningProcess,
    events: processkit::OutputEvents,
    group: Arc<processkit::ProcessGroup>,
    stdin_tx: Option<mpsc::Sender<StdinWrite>>,
    kill_grace: Duration,
    timeout_at: Option<tokio::time::Instant>,
    ev_tx: mpsc::Sender<ProcessEvent>,
    ctrl_rx: mpsc::Receiver<Ctrl>,
    term_tx: watch::Sender<Option<Result<TerminatedPayload, String>>>,
    pid_guard: Option<PidFileGuard>,
}

fn build_pk(cmd: &Command, include_timeout: bool) -> processkit::Command {
    let mut pk = processkit::Command::new(&cmd.program).args(&cmd.args);
    for (key, value) in &cmd.envs {
        pk = pk.env(key, value);
    }
    if let Some(dir) = &cmd.current_dir {
        pk = pk.current_dir(dir);
    }
    if let Some(encoding) = cmd.encoding {
        pk = pk.encoding(encoding);
    }
    if include_timeout && let Some(timeout) = cmd.timeout {
        pk = pk.timeout(timeout);
    }
    #[cfg(windows)]
    if cmd.hide_window {
        pk = pk.create_no_window();
    }
    pk = pk.output_buffer(processkit::OutputBufferPolicy::unbounded().with_max_bytes(256 * 1024));
    if cmd.pipe_stdin {
        pk = pk.keep_stdin_open();
    }
    pk
}

pub(crate) async fn run_capture(cmd: Command) -> Result<super::error::ProcessOutput, ProcessError> {
    let program = cmd.program.to_string_lossy().into_owned();
    let timeout = cmd.timeout;
    let result = build_pk(&cmd, true)
        .output_string()
        .await
        .map_err(|error| match error {
            processkit::Error::Spawn { .. } | processkit::Error::NotFound { .. } => {
                ProcessError::Spawn {
                    program,
                    message: error.to_string(),
                }
            }
            error => ProcessError::Engine(error.to_string()),
        })?;

    if result.timed_out() {
        return Err(ProcessError::Timeout {
            after: timeout.unwrap_or_default(),
        });
    }

    Ok(super::error::ProcessOutput {
        code: result.code(),
        stdout: result.stdout().clone(),
        stderr: result.stderr().to_owned(),
    })
}

fn map_containment(mechanism: processkit::Mechanism) -> Containment {
    match mechanism {
        processkit::Mechanism::JobObject => Containment::JobObject,
        processkit::Mechanism::CgroupV2 => Containment::CgroupV2,
        processkit::Mechanism::ProcessGroup => Containment::ProcessGroup,
        _ => Containment::ProcessGroup,
    }
}

fn map_outcome(outcome: &processkit::Outcome) -> TerminatedPayload {
    TerminatedPayload {
        code: outcome.code(),
        signal: outcome.signal(),
    }
}

struct FinishOutput {
    payload: TerminatedPayload,
    stderr_tail: Vec<String>,
    error: Option<String>,
}

/// Reconstruct the tail stderr lines that `finish()` drained from the shared
/// sink. processkit builds `Finished.stderr` as `lines.join("\n")` and its line
/// reader strips terminators, so `split('\n')` is the exact inverse for any
/// non-empty capture.
///
/// Known limitation: `Finished.stderr` is a plain `String`, so an empty capture
/// (`[]`) and a single empty line (`[""]`) both serialize to `""` and are
/// indistinguishable. We treat `""` as "no lines", so a lone blank stderr line
/// that `finish()` wins the drain for is dropped. Disambiguating would require a
/// processkit API change (out of scope).
fn split_stderr_tail(stderr: &str) -> Vec<String> {
    if stderr.is_empty() {
        Vec::new()
    } else {
        stderr.split('\n').map(str::to_owned).collect()
    }
}

fn normalize_finish(result: processkit::Result<processkit::Finished>) -> FinishOutput {
    match result {
        Ok(finished) => FinishOutput {
            payload: map_outcome(&finished.outcome),
            stderr_tail: split_stderr_tail(&finished.stderr),
            error: None,
        },
        Err(error) => FinishOutput {
            payload: TerminatedPayload {
                code: None,
                signal: None,
            },
            stderr_tail: Vec::new(),
            error: Some(error.to_string()),
        },
    }
}

async fn hard_kill_deadline(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => pending::<()>().await,
    }
}

type StdinWrite = (Vec<u8>, oneshot::Sender<Result<(), ProcessError>>);

const DYING_EVENT_STALL: Duration = Duration::from_secs(5);

async fn write_stdin(
    mut stdin: processkit::ProcessStdin,
    mut requests: mpsc::Receiver<StdinWrite>,
) {
    while let Some((data, reply)) = requests.recv().await {
        if stdin.write(&data).await.is_err() || stdin.flush().await.is_err() {
            let _ = reply.send(Err(ProcessError::StdinUnavailable));
            drop(stdin);
            requests.close();
            while let Some((_, reply)) = requests.recv().await {
                let _ = reply.send(Err(ProcessError::StdinUnavailable));
            }
            return;
        }
        let _ = reply.send(Ok(()));
    }
}

fn handle_ctrl(
    ctrl: Ctrl,
    group: &processkit::ProcessGroup,
    stdin_tx: &Option<mpsc::Sender<StdinWrite>>,
    kill_grace: Duration,
    hard_kill_at: &mut Option<tokio::time::Instant>,
) -> bool {
    match ctrl {
        Ctrl::Kill(reply) => {
            *hard_kill_at = None;
            let result = group
                .kill_all()
                .map_err(|error| ProcessError::Engine(error.to_string()));
            let _ = reply.send(result);
            true
        }
        Ctrl::GracefulKill(reply) => {
            #[cfg(unix)]
            let result = match group.signal(processkit::Signal::Term) {
                Ok(()) => {
                    let grace_deadline = tokio::time::Instant::now() + kill_grace;
                    if hard_kill_at.is_none_or(|deadline| grace_deadline < deadline) {
                        *hard_kill_at = Some(grace_deadline);
                    }
                    Ok(())
                }
                Err(_) => group
                    .kill_all()
                    .map_err(|error| ProcessError::Engine(error.to_string())),
            };

            #[cfg(windows)]
            let result = {
                let _ = kill_grace;
                *hard_kill_at = None;
                group
                    .kill_all()
                    .map_err(|error| ProcessError::Engine(error.to_string()))
            };

            let _ = reply.send(result);
            true
        }
        Ctrl::WriteStdin(data, reply) => {
            if let Some(stdin_tx) = stdin_tx {
                if let Err(error) = stdin_tx.try_send((data, reply)) {
                    let (_, reply) = error.into_inner();
                    let _ = reply.send(Err(ProcessError::StdinUnavailable));
                }
            } else {
                let _ = reply.send(Err(ProcessError::StdinUnavailable));
            }
            false
        }
    }
}

pub(crate) async fn spawn(cmd: Command) -> Result<SpawnParts, ProcessError> {
    let program = cmd.program.to_string_lossy().into_owned();
    let capacity = cmd.event_channel_capacity;
    let kill_grace = cmd.kill_grace;
    let pipe_stdin = cmd.pipe_stdin;
    let timeout = cmd.timeout;
    let epoch_pid_required = matches!(&cmd.pid_file, Some(PidFile::Epoch(_)));
    let expected_exe = std::path::Path::new(&cmd.program)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned);
    let pid_guard = match &cmd.pid_file {
        Some(PidFile::Legacy(path)) => Some(PidFileGuard::prepare_legacy(path.clone()).await?),
        Some(PidFile::Epoch(spec)) => Some(
            PidFileGuard::prepare_epoch(
                spec.clone(),
                expected_exe.ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "epoch pid records require a UTF-8 executable basename",
                    )
                })?,
            )
            .await?,
        ),
        None => None,
    };
    // A shared-group RunningProcess only times out its direct child. The pump
    // owns the shared group deadline so descendants holding inherited pipes are
    // killed as well.
    let pk = build_pk(&cmd, false);

    let spawn_error = |error: processkit::Error| ProcessError::Spawn {
        program: program.clone(),
        message: error.to_string(),
    };
    let group = Arc::new(processkit::ProcessGroup::new().map_err(&spawn_error)?);
    let containment = map_containment(group.mechanism());
    let mut run = group.start(&pk).await.map_err(spawn_error)?;
    let timeout_at = timeout.map(|timeout| tokio::time::Instant::now() + timeout);
    let pid = run
        .pid()
        .ok_or_else(|| ProcessError::Engine("spawned process has no pid".into()))?;
    if let Some(g) = &pid_guard
        && let Err(e) = g.write(pid).await
    {
        if epoch_pid_required {
            let _ = group.kill_all();
            let _ = run.finish().await;
            return Err(ProcessError::Io(e));
        }
        tracing::warn!("failed to write pid file: {e}");
    }
    let stdin_tx = if pipe_stdin {
        run.take_stdin().map(|stdin| {
            let (stdin_tx, stdin_rx) = mpsc::channel(64);
            tokio::spawn(write_stdin(stdin, stdin_rx));
            stdin_tx
        })
    } else {
        None
    };
    let events = run
        .output_events()
        .map_err(|error| ProcessError::Engine(error.to_string()))?;

    let (ev_tx, ev_rx) = mpsc::channel(capacity);
    let (ctrl_tx, ctrl_rx) = mpsc::channel(8);
    let (term_tx, term_rx) = watch::channel(None);

    tokio::spawn(pump(PumpParts {
        run,
        events,
        group,
        stdin_tx,
        kill_grace,
        timeout_at,
        ev_tx,
        ctrl_rx,
        term_tx,
        pid_guard,
    }));

    Ok(SpawnParts {
        pid,
        containment,
        ctrl_tx,
        terminated_rx: term_rx,
        events_rx: ev_rx,
    })
}

async fn pump(parts: PumpParts) {
    let PumpParts {
        run,
        mut events,
        group,
        stdin_tx,
        kill_grace,
        timeout_at,
        ev_tx,
        mut ctrl_rx,
        term_tx,
        pid_guard,
    } = parts;
    let mut finish = Box::pin(run.finish());
    let mut finish_output: Option<FinishOutput> = None;
    let mut pending_event = None;
    let mut hard_kill_at = timeout_at;
    let mut events_open = true;
    let mut ctrl_open = true;
    let mut dying = false;
    let mut drop_output = false;
    let mut drop_pending_at = None;
    let mut dropped_output_events = 0usize;
    let mut abandoned = false;

    loop {
        if !events_open && !ctrl_open && !abandoned {
            abandoned = true;
            dying = true;
            hard_kill_at = None;
            let _ = group.kill_all();
        }

        tokio::select! {
            biased;
            ctrl = ctrl_rx.recv(), if ctrl_open => match ctrl {
                Some(ctrl) => {
                    if handle_ctrl(
                        ctrl,
                        &group,
                        &stdin_tx,
                        kill_grace,
                        &mut hard_kill_at,
                    ) {
                        dying = true;
                        if pending_event.is_some() && drop_pending_at.is_none() {
                            drop_pending_at = Some(tokio::time::Instant::now() + DYING_EVENT_STALL);
                        }
                    }
                }
                None => ctrl_open = false,
            },
            _ = hard_kill_deadline(hard_kill_at) => {
                hard_kill_at = None;
                dying = true;
                let _ = group.kill_all();
                if pending_event.is_some() && drop_pending_at.is_none() {
                    drop_pending_at = Some(tokio::time::Instant::now() + DYING_EVENT_STALL);
                }
            }
            result = &mut finish, if finish_output.is_none() => {
                let out = normalize_finish(result);
                dying = true;
                if pending_event.is_some() && drop_pending_at.is_none() {
                    drop_pending_at = Some(tokio::time::Instant::now() + DYING_EVENT_STALL);
                }
                let watch = match &out.error {
                    Some(error) => Err(error.clone()),
                    None => Ok(out.payload.clone()),
                };
                let _ = term_tx.send(Some(watch));
                finish_output = Some(out);
            }
            _ = hard_kill_deadline(drop_pending_at), if dying && !drop_output && pending_event.is_some() => {
                drop_pending_at = None;
                if pending_event.take().is_some() {
                    dropped_output_events += 1;
                }
                drop_output = true;
            }
            permit = ev_tx.reserve(), if events_open && pending_event.is_some() && !drop_output => match permit {
                Ok(permit) => {
                    permit.send(pending_event.take().expect("pending event"));
                    drop_pending_at = None;
                }
                Err(_) => {
                    events_open = false;
                    pending_event = None;
                    drop_pending_at = None;
                }
            },
            _ = ev_tx.closed(), if events_open => {
                events_open = false;
                pending_event = None;
                drop_pending_at = None;
            }
            event = events.next(), if pending_event.is_none() => match event {
                Some(processkit::OutputEvent::Stdout(line)) => {
                    if drop_output {
                        dropped_output_events += 1;
                    } else if events_open {
                        pending_event = Some(ProcessEvent::Stdout(line.into_text()));
                        if dying {
                            drop_pending_at = Some(tokio::time::Instant::now() + DYING_EVENT_STALL);
                        }
                    }
                }
                Some(processkit::OutputEvent::Stderr(line)) => {
                    if drop_output {
                        dropped_output_events += 1;
                    } else if events_open {
                        pending_event = Some(ProcessEvent::Stderr(line.into_text()));
                        if dying {
                            drop_pending_at = Some(tokio::time::Instant::now() + DYING_EVENT_STALL);
                        }
                    }
                }
                Some(_) => {}
                None => break,
            },
        }
    }

    drop(events);
    let finish_output = match finish_output {
        Some(out) => out,
        None => {
            let out = loop {
                if !events_open && !ctrl_open && !abandoned {
                    abandoned = true;
                    hard_kill_at = None;
                    let _ = group.kill_all();
                }

                tokio::select! {
                    biased;
                    ctrl = ctrl_rx.recv(), if ctrl_open => match ctrl {
                        Some(ctrl) => {
                            let _ = handle_ctrl(
                                ctrl,
                                &group,
                                &stdin_tx,
                                kill_grace,
                                &mut hard_kill_at,
                            );
                        }
                        None => ctrl_open = false,
                    },
                    _ = hard_kill_deadline(hard_kill_at) => {
                        hard_kill_at = None;
                        let _ = group.kill_all();
                    }
                    _ = ev_tx.closed(), if events_open => events_open = false,
                    result = &mut finish => break normalize_finish(result),
                }
            };
            let watch = match &out.error {
                Some(error) => Err(error.clone()),
                None => Ok(out.payload.clone()),
            };
            let _ = term_tx.send(Some(watch));
            out
        }
    };
    cleanup_pid_file(&pid_guard).await;

    let mut terminal_events = VecDeque::new();
    for line in finish_output.stderr_tail {
        terminal_events.push_back(ProcessEvent::Stderr(line));
    }
    if dropped_output_events > 0 {
        terminal_events.push_back(ProcessEvent::Error(format!(
            "dropped {dropped_output_events} buffered output events: receiver not draining"
        )));
    }
    if let Some(error) = finish_output.error {
        terminal_events.push_back(ProcessEvent::Error(error));
    }
    terminal_events.push_back(ProcessEvent::Terminated(finish_output.payload));

    // wait() has already returned and the pid file is already cleaned, so the
    // terminal events are the only thing left to deliver. A receiver that keeps
    // the channel open but never drains would block `reserve()` forever and leak
    // this task; bound the flush so it gives up after a stall with no progress
    // (the deadline resets on every successful send).
    let mut flush_deadline = Some(tokio::time::Instant::now() + DYING_EVENT_STALL);
    while events_open && !terminal_events.is_empty() {
        tokio::select! {
            biased;
            ctrl = ctrl_rx.recv(), if ctrl_open => match ctrl {
                Some(ctrl) => {
                    let _ = handle_ctrl(
                        ctrl,
                        &group,
                        &stdin_tx,
                        kill_grace,
                        &mut hard_kill_at,
                    );
                }
                None => ctrl_open = false,
            },
            _ = hard_kill_deadline(hard_kill_at) => {
                hard_kill_at = None;
                let _ = group.kill_all();
            }
            _ = ev_tx.closed() => events_open = false,
            permit = ev_tx.reserve() => match permit {
                Ok(permit) => {
                    permit.send(terminal_events.pop_front().expect("terminal event"));
                    flush_deadline = Some(tokio::time::Instant::now() + DYING_EVENT_STALL);
                }
                Err(_) => events_open = false,
            },
            _ = hard_kill_deadline(flush_deadline) => events_open = false,
        }
    }
}

async fn cleanup_pid_file(pid_guard: &Option<PidFileGuard>) {
    if let Some(guard) = pid_guard {
        guard.cleanup().await;
    }
}

#[cfg(test)]
mod tests {
    use super::split_stderr_tail;

    #[test]
    fn split_stderr_tail_reconstructs_joined_lines() {
        // Empty capture -> no lines (see split_stderr_tail's known limitation).
        assert!(split_stderr_tail("").is_empty());
        assert_eq!(split_stderr_tail("only"), vec!["only".to_owned()]);
        assert_eq!(
            split_stderr_tail("first\nsecond"),
            vec!["first".to_owned(), "second".to_owned()]
        );
        // Trailing empty line survives: join("\n") of ["msg", ""] is "msg\n".
        assert_eq!(
            split_stderr_tail("msg\n"),
            vec!["msg".to_owned(), String::new()]
        );
    }
}
