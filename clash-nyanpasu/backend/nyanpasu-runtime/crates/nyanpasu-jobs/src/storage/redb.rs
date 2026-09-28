use crate::{model::*, storage::JobStore};
use jiff::Timestamp;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::{collections::BTreeMap, path::Path};

const RUNS: TableDefinition<&str, &[u8]> = TableDefinition::new("jobs_runs_v1");
const INDEX: TableDefinition<(&str, u64, &str), &str> = TableDefinition::new("jobs_by_job_v1");
const LOGS: TableDefinition<(&str, u64), &[u8]> = TableDefinition::new("jobs_logs_v1");
const META: TableDefinition<&str, u64> = TableDefinition::new("jobs_meta_v1");
fn err(e: impl std::fmt::Display) -> Error {
    Error::Journal(e.to_string())
}
fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value).map_err(err)
}
fn decode<T: serde::de::DeserializeOwned>(value: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(value).map_err(err)
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
            std::fs::create_dir_all(parent).map_err(err)?;
        }
        let db = Database::create(path).map_err(err)?;
        let tx = db.begin_write().map_err(err)?;
        tx.open_table(RUNS).map_err(err)?;
        tx.open_table(INDEX).map_err(err)?;
        tx.open_table(LOGS).map_err(err)?;
        tx.open_table(META).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Self { db })
    }
}
impl JobStore for RedbJobStore {
    fn recover(&mut self, now: Timestamp) -> Result<(), Error> {
        let tx = self.db.begin_write().map_err(err)?;
        {
            let mut table = tx.open_table(RUNS).map_err(err)?;
            let mut interrupted = Vec::new();
            for entry in table.iter().map_err(err)? {
                let (id, bytes) = entry.map_err(err)?;
                let mut record: RunRecord = decode(bytes.value())?;
                if record.completion().is_none() {
                    record.finished_at = Some(now);
                    record.state = RunState::Finished(Completion {
                        outcome: Outcome::Interrupted,
                        output: Output::None,
                        journal: JournalState::Durable,
                    });
                    // Log batches committed before the crash remain queryable.
                    let logs = tx.open_table(LOGS).map_err(err)?;
                    for row in logs
                        .range((id.value(), 0)..=(id.value(), u64::MAX))
                        .map_err(err)?
                    {
                        record.last_log_sequence =
                            record.last_log_sequence.max(row.map_err(err)?.0.value().1);
                    }
                    interrupted.push((id.value().to_owned(), encode(&record)?));
                }
            }
            for (id, bytes) in interrupted {
                table.insert(id.as_str(), bytes.as_slice()).map_err(err)?;
            }
        }
        tx.commit().map_err(err)
    }
    fn admit(&mut self, mut record: RunRecord) -> Result<RunRecord, Error> {
        let tx = self.db.begin_write().map_err(err)?;
        let id = record.id.to_string();
        {
            let mut runs = tx.open_table(RUNS).map_err(err)?;
            if runs.get(id.as_str()).map_err(err)?.is_some() {
                return Err(Error::AlreadySubmitted(record.id));
            }
            let mut meta = tx.open_table(META).map_err(err)?;
            let previous = meta
                .get("sequence")
                .map_err(err)?
                .map(|v| v.value())
                .unwrap_or(0);
            record.admission_sequence = previous
                .checked_add(1)
                .ok_or_else(|| err("sequence exhausted"))?;
            meta.insert("sequence", record.admission_sequence)
                .map_err(err)?;
            let bytes = encode(&record)?;
            runs.insert(id.as_str(), bytes.as_slice()).map_err(err)?;
            tx.open_table(INDEX)
                .map_err(err)?
                .insert(
                    (record.job.as_str(), record.admission_sequence, id.as_str()),
                    id.as_str(),
                )
                .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(record)
    }
    fn append(&mut self, id: RunId, logs: &[RunLog]) -> Result<(), Error> {
        let tx = self.db.begin_write().map_err(err)?;
        {
            let id = id.to_string();
            let runs = tx.open_table(RUNS).map_err(err)?;
            let bytes = runs.get(id.as_str()).map_err(err)?.ok_or(Error::NotFound)?;
            let record: RunRecord = decode(bytes.value())?;
            if record.completion().is_some() {
                return Err(err("run sealed"));
            }
            let mut table = tx.open_table(LOGS).map_err(err)?;
            for log in logs {
                if log.run_id != record.id {
                    return Err(err("wrong run log"));
                }
                let bytes = encode(log)?;
                table
                    .insert((id.as_str(), log.sequence), bytes.as_slice())
                    .map_err(err)?;
            }
        }
        tx.commit().map_err(err)
    }
    fn finish(&mut self, record: &RunRecord, logs: &[RunLog]) -> Result<(), Error> {
        if record.completion().is_none() {
            return Err(err("missing completion"));
        }
        let tx = self.db.begin_write().map_err(err)?;
        {
            let id = record.id.to_string();
            let mut runs = tx.open_table(RUNS).map_err(err)?;
            let existing: RunRecord = decode(
                runs.get(id.as_str())
                    .map_err(err)?
                    .ok_or(Error::NotFound)?
                    .value(),
            )?;
            if existing.completion().is_some() && &existing != record {
                return Err(err("terminal result is immutable"));
            }
            if existing.job != record.job
                || existing.admission_sequence != record.admission_sequence
            {
                return Err(err("run identity changed"));
            }
            let mut table = tx.open_table(LOGS).map_err(err)?;
            for log in logs {
                if log.run_id != record.id {
                    return Err(err("wrong run log"));
                }
                let bytes = encode(log)?;
                table
                    .insert((id.as_str(), log.sequence), bytes.as_slice())
                    .map_err(err)?;
            }
            let bytes = encode(record)?;
            runs.insert(id.as_str(), bytes.as_slice()).map_err(err)?;
        }
        tx.commit().map_err(err)
    }
    fn get(&mut self, id: RunId) -> Result<Option<RunRecord>, Error> {
        let tx = self.db.begin_read().map_err(err)?;
        tx.open_table(RUNS)
            .map_err(err)?
            .get(id.to_string().as_str())
            .map_err(err)?
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
        let tx = self.db.begin_read().map_err(err)?;
        let index = tx.open_table(INDEX).map_err(err)?;
        let runs = tx.open_table(RUNS).map_err(err)?;
        let start_id = after.as_ref().map(|c| c.id.to_string()).unwrap_or_default();
        let start = after.as_ref().map(|c| c.sequence).unwrap_or(0);
        let mut items = Vec::new();
        for row in index
            .range((
                std::ops::Bound::Excluded((job, start, start_id.as_str())),
                std::ops::Bound::Included((job, u64::MAX, "\u{10ffff}")),
            ))
            .map_err(err)?
            .take(limit + 1)
        {
            let (_, id) = row.map_err(err)?;
            items.push(decode::<RunRecord>(
                runs.get(id.value())
                    .map_err(err)?
                    .ok_or_else(|| err("orphan index"))?
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
        let tx = self.db.begin_read().map_err(err)?;
        let id = id.to_string();
        if tx
            .open_table(RUNS)
            .map_err(err)?
            .get(id.as_str())
            .map_err(err)?
            .is_none()
        {
            return Err(Error::NotFound);
        }
        let logs = tx.open_table(LOGS).map_err(err)?;
        let mut items = Vec::new();
        let mut bytes = 0;
        let mut more = false;
        for row in logs
            .range((
                std::ops::Bound::Excluded((id.as_str(), after)),
                std::ops::Bound::Included((id.as_str(), u64::MAX)),
            ))
            .map_err(err)?
        {
            let (_, value) = row.map_err(err)?;
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
        let tx = self.db.begin_write().map_err(err)?;
        {
            let mut runs = tx.open_table(RUNS).map_err(err)?;
            let mut logs = tx.open_table(LOGS).map_err(err)?;
            let mut index = tx.open_table(INDEX).map_err(err)?;
            let mut terminal = Vec::new();
            for row in runs.iter().map_err(err)? {
                let (id, value) = row.map_err(err)?;
                let record: RunRecord = decode(value.value())?;
                if record.completion().is_some() {
                    let mut bytes = value.value().len();
                    for row in logs
                        .range((id.value(), 0)..=(id.value(), u64::MAX))
                        .map_err(err)?
                    {
                        bytes += row.map_err(err)?.1.value().len();
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
                    runs.remove(id.as_str()).map_err(err)?;
                    index
                        .remove((record.job.as_str(), record.admission_sequence, id.as_str()))
                        .map_err(err)?;
                    let keys: Vec<_> = logs
                        .range((id.as_str(), 0)..=(id.as_str(), u64::MAX))
                        .map_err(err)?
                        .map(|r| r.map(|(k, _)| k.value().1).map_err(err))
                        .collect::<Result<_, _>>()?;
                    for sequence in keys {
                        logs.remove((id.as_str(), sequence)).map_err(err)?;
                    }
                } else {
                    *count += 1;
                    kept += 1;
                    bytes += size;
                }
            }
        }
        tx.commit().map_err(err)
    }
    fn health(&mut self) -> Result<(), Error> {
        let tx = self.db.begin_write().map_err(err)?;
        tx.commit().map_err(err)
    }
}
