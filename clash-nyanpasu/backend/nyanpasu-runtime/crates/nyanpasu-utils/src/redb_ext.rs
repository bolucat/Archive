//! Classify redb errors by what they say about the database file, so every
//! redb owner decides recovery from the same facts.
//!
//! Classification is not policy: whether a corrupt or incompatible file may be
//! preserved and rebuilt stays with the owner of that file.

use std::error::Error;

use crate::io::{IoFailure, classify};

/// What a redb error says about the database file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedbFailure {
    /// The file is not a usable redb database: corrupted pages, a wrong magic
    /// number, an empty file opened without creation, or a read past the end
    /// of a truncated file.
    Corrupt,
    /// The file is a redb database this build cannot use as it is: a file
    /// format that requires an upgrade, or a table whose key, value or
    /// multimap kind differs from its definition.
    Incompatible,
    /// A table named by the caller does not exist.
    MissingTable,
    /// Another handle, in this process or another one, holds the database.
    AlreadyOpen,
    /// Any other IO error of the database file.
    Io(IoFailure),
    /// Any other failure: misuse, a poisoned or closed transaction, a handle
    /// poisoned by an earlier IO error, a value that is too large, or an
    /// aborted repair, which a read-only open of a database that needs
    /// recovery also returns. It does not say whether the file is healthy.
    Internal,
}

/// Classifies a redb error, including the storage error nested in it.
pub trait RedbErrorExt {
    fn failure(&self) -> RedbFailure;
}

/// Finds the first redb error in `error` or its sources and classifies it.
///
/// Returns `None` when the chain holds no redb error. Downcasting only
/// recognizes errors of the redb version this crate is linked against, so
/// every caller must resolve to the same semver-compatible redb.
pub fn find_failure(error: &(dyn Error + 'static)) -> Option<RedbFailure> {
    let mut cause = Some(error);
    while let Some(error) = cause {
        let failure = downcast::<redb::Error>(error)
            .or_else(|| downcast::<redb::DatabaseError>(error))
            .or_else(|| downcast::<redb::StorageError>(error))
            .or_else(|| downcast::<redb::TableError>(error))
            .or_else(|| downcast::<redb::TransactionError>(error))
            .or_else(|| downcast::<redb::CommitError>(error))
            .or_else(|| downcast::<redb::SavepointError>(error))
            .or_else(|| downcast::<redb::CompactionError>(error))
            .or_else(|| downcast::<redb::SetDurabilityError>(error))
            .or_else(|| downcast::<redb::CursorError>(error))
            .or_else(|| downcast::<redb::BackendError>(error));
        if failure.is_some() {
            return failure;
        }
        cause = error.source();
    }
    None
}

fn downcast<E: Error + RedbErrorExt + 'static>(
    error: &(dyn Error + 'static),
) -> Option<RedbFailure> {
    error.downcast_ref::<E>().map(E::failure)
}

fn io_failure(error: &std::io::Error) -> RedbFailure {
    match error.kind() {
        // redb reports a wrong magic number and an empty file opened without
        // creation as InvalidData; a truncated file fails a read with
        // UnexpectedEof.
        std::io::ErrorKind::InvalidData | std::io::ErrorKind::UnexpectedEof => RedbFailure::Corrupt,
        _ => RedbFailure::Io(classify(error)),
    }
}

impl RedbErrorExt for redb::Error {
    fn failure(&self) -> RedbFailure {
        match self {
            redb::Error::Corrupted(_) => RedbFailure::Corrupt,
            redb::Error::Io(error) => io_failure(error),
            redb::Error::UpgradeRequired(_)
            | redb::Error::TableTypeMismatch { .. }
            | redb::Error::TableIsMultimap(_)
            | redb::Error::TableIsNotMultimap(_)
            | redb::Error::TypeDefinitionChanged { .. } => RedbFailure::Incompatible,
            redb::Error::TableDoesNotExist(_) => RedbFailure::MissingTable,
            redb::Error::DatabaseAlreadyOpen => RedbFailure::AlreadyOpen,
            _ => RedbFailure::Internal,
        }
    }
}

impl RedbErrorExt for redb::StorageError {
    fn failure(&self) -> RedbFailure {
        match self {
            redb::StorageError::Corrupted(_) => RedbFailure::Corrupt,
            redb::StorageError::Io(error) => io_failure(error),
            _ => RedbFailure::Internal,
        }
    }
}

impl RedbErrorExt for redb::DatabaseError {
    fn failure(&self) -> RedbFailure {
        match self {
            redb::DatabaseError::DatabaseAlreadyOpen => RedbFailure::AlreadyOpen,
            redb::DatabaseError::UpgradeRequired(_) => RedbFailure::Incompatible,
            redb::DatabaseError::Storage(error) => error.failure(),
            _ => RedbFailure::Internal,
        }
    }
}

impl RedbErrorExt for redb::TableError {
    fn failure(&self) -> RedbFailure {
        match self {
            redb::TableError::TableTypeMismatch { .. }
            | redb::TableError::TableIsMultimap(_)
            | redb::TableError::TableIsNotMultimap(_)
            | redb::TableError::TypeDefinitionChanged { .. } => RedbFailure::Incompatible,
            redb::TableError::TableDoesNotExist(_) => RedbFailure::MissingTable,
            redb::TableError::Storage(error) => error.failure(),
            _ => RedbFailure::Internal,
        }
    }
}

impl RedbErrorExt for redb::TransactionError {
    fn failure(&self) -> RedbFailure {
        match self {
            redb::TransactionError::Storage(error) => error.failure(),
            _ => RedbFailure::Internal,
        }
    }
}

impl RedbErrorExt for redb::CommitError {
    fn failure(&self) -> RedbFailure {
        match self {
            redb::CommitError::Storage(error) => error.failure(),
            _ => RedbFailure::Internal,
        }
    }
}

impl RedbErrorExt for redb::SavepointError {
    fn failure(&self) -> RedbFailure {
        match self {
            redb::SavepointError::Storage(error) => error.failure(),
            _ => RedbFailure::Internal,
        }
    }
}

impl RedbErrorExt for redb::CompactionError {
    fn failure(&self) -> RedbFailure {
        match self {
            redb::CompactionError::Storage(error) => error.failure(),
            _ => RedbFailure::Internal,
        }
    }
}

impl RedbErrorExt for redb::SetDurabilityError {
    fn failure(&self) -> RedbFailure {
        RedbFailure::Internal
    }
}

impl RedbErrorExt for redb::CursorError {
    fn failure(&self) -> RedbFailure {
        match self {
            redb::CursorError::Storage(error) => error.failure(),
            _ => RedbFailure::Internal,
        }
    }
}

impl RedbErrorExt for redb::BackendError {
    fn failure(&self) -> RedbFailure {
        match self {
            redb::BackendError::Io(error) => io_failure(error),
            _ => RedbFailure::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Error as IoError, ErrorKind};

    use redb::{
        BackendError, CommitError, CompactionError, CursorError, Database, DatabaseError,
        MultimapTableDefinition, ReadableDatabase, SavepointError, SetDurabilityError,
        StorageError, TableDefinition, TableError, TransactionError,
    };

    use super::{RedbErrorExt, RedbFailure, find_failure};
    use crate::io::IoFailure;

    const COUNTS: TableDefinition<&str, u64> = TableDefinition::new("counts");

    fn database_with_counts(path: &std::path::Path) -> Database {
        let db = Database::create(path).unwrap();
        let tx = db.begin_write().unwrap();
        tx.open_table(COUNTS).unwrap().insert("a", 1).unwrap();
        tx.commit().unwrap();
        db
    }

    #[test]
    fn truncated_files_are_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("whole.redb");
        drop(database_with_counts(&path));
        let len = std::fs::metadata(&path).unwrap().len();

        // Half the file fails redb's layout check; a stub shorter than the
        // header fails the first read.
        for cut in [len / 2, 100] {
            let truncated = dir.path().join(format!("cut-{cut}.redb"));
            std::fs::copy(&path, &truncated).unwrap();
            std::fs::OpenOptions::new()
                .write(true)
                .open(&truncated)
                .unwrap()
                .set_len(cut)
                .unwrap();

            let error = Database::open(&truncated).unwrap_err();

            assert_eq!(error.failure(), RedbFailure::Corrupt, "{cut}: {error}");
        }
    }

    #[test]
    fn foreign_and_empty_files_are_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let foreign = dir.path().join("foreign.redb");
        std::fs::write(&foreign, vec![b'x'; 4096]).unwrap();
        let empty = dir.path().join("empty.redb");
        std::fs::write(&empty, []).unwrap();

        for error in [
            Database::open(&foreign).unwrap_err(),
            Database::create(&foreign).unwrap_err(),
            Database::open(&empty).unwrap_err(),
        ] {
            assert_eq!(error.failure(), RedbFailure::Corrupt, "{error}");
        }
    }

    #[test]
    fn a_held_database_is_already_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("held.redb");
        let _held = Database::create(&path).unwrap();

        let error = Database::create(&path).unwrap_err();

        assert_eq!(error.failure(), RedbFailure::AlreadyOpen);
    }

    #[test]
    fn table_shape_mismatches_are_incompatible_and_absent_tables_missing() {
        let dir = tempfile::tempdir().unwrap();
        let db = database_with_counts(&dir.path().join("tables.redb"));
        let tx = db.begin_read().unwrap();

        let mismatch = tx
            .open_table(TableDefinition::<u64, u64>::new("counts"))
            .unwrap_err();
        let Err(multimap) =
            tx.open_multimap_table(MultimapTableDefinition::<&str, u64>::new("counts"))
        else {
            panic!("a plain table opened as a multimap table");
        };
        let missing = tx
            .open_table(TableDefinition::<&str, u64>::new("missing"))
            .unwrap_err();

        assert_eq!(mismatch.failure(), RedbFailure::Incompatible, "{mismatch}");
        assert_eq!(multimap.failure(), RedbFailure::Incompatible, "{multimap}");
        assert_eq!(missing.failure(), RedbFailure::MissingTable, "{missing}");
        assert_eq!(
            redb::Error::from(missing).failure(),
            RedbFailure::MissingTable
        );
    }

    #[test]
    fn nested_storage_errors_keep_their_classification() {
        let io = |kind| StorageError::Io(IoError::from(kind));

        assert_eq!(
            StorageError::Corrupted("bad page".into()).failure(),
            RedbFailure::Corrupt
        );
        assert_eq!(io(ErrorKind::UnexpectedEof).failure(), RedbFailure::Corrupt);
        assert_eq!(
            io(ErrorKind::PermissionDenied).failure(),
            RedbFailure::Io(IoFailure::PermissionDenied)
        );
        assert_eq!(
            io(ErrorKind::Interrupted).failure(),
            RedbFailure::Io(IoFailure::Transient)
        );
        assert_eq!(
            StorageError::ValueTooLarge(1).failure(),
            RedbFailure::Internal
        );

        assert_eq!(
            DatabaseError::UpgradeRequired(2).failure(),
            RedbFailure::Incompatible
        );
        assert_eq!(
            DatabaseError::RepairAborted.failure(),
            RedbFailure::Internal
        );
        assert_eq!(
            TransactionError::Storage(StorageError::Corrupted("bad page".into())).failure(),
            RedbFailure::Corrupt
        );
        assert_eq!(
            TableError::Storage(io(ErrorKind::PermissionDenied)).failure(),
            RedbFailure::Io(IoFailure::PermissionDenied)
        );
        assert_eq!(
            CommitError::TransactionPoisoned.failure(),
            RedbFailure::Internal
        );
        assert_eq!(
            CursorError::Storage(StorageError::Corrupted("bad page".into())).failure(),
            RedbFailure::Corrupt
        );
        assert_eq!(CursorError::UnorderedKey.failure(), RedbFailure::Internal);
        assert_eq!(
            SavepointError::Storage(StorageError::Corrupted("bad page".into())).failure(),
            RedbFailure::Corrupt
        );
        assert_eq!(
            CompactionError::Storage(StorageError::PreviousIo).failure(),
            RedbFailure::Internal
        );
        assert_eq!(
            SetDurabilityError::PersistentSavepointModified.failure(),
            RedbFailure::Internal
        );
        assert_eq!(
            BackendError::Io(IoError::from(ErrorKind::PermissionDenied)).failure(),
            RedbFailure::Io(IoFailure::PermissionDenied)
        );
        assert_eq!(BackendError::Unsupported.failure(), RedbFailure::Internal);
        assert_eq!(
            redb::Error::from(DatabaseError::DatabaseAlreadyOpen).failure(),
            RedbFailure::AlreadyOpen
        );
        assert_eq!(
            redb::Error::from(DatabaseError::UpgradeRequired(2)).failure(),
            RedbFailure::Incompatible
        );
    }

    #[derive(Debug, thiserror::Error)]
    #[error("failed to open the journal")]
    struct OpenJournal(#[source] DatabaseError);

    #[derive(Debug, thiserror::Error)]
    #[error("failed to start jobs")]
    struct StartJobs(#[source] OpenJournal);

    #[derive(Debug, thiserror::Error)]
    #[error("failed to insert a run")]
    struct InsertRun(#[source] CursorError);

    #[test]
    fn find_failure_walks_owner_error_chains() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("foreign.redb");
        std::fs::write(&path, vec![b'x'; 4096]).unwrap();
        let error = StartJobs(OpenJournal(Database::open(&path).unwrap_err()));

        assert_eq!(find_failure(&error), Some(RedbFailure::Corrupt));
        assert_eq!(
            find_failure(&InsertRun(CursorError::Storage(StorageError::Corrupted(
                "bad page".into()
            )))),
            Some(RedbFailure::Corrupt)
        );
        assert_eq!(
            find_failure(&BackendError::Io(IoError::from(ErrorKind::Interrupted))),
            Some(RedbFailure::Io(IoFailure::Transient)),
            "a backend error is found before its IO source"
        );
        assert_eq!(
            find_failure(&IoError::from(ErrorKind::InvalidData)),
            None,
            "a bare IO error is not a redb failure"
        );
    }
}
