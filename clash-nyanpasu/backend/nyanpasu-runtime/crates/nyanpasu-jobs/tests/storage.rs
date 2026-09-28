#![cfg(feature = "redb-store")]
use jiff::Timestamp;
use nyanpasu_jobs::*;
use std::time::Duration;
fn record(job: &str) -> RunRecord {
    RunRecord {
        id: RunId::new_v4(),
        job: job.into(),
        definition_version: 1,
        trigger: Trigger::Manual,
        scheduled_at: None,
        admitted_at: Timestamp::now(),
        admission_sequence: 0,
        finished_at: None,
        state: RunState::Admitted,
        last_log_sequence: 0,
        dropped_log_count: 0,
    }
}
fn finish(record: &mut RunRecord) {
    record.finished_at = Some(Timestamp::now());
    record.state = RunState::Finished(Completion {
        outcome: Outcome::Succeeded,
        output: Output::Json("42".into()),
        journal: JournalState::Durable,
    });
}
#[test]
fn reopen_recovers_unfinished_runs_without_replaying_and_keeps_committed_logs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.redb");
    let mut store = RedbJobStore::open(&path).unwrap();
    assert!(RedbJobStore::open(&path).is_err());
    let running = store.admit(record("job")).unwrap();
    let mut done = store.admit(record("job")).unwrap();
    finish(&mut done);
    store.finish(&done, &[]).unwrap();
    let log = RunLog {
        run_id: running.id,
        sequence: 8,
        time: Timestamp::now(),
        level: "INFO".into(),
        target: "safe".into(),
        fields: Default::default(),
    };
    store.append(running.id, &[log]).unwrap();
    drop(store);
    let mut store = RedbJobStore::open(&path).unwrap();
    store.recover(Timestamp::now()).unwrap();
    let recovered = store.get(running.id).unwrap().unwrap();
    assert_eq!(
        recovered.completion().unwrap().outcome,
        Outcome::Interrupted
    );
    assert_eq!(recovered.last_log_sequence, 8);
    assert_eq!(store.logs(running.id, 0, 10).unwrap().items.len(), 1);
    assert_eq!(
        store
            .get(done.id)
            .unwrap()
            .unwrap()
            .completion()
            .unwrap()
            .outcome,
        Outcome::Succeeded
    );
    assert!(matches!(
        store.admit(running),
        Err(Error::AlreadySubmitted(_))
    ));
}
#[test]
fn transaction_rejects_wrong_run_log_without_partial_terminal_record() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = RedbJobStore::open(dir.path().join("jobs.redb")).unwrap();
    let mut run = store.admit(record("job")).unwrap();
    finish(&mut run);
    let log = RunLog {
        run_id: RunId::new_v4(),
        sequence: 1,
        time: Timestamp::now(),
        level: "INFO".into(),
        target: "safe".into(),
        fields: Default::default(),
    };
    assert!(store.finish(&run, &[log]).is_err());
    assert!(store.get(run.id).unwrap().unwrap().completion().is_none());
    assert!(store.logs(run.id, 0, 10).unwrap().items.is_empty());
    store.finish(&run, &[]).unwrap();
    store.finish(&run, &[]).unwrap();
    let mut changed = run.clone();
    changed.state = RunState::Finished(Completion {
        outcome: Outcome::Cancelled,
        output: Output::None,
        journal: JournalState::Durable,
    });
    assert!(store.finish(&changed, &[]).is_err());
    assert_eq!(
        store
            .get(run.id)
            .unwrap()
            .unwrap()
            .completion()
            .unwrap()
            .outcome,
        Outcome::Succeeded
    );
}
#[test]
fn sequence_cursor_survives_clock_rollback_and_retention_removes_indexes_and_logs() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = RedbJobStore::open(dir.path().join("jobs.redb")).unwrap();
    let active = store.admit(record("job")).unwrap();
    let mut first = store.admit(record("job")).unwrap();
    finish(&mut first);
    store
        .finish(
            &first,
            &[RunLog {
                run_id: first.id,
                sequence: 1,
                time: Timestamp::now(),
                level: "INFO".into(),
                target: "safe".into(),
                fields: Default::default(),
            }],
        )
        .unwrap();
    let page = store.runs("job", None, 1).unwrap();
    assert_eq!(page.items[0].id, active.id);
    let mut later = record("job");
    later.admitted_at = "2000-01-01T00:00:00Z".parse().unwrap();
    let mut later = store.admit(later).unwrap();
    finish(&mut later);
    store.finish(&later, &[]).unwrap();
    let page = store.runs("job", page.next, 10).unwrap();
    assert_eq!(
        page.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![first.id, later.id]
    );
    store
        .prune(
            Timestamp::now(),
            &Retention {
                per_job: 1,
                ..Retention::default()
            },
        )
        .unwrap();
    assert!(store.get(first.id).unwrap().is_none());
    assert!(matches!(store.logs(first.id, 0, 1), Err(Error::NotFound)));
    let page = store.runs("job", None, 50).unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].id, active.id);
    store
        .prune(
            Timestamp::now()
                .checked_add(Duration::from_secs(1))
                .unwrap(),
            &Retention {
                age: Duration::ZERO,
                ..Retention::default()
            },
        )
        .unwrap();
    assert_eq!(store.runs("job", None, 50).unwrap().items.len(), 1);
    assert!(
        store
            .get(active.id)
            .unwrap()
            .unwrap()
            .completion()
            .is_none()
    );
}

#[test]
fn custom_nested_storage_path_is_created_and_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir
        .path()
        .join("custom")
        .join("history")
        .join("runs.custom.redb");
    let mut store = RedbJobStore::open(&path).unwrap();
    let saved = store.admit(record("custom/path")).unwrap();
    assert!(path.is_file());
    drop(store);
    let mut store = RedbJobStore::open(&path).unwrap();
    assert_eq!(store.get(saved.id).unwrap().unwrap(), saved);
}

#[test]
fn storage_accepts_a_bare_relative_filename() {
    let path = std::path::PathBuf::from(format!("jobs-{}.redb", RunId::new_v4()));
    let mut store = RedbJobStore::open(&path).unwrap();
    let saved = store.admit(record("relative")).unwrap();
    drop(store);
    let mut store = RedbJobStore::open(&path).unwrap();
    assert_eq!(store.get(saved.id).unwrap().unwrap(), saved);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn storage_reports_parent_path_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("already-a-file");
    std::fs::write(&parent, b"occupied").unwrap();
    assert!(RedbJobStore::open(parent.join("jobs.redb")).is_err());
}
