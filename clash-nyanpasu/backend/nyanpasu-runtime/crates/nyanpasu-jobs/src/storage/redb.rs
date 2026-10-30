use crate::{model::*, storage::JobStore};
use jiff::Timestamp;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::{collections::BTreeMap, path::Path};

const RUNS: TableDefinition<&str, &[u8]> = TableDefinition::new("jobs_runs_v1");
const INDEX: TableDefinition<(&str, u64, &str), &str> = TableDefinition::new("jobs_by_job_v1");
const LOGS: TableDefinition<(&str, u64), &[u8]> = TableDefinition::new("jobs_logs_v1");
const META: TableDefinition<&str, u64> = TableDefinition::new("jobs_meta_v1");
fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value).map_err(Error::JournalEncode)
}
fn decode<T: serde::de::DeserializeOwned>(value: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(value).map_err(Error::JournalDecode)
}
fn transaction_error(operation: JournalOperation) -> impl Fn(redb::TransactionError) -> Error {
    move |source| Error::JournalTransaction { operation, source }
}
fn table_error(operation: JournalOperation) -> impl Fn(redb::TableError) -> Error {
    move |source| Error::JournalTable { operation, source }
}
fn storage_error(operation: JournalOperation) -> impl Fn(redb::StorageError) -> Error {
    move |source| Error::JournalStorage { operation, source }
}
fn commit_error(operation: JournalOperation) -> impl Fn(redb::CommitError) -> Error {
    move |source| Error::JournalCommit { operation, source }
}

/// Owns a dedicated database file. Open in a blocking adapter at composition time.
pub struct RedbJobStore {
    db: Database,
}
impl RedbJobStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(Error::JournalDirectory)?;
        }
        let db = Database::create(path).map_err(Error::JournalOpen)?;
        let tx = db
            .begin_write()
            .map_err(transaction_error(JournalOperation::Open))?;
        tx.open_table(RUNS)
            .map_err(table_error(JournalOperation::Open))?;
        tx.open_table(INDEX)
            .map_err(table_error(JournalOperation::Open))?;
        tx.open_table(LOGS)
            .map_err(table_error(JournalOperation::Open))?;
        tx.open_table(META)
            .map_err(table_error(JournalOperation::Open))?;
        tx.commit().map_err(commit_error(JournalOperation::Open))?;
        Ok(Self { db })
    }
}
impl JobStore for RedbJobStore {
    fn recover(&mut self, now: Timestamp) -> Result<(), Error> {
        let tx = self
            .db
            .begin_write()
            .map_err(transaction_error(JournalOperation::Recover))?;
        {
            let mut table = tx
                .open_table(RUNS)
                .map_err(table_error(JournalOperation::Recover))?;
            let mut interrupted = Vec::new();
            for entry in table
                .iter()
                .map_err(storage_error(JournalOperation::Recover))?
            {
                let (id, bytes) = entry.map_err(storage_error(JournalOperation::Recover))?;
                let mut record: RunRecord =
                    serde_json::from_slice(bytes.value()).map_err(Error::JournalRecoveryDecode)?;
                if record.completion().is_none() {
                    record.finished_at = Some(now);
                    record.state = RunState::Finished(Completion {
                        outcome: Outcome::Interrupted,
                        output: Output::None,
                        journal: JournalState::Durable,
                    });
                    // Log batches committed before the crash remain queryable.
                    let logs = tx
                        .open_table(LOGS)
                        .map_err(table_error(JournalOperation::Recover))?;
                    for row in logs
                        .range((id.value(), 0)..=(id.value(), u64::MAX))
                        .map_err(storage_error(JournalOperation::Recover))?
                    {
                        record.last_log_sequence = record.last_log_sequence.max(
                            row.map_err(storage_error(JournalOperation::Recover))?
                                .0
                                .value()
                                .1,
                        );
                    }
                    interrupted.push((id.value().to_owned(), encode(&record)?));
                }
            }
            for (id, bytes) in interrupted {
                table
                    .insert(id.as_str(), bytes.as_slice())
                    .map_err(storage_error(JournalOperation::Recover))?;
            }
        }
        tx.commit().map_err(commit_error(JournalOperation::Recover))
    }
    fn admit(&mut self, mut record: RunRecord) -> Result<RunRecord, Error> {
        let tx = self
            .db
            .begin_write()
            .map_err(transaction_error(JournalOperation::Admit))?;
        let id = record.id.to_string();
        {
            let mut runs = tx
                .open_table(RUNS)
                .map_err(table_error(JournalOperation::Admit))?;
            if runs
                .get(id.as_str())
                .map_err(storage_error(JournalOperation::Admit))?
                .is_some()
            {
                return Err(Error::AlreadySubmitted(record.id));
            }
            let mut meta = tx
                .open_table(META)
                .map_err(table_error(JournalOperation::Admit))?;
            let previous = meta
                .get("sequence")
                .map_err(storage_error(JournalOperation::Admit))?
                .map(|v| v.value())
                .unwrap_or(0);
            record.admission_sequence = previous
                .checked_add(1)
                .ok_or_else(|| Error::Journal("sequence exhausted".into()))?;
            meta.insert("sequence", record.admission_sequence)
                .map_err(storage_error(JournalOperation::Admit))?;
            let bytes = encode(&record)?;
            runs.insert(id.as_str(), bytes.as_slice())
                .map_err(storage_error(JournalOperation::Admit))?;
            tx.open_table(INDEX)
                .map_err(table_error(JournalOperation::Admit))?
                .insert(
                    (record.job.as_str(), record.admission_sequence, id.as_str()),
                    id.as_str(),
                )
                .map_err(storage_error(JournalOperation::Admit))?;
        }
        tx.commit().map_err(commit_error(JournalOperation::Admit))?;
        Ok(record)
    }
    fn append(&mut self, id: RunId, logs: &[RunLog]) -> Result<(), Error> {
        let tx = self
            .db
            .begin_write()
            .map_err(transaction_error(JournalOperation::Append))?;
        {
            let id = id.to_string();
            let runs = tx
                .open_table(RUNS)
                .map_err(table_error(JournalOperation::Append))?;
            let bytes = runs
                .get(id.as_str())
                .map_err(storage_error(JournalOperation::Append))?
                .ok_or(Error::NotFound)?;
            let record: RunRecord = decode(bytes.value())?;
            if record.completion().is_some() {
                return Err(Error::Journal("run sealed".into()));
            }
            let mut table = tx
                .open_table(LOGS)
                .map_err(table_error(JournalOperation::Append))?;
            for log in logs {
                if log.run_id != record.id {
                    return Err(Error::Journal("wrong run log".into()));
                }
                let bytes = encode(log)?;
                table
                    .insert((id.as_str(), log.sequence), bytes.as_slice())
                    .map_err(storage_error(JournalOperation::Append))?;
            }
        }
        tx.commit().map_err(commit_error(JournalOperation::Append))
    }
    fn finish(&mut self, record: &RunRecord, logs: &[RunLog]) -> Result<(), Error> {
        if record.completion().is_none() {
            return Err(Error::Journal("missing completion".into()));
        }
        let tx = self
            .db
            .begin_write()
            .map_err(transaction_error(JournalOperation::Finish))?;
        {
            let id = record.id.to_string();
            let mut runs = tx
                .open_table(RUNS)
                .map_err(table_error(JournalOperation::Finish))?;
            let existing: RunRecord = decode(
                runs.get(id.as_str())
                    .map_err(storage_error(JournalOperation::Finish))?
                    .ok_or(Error::NotFound)?
                    .value(),
            )?;
            if existing.completion().is_some() && &existing != record {
                return Err(Error::Journal("terminal result is immutable".into()));
            }
            if existing.job != record.job
                || existing.admission_sequence != record.admission_sequence
            {
                return Err(Error::Journal("run identity changed".into()));
            }
            let mut table = tx
                .open_table(LOGS)
                .map_err(table_error(JournalOperation::Finish))?;
            for log in logs {
                if log.run_id != record.id {
                    return Err(Error::Journal("wrong run log".into()));
                }
                let bytes = encode(log)?;
                table
                    .insert((id.as_str(), log.sequence), bytes.as_slice())
                    .map_err(storage_error(JournalOperation::Finish))?;
            }
            let bytes = encode(record)?;
            runs.insert(id.as_str(), bytes.as_slice())
                .map_err(storage_error(JournalOperation::Finish))?;
        }
        tx.commit().map_err(commit_error(JournalOperation::Finish))
    }
    fn get(&mut self, id: RunId) -> Result<Option<RunRecord>, Error> {
        let tx = self
            .db
            .begin_read()
            .map_err(transaction_error(JournalOperation::Get))?;
        tx.open_table(RUNS)
            .map_err(table_error(JournalOperation::Get))?
            .get(id.to_string().as_str())
            .map_err(storage_error(JournalOperation::Get))?
            .map(|v| decode(v.value()))
            .transpose()
    }
    fn runs(
        &mut self,
        job: &str,
        after: Option<RunCursor>,
        limit: usize,
    ) -> Result<Page<RunRecord, RunCursor>, Error> {
        let limit = limit.clamp(1, 200);
        let tx = self
            .db
            .begin_read()
            .map_err(transaction_error(JournalOperation::ListRuns))?;
        let index = tx
            .open_table(INDEX)
            .map_err(table_error(JournalOperation::ListRuns))?;
        let runs = tx
            .open_table(RUNS)
            .map_err(table_error(JournalOperation::ListRuns))?;
        let start_id = after.as_ref().map(|c| c.id.to_string()).unwrap_or_default();
        let start = after.as_ref().map(|c| c.sequence).unwrap_or(0);
        let mut items = Vec::new();
        for row in index
            .range((
                std::ops::Bound::Excluded((job, start, start_id.as_str())),
                std::ops::Bound::Included((job, u64::MAX, "\u{10ffff}")),
            ))
            .map_err(storage_error(JournalOperation::ListRuns))?
            .take(limit + 1)
        {
            let (_, id) = row.map_err(storage_error(JournalOperation::ListRuns))?;
            items.push(decode::<RunRecord>(
                runs.get(id.value())
                    .map_err(storage_error(JournalOperation::ListRuns))?
                    .ok_or_else(|| Error::Journal("orphan index".into()))?
                    .value(),
            )?);
        }
        let more = items.len() > limit;
        items.truncate(limit);
        let next = if more {
            items.last().map(|r| RunCursor {
                sequence: r.admission_sequence,
                id: r.id,
            })
        } else {
            None
        };
        Ok(Page { items, next })
    }
    fn logs(&mut self, id: RunId, after: u64, limit: usize) -> Result<Page<RunLog, u64>, Error> {
        let limit = limit.clamp(1, 200);
        let tx = self
            .db
            .begin_read()
            .map_err(transaction_error(JournalOperation::ReadLogs))?;
        let id = id.to_string();
        if tx
            .open_table(RUNS)
            .map_err(table_error(JournalOperation::ReadLogs))?
            .get(id.as_str())
            .map_err(storage_error(JournalOperation::ReadLogs))?
            .is_none()
        {
            return Err(Error::NotFound);
        }
        let logs = tx
            .open_table(LOGS)
            .map_err(table_error(JournalOperation::ReadLogs))?;
        let mut items = Vec::new();
        let mut bytes = 0;
        let mut more = false;
        for row in logs
            .range((
                std::ops::Bound::Excluded((id.as_str(), after)),
                std::ops::Bound::Included((id.as_str(), u64::MAX)),
            ))
            .map_err(storage_error(JournalOperation::ReadLogs))?
        {
            let (_, value) = row.map_err(storage_error(JournalOperation::ReadLogs))?;
            if items.len() == limit
                || (!items.is_empty() && bytes + value.value().len() > 256 * 1024)
            {
                more = true;
                break;
            }
            bytes += value.value().len();
            items.push(decode::<RunLog>(value.value())?);
        }
        let next = if more {
            items.last().map(|r| r.sequence)
        } else {
            None
        };
        Ok(Page { items, next })
    }
    fn prune(&mut self, now: Timestamp, retention: &Retention) -> Result<(), Error> {
        let tx = self
            .db
            .begin_write()
            .map_err(transaction_error(JournalOperation::Prune))?;
        {
            let mut runs = tx
                .open_table(RUNS)
                .map_err(table_error(JournalOperation::Prune))?;
            let mut logs = tx
                .open_table(LOGS)
                .map_err(table_error(JournalOperation::Prune))?;
            let mut index = tx
                .open_table(INDEX)
                .map_err(table_error(JournalOperation::Prune))?;
            let mut terminal = Vec::new();
            for row in runs
                .iter()
                .map_err(storage_error(JournalOperation::Prune))?
            {
                let (id, value) = row.map_err(storage_error(JournalOperation::Prune))?;
                let record: RunRecord =
                    serde_json::from_slice(value.value()).map_err(Error::JournalPruneDecode)?;
                if record.completion().is_some() {
                    let mut bytes = value.value().len();
                    for row in logs
                        .range((id.value(), 0)..=(id.value(), u64::MAX))
                        .map_err(storage_error(JournalOperation::Prune))?
                    {
                        bytes += row
                            .map_err(storage_error(JournalOperation::Prune))?
                            .1
                            .value()
                            .len();
                    }
                    terminal.push((record, bytes));
                }
            }
            terminal.sort_by_key(|(r, _)| std::cmp::Reverse(r.admission_sequence));
            let mut jobs = BTreeMap::<String, usize>::new();
            let mut kept = 0;
            let mut bytes = 0;
            for (record, size) in terminal {
                let age = std::time::Duration::try_from(
                    now.duration_since(record.finished_at.unwrap_or(record.admitted_at)),
                )
                .unwrap_or_default();
                let count = jobs.entry(record.job.clone()).or_default();
                if age > retention.age
                    || *count >= retention.per_job
                    || kept >= retention.runs
                    || bytes + size > retention.bytes
                {
                    let id = record.id.to_string();
                    runs.remove(id.as_str())
                        .map_err(storage_error(JournalOperation::Prune))?;
                    index
                        .remove((record.job.as_str(), record.admission_sequence, id.as_str()))
                        .map_err(storage_error(JournalOperation::Prune))?;
                    let keys: Vec<_> = logs
                        .range((id.as_str(), 0)..=(id.as_str(), u64::MAX))
                        .map_err(storage_error(JournalOperation::Prune))?
                        .map(|r| {
                            r.map(|(k, _)| k.value().1)
                                .map_err(storage_error(JournalOperation::Prune))
                        })
                        .collect::<Result<_, _>>()?;
                    for sequence in keys {
                        logs.remove((id.as_str(), sequence))
                            .map_err(storage_error(JournalOperation::Prune))?;
                    }
                } else {
                    *count += 1;
                    kept += 1;
                    bytes += size;
                }
            }
        }
        tx.commit().map_err(commit_error(JournalOperation::Prune))
    }
    fn health(&mut self) -> Result<(), Error> {
        let tx = self
            .db
            .begin_write()
            .map_err(transaction_error(JournalOperation::Health))?;
        tx.commit().map_err(commit_error(JournalOperation::Health))
    }
}
