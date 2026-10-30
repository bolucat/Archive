#![cfg(feature = "redb-store")]
mod support;

use nyanpasu_jobs::*;
use std::{
    error::Error as _,
    sync::{Arc, atomic::Ordering},
};

#[test]
fn garbage_database_preserves_open_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.redb");
    std::fs::write(&path, b"not a redb database").unwrap();
    let error = RedbJobStore::open(&path)
        .err()
        .expect("garbage database must fail");
    assert!(matches!(&error, Error::JournalOpen(_)));
    assert!(error.source().unwrap().is::<redb::DatabaseError>());
    assert_eq!(std::fs::read(path).unwrap(), b"not a redb database");
}

#[test]
fn incompatible_table_preserves_table_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.redb");
    let db = redb::Database::create(&path).unwrap();
    let tx = db.begin_write().unwrap();
    tx.open_table(redb::TableDefinition::<u64, u64>::new("jobs_runs_v1"))
        .unwrap();
    tx.commit().unwrap();
    drop(db);
    let error = RedbJobStore::open(path)
        .err()
        .expect("wrong table type must fail");
    assert!(matches!(
        &error,
        Error::JournalTable {
            source: redb::TableError::TableTypeMismatch { .. },
            ..
        }
    ));
    assert!(error.source().unwrap().is::<redb::TableError>());
}

async fn corrupt_record_start_error(skip_recovery: bool) -> Error {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.redb");
    drop(RedbJobStore::open(&path).unwrap());
    let db = redb::Database::create(&path).unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut runs = tx
            .open_table(redb::TableDefinition::<&str, &[u8]>::new("jobs_runs_v1"))
            .unwrap();
        runs.insert("corrupt-record", b"{broken JSON".as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    let store = RedbJobStore::open(&path).unwrap();
    let store: Box<dyn JobStore> = if skip_recovery {
        // Recovery would reject the same record first. Bypass only that stage
        // to exercise the real redb pruning decoder and startup propagation.
        let faults = Arc::new(support::Faults::default());
        faults.skip_recovery.store(true, Ordering::SeqCst);
        Box::new(support::Store {
            inner: store,
            faults,
        })
    } else {
        Box::new(store)
    };
    let error = match JobsService::start(
        store,
        LogCapture::new(LogPolicy::default()),
        Limits::default(),
    )
    .await
    {
        Err(error) => error,
        Ok(mut service) => {
            service.shutdown().await.unwrap();
            panic!("corrupt stored record must fail startup");
        }
    };
    // Failed startup joined the journal worker and released the database lock.
    drop(RedbJobStore::open(&path).unwrap());
    error
}

#[tokio::test]
async fn startup_recovery_preserves_record_decode_source() {
    let error = corrupt_record_start_error(false).await;
    assert!(matches!(&error, Error::JournalRecoveryDecode(source) if source.is_syntax()));
    assert!(error.source().unwrap().is::<serde_json::Error>());
}

#[tokio::test]
async fn startup_pruning_preserves_record_decode_source() {
    let error = corrupt_record_start_error(true).await;
    assert!(matches!(&error, Error::JournalPruneDecode(source) if source.is_syntax()));
    assert!(error.source().unwrap().is::<serde_json::Error>());
}
