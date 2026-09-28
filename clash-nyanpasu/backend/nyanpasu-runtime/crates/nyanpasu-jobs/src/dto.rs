//! Finite wire shapes for adapters. JSON output is bounded text, never a
//! recursive dynamic schema. IDs, revisions and counters preserve JS precision.
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RunDto {
    pub id: String,
    pub job: String,
    pub definition_version: String,
    pub admission_sequence: String,
    pub trigger: String,
    pub scheduled_at: Option<String>,
    pub admitted_at: String,
    pub finished_at: Option<String>,
    pub state: RunStateDto,
    pub last_log_sequence: String,
    pub dropped_log_count: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum RunStateDto {
    Admitted,
    Running,
    Cancelling,
    Finalizing,
    Finished { completion: CompletionDto },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CompletionDto {
    pub outcome: OutcomeDto,
    pub output: OutputDto,
    pub journal: JournalDto,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum OutcomeDto {
    Succeeded,
    Failed { code: String, message: String },
    Cancelled,
    Interrupted,
    Skipped,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum OutputDto {
    JsonText { value: String },
    Unavailable { code: String },
    None,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum JournalDto {
    Durable,
    Degraded { code: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LogDto {
    pub run_id: String,
    pub sequence: String,
    pub time: String,
    pub level: String,
    pub target: String,
    pub fields: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RunCursorDto {
    pub sequence: String,
    pub id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RunPageDto {
    pub items: Vec<RunDto>,
    pub next: Option<RunCursorDto>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LogPageDto {
    pub items: Vec<LogDto>,
    pub next: Option<String>,
}
impl From<RunRecord> for RunDto {
    fn from(r: RunRecord) -> Self {
        let state = match r.state {
            RunState::Admitted => RunStateDto::Admitted,
            RunState::Running => RunStateDto::Running,
            RunState::Cancelling => RunStateDto::Cancelling,
            RunState::Finalizing => RunStateDto::Finalizing,
            RunState::Finished(c) => RunStateDto::Finished {
                completion: c.into(),
            },
        };
        Self {
            id: r.id.to_string(),
            job: r.job,
            definition_version: r.definition_version.to_string(),
            admission_sequence: r.admission_sequence.to_string(),
            trigger: match r.trigger {
                Trigger::Manual => "manual",
                Trigger::Once => "once",
                Trigger::Interval => "interval",
                Trigger::Cron => "cron",
                Trigger::StartupCatchUp => "startup_catch_up",
            }
            .into(),
            scheduled_at: r.scheduled_at.map(|t| t.to_string()),
            admitted_at: r.admitted_at.to_string(),
            finished_at: r.finished_at.map(|t| t.to_string()),
            state,
            last_log_sequence: r.last_log_sequence.to_string(),
            dropped_log_count: r.dropped_log_count.to_string(),
        }
    }
}
impl From<Completion> for CompletionDto {
    fn from(c: Completion) -> Self {
        Self {
            outcome: match c.outcome {
                Outcome::Succeeded => OutcomeDto::Succeeded,
                Outcome::Failed(e) => OutcomeDto::Failed {
                    code: e.code,
                    message: e.message,
                },
                Outcome::Cancelled => OutcomeDto::Cancelled,
                Outcome::Interrupted => OutcomeDto::Interrupted,
                Outcome::Skipped => OutcomeDto::Skipped,
            },
            output: match c.output {
                Output::Json(value) => OutputDto::JsonText { value },
                Output::Unavailable(code) => OutputDto::Unavailable { code },
                Output::None => OutputDto::None,
            },
            journal: match c.journal {
                JournalState::Durable => JournalDto::Durable,
                JournalState::Degraded(code) => JournalDto::Degraded { code },
            },
        }
    }
}
impl From<RunLog> for LogDto {
    fn from(l: RunLog) -> Self {
        Self {
            run_id: l.run_id.to_string(),
            sequence: l.sequence.to_string(),
            time: l.time.to_string(),
            level: l.level,
            target: l.target,
            fields: l.fields,
        }
    }
}
impl From<RunCursor> for RunCursorDto {
    fn from(c: RunCursor) -> Self {
        Self {
            sequence: c.sequence.to_string(),
            id: c.id.to_string(),
        }
    }
}
impl TryFrom<RunCursorDto> for RunCursor {
    type Error = Error;
    fn try_from(c: RunCursorDto) -> Result<Self, Error> {
        Ok(Self {
            sequence: c
                .sequence
                .parse()
                .map_err(|_| Error::Invalid("invalid cursor sequence".into()))?,
            id: c
                .id
                .parse()
                .map_err(|_| Error::Invalid("invalid cursor ID".into()))?,
        })
    }
}
impl From<Page<RunRecord, RunCursor>> for RunPageDto {
    fn from(p: Page<RunRecord, RunCursor>) -> Self {
        Self {
            items: p.items.into_iter().map(Into::into).collect(),
            next: p.next.map(Into::into),
        }
    }
}
impl From<Page<RunLog, u64>> for LogPageDto {
    fn from(p: Page<RunLog, u64>) -> Self {
        Self {
            items: p.items.into_iter().map(Into::into).collect(),
            next: p.next.map(|c| c.to_string()),
        }
    }
}
