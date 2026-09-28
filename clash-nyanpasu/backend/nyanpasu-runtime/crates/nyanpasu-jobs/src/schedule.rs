use crate::Error;
use jiff::{RoundMode, Timestamp, TimestampRound, Unit, tz::TimeZone};
use jiff_cron::Schedule as CronSchedule;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Schedule {
    #[default]
    Manual,
    Once {
        delay_ms: u64,
    },
    Interval {
        every_ms: u64,
    },
    Cron {
        expression: String,
        timezone: String,
    },
}
impl Schedule {
    pub fn validate(&self, now: Timestamp) -> Result<(), Error> {
        match self {
            Self::Interval { every_ms: 0 } => Err(Error::Invalid("zero interval".into())),
            Self::Once { delay_ms } | Self::Interval { every_ms: delay_ms }
                if *delay_ms > 10 * 365 * 86400 * 1000 =>
            {
                Err(Error::Invalid("delay exceeds ten years".into()))
            }
            Self::Cron { .. } => self.next_cron(now).map(|_| ()),
            _ => Ok(()),
        }
    }
    pub fn next_cron(&self, after: Timestamp) -> Result<Timestamp, Error> {
        let Self::Cron {
            expression,
            timezone,
        } = self
        else {
            return Err(Error::Invalid("not cron".into()));
        };
        let timezone =
            TimeZone::get(timezone).map_err(|_| Error::Invalid("unknown IANA timezone".into()))?;
        let expression = if expression.starts_with('@') {
            expression.clone()
        } else {
            match expression.split_whitespace().count() {
                5 => format!("0 {expression}"),
                6 => expression.clone(),
                _ => return Err(Error::Invalid("cron requires five or six fields".into())),
            }
        };
        let cron: CronSchedule = expression
            .parse()
            .map_err(|e| Error::Invalid(format!("{e}")))?;
        // Cron fires on whole seconds. Floor before searching to avoid the
        // library's fractional-second fast path skipping weekday checks.
        let after = after
            .round(
                TimestampRound::new()
                    .smallest(Unit::Second)
                    .mode(RoundMode::Floor),
            )
            .map_err(|e| Error::Invalid(e.to_string()))?;
        cron.after(after.to_zoned(timezone))
            .next()
            .map(|zoned| zoned.timestamp())
            .ok_or_else(|| Error::Invalid("no future cron occurrence".into()))
    }
}
