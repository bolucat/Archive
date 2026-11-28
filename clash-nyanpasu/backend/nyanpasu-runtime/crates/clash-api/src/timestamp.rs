use std::{borrow::Cow, cmp::Ordering, str::FromStr};

use jiff::{
    Timestamp,
    fmt::temporal::{DateTimeParser, DateTimePrinter},
    tz::Offset,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// An instant together with the UTC offset it was written with.
///
/// Neither `jiff::Timestamp` (always UTC) nor `jiff::Zoned` (adds a bracketed
/// zone annotation) round-trips a controller's RFC 3339 text. This type keeps
/// the offset and serializes with 0, 3, 6 or 9 fraction digits, writing a zero
/// offset as `Z`. Comparison looks at the instant only.
#[derive(Clone, Copy, Debug)]
pub struct OffsetTimestamp {
    timestamp: Timestamp,
    offset: Offset,
}

impl OffsetTimestamp {
    pub fn new(timestamp: Timestamp, offset: Offset) -> Self {
        Self { timestamp, offset }
    }

    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    pub fn offset(&self) -> Offset {
        self.offset
    }

    /// RFC 3339 text that always has a numeric offset: UTC is `+00:00`.
    pub fn to_rfc3339(&self) -> String {
        self.printer()
            .timestamp_with_offset_to_string(&self.timestamp, self.offset)
    }

    fn printer(&self) -> DateTimePrinter {
        let nanos = self.timestamp.subsec_nanosecond().unsigned_abs();
        let precision = match nanos {
            0 => 0,
            _ if nanos % 1_000_000 == 0 => 3,
            _ if nanos % 1_000 == 0 => 6,
            _ => 9,
        };
        DateTimePrinter::new().precision(Some(precision))
    }
}

impl PartialEq for OffsetTimestamp {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
    }
}

impl Eq for OffsetTimestamp {}

impl PartialOrd for OffsetTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OffsetTimestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl std::hash::Hash for OffsetTimestamp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.timestamp.hash(state);
    }
}

impl FromStr for OffsetTimestamp {
    type Err = jiff::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = truncate_fraction(value);
        let pieces = DateTimeParser::new().parse_pieces(value.as_ref())?;
        let (Some(time), Some(offset)) = (pieces.time(), pieces.offset()) else {
            return Err(jiff::Error::from_args(format_args!(
                "expected an RFC 3339 date, time and offset, got `{value}`"
            )));
        };
        let offset = offset.to_numeric_offset();
        let timestamp = offset.to_timestamp(pieces.date().to_datetime(time))?;
        Ok(Self { timestamp, offset })
    }
}

/// RFC 3339 does not limit the fraction length, but jiff stops at nanoseconds.
/// Drops the digits past the ninth.
fn truncate_fraction(value: &str) -> Cow<'_, str> {
    let Some(start) = value.find('.').map(|dot| dot + 1) else {
        return Cow::Borrowed(value);
    };
    let digits = value[start..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    if digits <= 9 {
        return Cow::Borrowed(value);
    }
    Cow::Owned(format!(
        "{}{}",
        &value[..start + 9],
        &value[start + digits..]
    ))
}

impl Serialize for OffsetTimestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text = if self.offset == Offset::UTC {
            self.printer().timestamp_to_string(&self.timestamp)
        } else {
            self.to_rfc3339()
        };
        serializer.serialize_str(&text)
    }
}

impl<'de> Deserialize<'de> for OffsetTimestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl de::Visitor<'_> for Visitor {
            type Value = OffsetTimestamp;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an RFC 3339 formatted date and time string")
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                value.parse().map_err(E::custom)
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

impl specta::Type for OffsetTimestamp {
    fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
        String::definition(types)
    }
}
