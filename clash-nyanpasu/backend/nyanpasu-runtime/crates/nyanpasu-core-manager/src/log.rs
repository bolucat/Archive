//! Normalization of core console output into a single [`LogFrame`] shape.
//!
//! Each kind prints its own layout (mihomo logfmt, clash premium's `PrettyPrint`,
//! and two different tracing formats), all of them on stdout, and only meow keeps
//! ANSI when writing to a pipe. Parsing is header-only and per kind: a line whose
//! header does not match degrades to an unformatted frame instead of being lost.

use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use clash_api::LogField;

use crate::kind::CoreKind;

pub(crate) const LOG_CHANNEL_CAPACITY: usize = 256;
const MAX_CONTINUATION_LINES: usize = 16;
const MAX_LOGICAL_FRAME_BYTES: usize = 16 * 1024;

/// Normalized severity. Go's `fatal` and `panic` both terminate the process, so
/// they collapse into `Fatal`; the original spelling survives in [`LogFrame::raw`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogTimestamp {
    /// Exactly as the core printed it.
    pub raw: String,
    /// Unix milliseconds, the same convention as `state::now_ms`.
    pub unix_ms: Option<i64>,
    /// The date or the zone offset came from the observation instant because the
    /// core did not print one (clash premium and clash-rs).
    pub inferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFrame {
    pub epoch: u64,
    pub kind: CoreKind,
    pub stream: LogStream,
    pub timestamp: Option<LogTimestamp>,
    pub level: LogLevel,
    /// clash premium's `[Tag]` prefix (a message convention, not a field), meow's
    /// tracing target, or clash-rs's `file:line`.
    pub target: Option<String>,
    pub message: String,
    /// Filled only where field boundaries are decidable, which today means
    /// mihomo's quoted logfmt.
    pub fields: Vec<LogField>,
    /// The whole logical record with ANSI removed, continuations included.
    pub raw: String,
    /// Continuations were dropped after hitting a size limit.
    pub truncated: bool,
}

/// At most two frames leave the parser per line: a flushed multi-line record and
/// the line that ended it.
pub(crate) type ParsedFrames = [Option<LogFrame>; 2];

struct ParsedLine {
    timestamp: Option<LogTimestamp>,
    level: LogLevel,
    target: Option<String>,
    message: String,
    fields: Vec<LogField>,
}

struct Pending {
    frame: LogFrame,
    continuations: usize,
}

impl Pending {
    fn append(&mut self, line: &str) {
        if self.continuations == MAX_CONTINUATION_LINES
            || self.frame.raw.len() + 1 + line.len() > MAX_LOGICAL_FRAME_BYTES
        {
            self.frame.truncated = true;
            return;
        }
        self.frame.raw.push('\n');
        self.frame.raw.push_str(line);
        self.frame.message.push('\n');
        self.frame.message.push_str(line);
        self.continuations += 1;
    }
}

/// Stateful because clash premium needs the previous timestamp to infer date
/// rollovers, and because a fatal record can span several lines.
pub(crate) struct LogParser {
    kind: CoreKind,
    epoch: u64,
    premium_clock: Option<(NaiveDate, NaiveTime)>,
    pending: [Option<Pending>; 2],
    newest_pending: Option<LogStream>,
}

impl LogParser {
    pub(crate) fn new(kind: CoreKind, epoch: u64) -> Self {
        Self {
            kind,
            epoch,
            premium_clock: None,
            pending: [None, None],
            newest_pending: None,
        }
    }

    pub(crate) fn push(&mut self, stream: LogStream, line: String) -> ParsedFrames {
        self.push_at(stream, line, chrono::Local::now().fixed_offset())
    }

    pub(crate) fn push_at(
        &mut self,
        stream: LogStream,
        line: String,
        observed_at: DateTime<FixedOffset>,
    ) -> ParsedFrames {
        let line = strip_ansi(line);
        if let Some(parsed) = self.parse_header(&line, observed_at) {
            let hold = parsed.level >= LogLevel::Error;
            let frame = LogFrame {
                epoch: self.epoch,
                kind: self.kind,
                stream,
                timestamp: parsed.timestamp,
                level: parsed.level,
                target: parsed.target,
                message: parsed.message,
                fields: parsed.fields,
                raw: line,
                truncated: false,
            };
            return self.emit(stream, frame, hold);
        }

        // clash-rs and meow also write plain, level-less text to stderr.
        let error_root = line.starts_with("Error:");
        let warning_root = line.starts_with("warning:");
        if !error_root
            && !warning_root
            && let Some(pending) = self.pending[stream_index(stream)].as_mut()
        {
            pending.append(&line);
            return [None, None];
        }
        let level = match (error_root, warning_root, stream) {
            (true, _, _) => LogLevel::Error,
            (_, true, _) => LogLevel::Warning,
            (.., LogStream::Stdout) => LogLevel::Info,
            (.., LogStream::Stderr) => LogLevel::Warning,
        };
        let frame = LogFrame {
            epoch: self.epoch,
            kind: self.kind,
            stream,
            timestamp: None,
            level,
            target: None,
            message: line.clone(),
            fields: Vec::new(),
            raw: line,
            truncated: false,
        };
        self.emit(stream, frame, error_root && stream == LogStream::Stderr)
    }

    /// Releases records still waiting for continuations, for process termination
    /// and for buffered one-shot output.
    pub(crate) fn finish(&mut self) -> ParsedFrames {
        let stdout = self.pending[0].take().map(|pending| pending.frame);
        let stderr = self.pending[1].take().map(|pending| pending.frame);
        // Oldest first, so consumers that read the tail backwards still see the
        // newest record first when both streams held one.
        match self.newest_pending.take() {
            Some(LogStream::Stdout) => [stderr, stdout],
            _ => [stdout, stderr],
        }
    }

    fn parse_header(
        &mut self,
        line: &str,
        observed_at: DateTime<FixedOffset>,
    ) -> Option<ParsedLine> {
        match self.kind {
            CoreKind::Mihomo => parse_mihomo(line),
            CoreKind::Meow => parse_meow(line),
            CoreKind::ClashRust => parse_clash_rs(line, observed_at),
            CoreKind::ClashPremium => parse_premium(line, observed_at, &mut self.premium_clock),
        }
    }

    /// Continuations only ever attach to a root from the same stream, so an
    /// interleaved stdout line cannot be glued onto a stderr error block.
    fn emit(&mut self, stream: LogStream, frame: LogFrame, hold: bool) -> ParsedFrames {
        let index = stream_index(stream);
        let flushed = self.pending[index].take().map(|pending| pending.frame);
        if hold {
            self.newest_pending = Some(stream);
            self.pending[index] = Some(Pending {
                frame,
                continuations: 0,
            });
            [flushed, None]
        } else {
            [flushed, Some(frame)]
        }
    }
}

/// The most severe recent frame, latest first within that severity. `None` when
/// nothing above `Info` was logged.
pub(crate) fn error_summary(frames: &[LogFrame]) -> Option<String> {
    let level = frames
        .iter()
        .map(|frame| frame.level)
        .filter(|level| *level >= LogLevel::Warning)
        .max()?;
    frames
        .iter()
        .rev()
        .find(|frame| frame.level == level)
        .map(|frame| frame.message.clone())
}

pub(crate) fn format_tail(frames: &[LogFrame]) -> String {
    frames
        .iter()
        .map(|frame| frame.raw.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Condenses the buffered output of a one-shot run into a single cause.
pub(crate) fn summarize_output(kind: CoreKind, stdout: &str, stderr: &str) -> String {
    let mut parser = LogParser::new(kind, 0);
    let mut frames = Vec::new();
    let mut drain = |stream, text: &str| {
        for line in text.lines() {
            frames.extend(parser.push(stream, line.to_owned()).into_iter().flatten());
        }
    };
    drain(LogStream::Stdout, stdout);
    drain(LogStream::Stderr, stderr);
    frames.extend(parser.finish().into_iter().flatten());
    error_summary(&frames).unwrap_or_else(|| verbatim_output(stdout, stderr))
}

fn verbatim_output(stdout: &str, stderr: &str) -> String {
    let output = [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if output.is_empty() {
        "core reported no output".to_owned()
    } else {
        output
    }
}

fn stream_index(stream: LogStream) -> usize {
    match stream {
        LogStream::Stdout => 0,
        LogStream::Stderr => 1,
    }
}

/// Drops complete CSI sequences rather than bare escapes, so `[32m` cannot leak
/// into a level or target. Lines without an escape keep their allocation.
fn strip_ansi(line: String) -> String {
    if !line.as_bytes().contains(&0x1b) {
        return line;
    }
    let bytes = line.into_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0x1b {
            output.push(bytes[index]);
            index += 1;
            continue;
        }
        if bytes.get(index + 1) != Some(&b'[') {
            index += 1;
            continue;
        }
        index += 2;
        // Parameter and intermediate bytes run until the final byte, which is the
        // only ASCII byte in the sequence and therefore never splits a character.
        while index < bytes.len() && !(0x40..=0x7e).contains(&bytes[index]) {
            index += 1;
        }
        index = (index + 1).min(bytes.len());
    }
    String::from_utf8(output).expect("dropping ASCII escapes preserves UTF-8")
}

fn parse_level(level: &str) -> Option<LogLevel> {
    match level {
        "trace" | "TRC" | "TRACE" => Some(LogLevel::Trace),
        "debug" | "DBG" | "DEBUG" => Some(LogLevel::Debug),
        "info" | "INF" | "INFO" => Some(LogLevel::Info),
        "warn" | "warning" | "WRN" | "WARN" => Some(LogLevel::Warning),
        "error" | "ERR" | "ERROR" => Some(LogLevel::Error),
        "fatal" | "panic" | "FTL" | "PNC" => Some(LogLevel::Fatal),
        // phuslu/log's placeholder for a level it does not know.
        "???" => Some(LogLevel::Info),
        _ => None,
    }
}

/// logrus logfmt: `time="..." level=... msg="..."` followed by the call site's
/// own fields, all of them quoted when they need to be.
fn parse_mihomo(line: &str) -> Option<ParsedLine> {
    let mut raw_time = None;
    let mut level = None;
    let mut message = None;
    let mut fields = Vec::new();
    for (key, value) in scan_logfmt(line)? {
        match key.as_str() {
            "time" => raw_time = Some(value),
            "level" => level = parse_level(&value),
            "msg" => message = Some(value),
            _ => fields.push(LogField { key, value }),
        }
    }
    let raw_time = raw_time?;
    Some(ParsedLine {
        timestamp: Some(LogTimestamp {
            unix_ms: parse_rfc3339_ms(&raw_time),
            raw: raw_time,
            inferred: false,
        }),
        level: level?,
        target: None,
        message: message?,
        fields,
    })
}

fn scan_logfmt(line: &str) -> Option<Vec<(String, String)>> {
    let bytes = line.as_bytes();
    let mut fields = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if index == bytes.len() {
            break;
        }
        let key_start = index;
        while index < bytes.len() && bytes[index] != b'=' {
            if bytes[index].is_ascii_whitespace() {
                return None;
            }
            index += 1;
        }
        if index == key_start || index == bytes.len() {
            return None;
        }
        let key = line.get(key_start..index)?.to_owned();
        index += 1;

        let value = if bytes.get(index) == Some(&b'"') {
            index += 1;
            let mut value = Vec::new();
            let mut closed = false;
            while index < bytes.len() {
                match bytes[index] {
                    b'"' => {
                        index += 1;
                        closed = true;
                        break;
                    }
                    b'\\' => {
                        let escaped = *bytes.get(index + 1)?;
                        match escaped {
                            b'"' | b'\\' => value.push(escaped),
                            b'n' => value.push(b'\n'),
                            b'r' => value.push(b'\r'),
                            b't' => value.push(b'\t'),
                            _ => value.extend_from_slice(&[b'\\', escaped]),
                        }
                        index += 2;
                    }
                    byte => {
                        value.push(byte);
                        index += 1;
                    }
                }
            }
            if !closed
                || bytes
                    .get(index)
                    .is_some_and(|byte| !byte.is_ascii_whitespace())
            {
                return None;
            }
            String::from_utf8(value).ok()?
        } else {
            let value_start = index;
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            line.get(value_start..index)?.to_owned()
        };
        fields.push((key, value));
    }
    Some(fields)
}

/// tracing-subscriber's default layout:
/// `<RFC3339 UTC> <right-aligned level> <target>: message`.
fn parse_meow(line: &str) -> Option<ParsedLine> {
    let (raw_time, rest) = take_token(line)?;
    let (level, rest) = take_token(rest)?;
    let level = parse_level(level)?;
    let (target, message) = rest.split_once(": ")?;
    if target.is_empty() {
        return None;
    }
    Some(ParsedLine {
        timestamp: Some(LogTimestamp {
            unix_ms: parse_rfc3339_ms(raw_time),
            raw: raw_time.to_owned(),
            inferred: false,
        }),
        level,
        target: Some(target.to_owned()),
        message: message.to_owned(),
        fields: Vec::new(),
    })
}

/// `<timestamp> <LEVEL> [ThreadId(N)] [target] <file:line>: message`. The two
/// middle segments only exist in debug builds of the core, so the header ends at
/// the first `file:line` anchor rather than at a fixed offset from the level.
fn parse_clash_rs(line: &str, observed_at: DateTime<FixedOffset>) -> Option<ParsedLine> {
    let timestamp_end = clash_rs_timestamp_end(line)?;
    let raw_time = line.get(..timestamp_end)?;
    let (level, rest) = take_token(line.get(timestamp_end..)?)?;
    let level = parse_level(level)?;
    let (target, message) = clash_rs_source(rest)?;
    Some(ParsedLine {
        timestamp: Some(LogTimestamp {
            unix_ms: clash_rs_unix_ms(raw_time, observed_at.offset()),
            raw: raw_time.to_owned(),
            inferred: true,
        }),
        level,
        target: Some(target.to_owned()),
        message: message.to_owned(),
        fields: Vec::new(),
    })
}

/// `yy-MM-dd HH:mm:ss:<subsecond>`. The separator before the subsecond is a
/// colon and `time`'s `[subsecond]` prints 1 to 9 digits, so no RFC3339 parser
/// applies.
fn clash_rs_timestamp_end(line: &str) -> Option<usize> {
    const SEPARATORS: [(usize, u8); 6] = [
        (2, b'-'),
        (5, b'-'),
        (8, b' '),
        (11, b':'),
        (14, b':'),
        (17, b':'),
    ];
    const NUMBERS: [(usize, usize); 6] = [(0, 2), (3, 5), (6, 8), (9, 11), (12, 14), (15, 17)];
    let bytes = line.as_bytes();
    if !SEPARATORS
        .iter()
        .all(|(index, separator)| bytes.get(*index) == Some(separator))
        || !NUMBERS
            .iter()
            .all(|(start, end)| bytes[*start..*end].iter().all(u8::is_ascii_digit))
    {
        return None;
    }
    let digits = bytes[18..]
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    // The trailing space is what separates the header from the level; without it
    // an arbitrary line of the same shape would be accepted as a record root.
    ((1..=9).contains(&digits) && bytes.get(18 + digits) == Some(&b' ')).then_some(18 + digits)
}

fn clash_rs_unix_ms(raw: &str, offset: &FixedOffset) -> Option<i64> {
    let number = |range: std::ops::Range<usize>| raw.get(range)?.parse::<u32>().ok();
    let subsecond = raw.get(18..)?;
    let nanos = subsecond.parse::<u32>().ok()? * 10_u32.pow(9 - subsecond.len() as u32);
    let date = NaiveDate::from_ymd_opt(2000 + number(0..2)? as i32, number(3..5)?, number(6..8)?)?;
    let time =
        NaiveTime::from_hms_nano_opt(number(9..11)?, number(12..14)?, number(15..17)?, nanos)?;
    local_unix_ms(date, time, offset)
}

/// Scans forward: neither `ThreadId(N)` nor a tracing target can look like
/// `<path>:<line>: `, so the first match closes the header, while a later one
/// may well be an ordinary `file:line:` reference inside the message.
fn clash_rs_source(rest: &str) -> Option<(&str, &str)> {
    for (separator, _) in rest.match_indices(": ") {
        let head = &rest[..separator];
        let Some(colon) = head.rfind(':') else {
            continue;
        };
        if head[colon + 1..].is_empty()
            || !head[colon + 1..].bytes().all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        let start = head[..colon]
            .rfind(char::is_whitespace)
            .map_or(0, |index| index + 1);
        if start == colon {
            continue;
        }
        return Some((&head[start..], &rest[separator + 2..]));
    }
    None
}

/// `HH:MM:SS LVL message (key=value)*`. The trailing pairs are printed without
/// quoting and their values may contain spaces and colons, so they stay in the
/// message instead of becoming fields.
fn parse_premium(
    line: &str,
    observed_at: DateTime<FixedOffset>,
    clock: &mut Option<(NaiveDate, NaiveTime)>,
) -> Option<ParsedLine> {
    let bytes = line.as_bytes();
    if bytes.get(8) != Some(&b' ') || bytes.get(12) != Some(&b' ') {
        return None;
    }
    let raw_time = line.get(..8)?;
    let time = NaiveTime::parse_from_str(raw_time, "%H:%M:%S").ok()?;
    let level = parse_level(line.get(9..12)?)?;
    let (target, message) = premium_target(line.get(13..)?);
    Some(ParsedLine {
        timestamp: Some(LogTimestamp {
            unix_ms: premium_unix_ms(time, observed_at, clock),
            raw: raw_time.to_owned(),
            inferred: true,
        }),
        level,
        target,
        message,
        fields: Vec::new(),
    })
}

/// The `[Tag]` prefix is a literal the call site writes into the message, not a
/// structured field, so failing to find one is normal.
fn premium_target(body: &str) -> (Option<String>, String) {
    let tagged = body
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("] "));
    match tagged {
        Some((tag, message)) if !tag.is_empty() && !tag.contains(' ') => {
            (Some(tag.to_owned()), message.to_owned())
        }
        _ => (None, body.to_owned()),
    }
}

/// premium prints no date. The first line adopts the observed one — a printed
/// time more than half a day ahead of it belongs to the previous day — and later
/// lines roll forward when the clock falls from end-of-day back to start-of-day.
fn premium_unix_ms(
    time: NaiveTime,
    observed_at: DateTime<FixedOffset>,
    clock: &mut Option<(NaiveDate, NaiveTime)>,
) -> Option<i64> {
    let date = match *clock {
        Some((date, previous)) if previous.signed_duration_since(time) > Duration::hours(12) => {
            date.succ_opt()?
        }
        Some((date, _)) => date,
        None => {
            let observed = observed_at.date_naive();
            if time.signed_duration_since(observed_at.time()) > Duration::hours(12) {
                observed.pred_opt()?
            } else {
                observed
            }
        }
    };
    *clock = Some((date, time));
    local_unix_ms(date, time, observed_at.offset())
}

fn local_unix_ms(date: NaiveDate, time: NaiveTime, offset: &FixedOffset) -> Option<i64> {
    offset
        .from_local_datetime(&date.and_time(time))
        .single()
        .map(|timestamp| timestamp.timestamp_millis())
}

fn parse_rfc3339_ms(raw: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|timestamp| timestamp.timestamp_millis())
}

fn take_token(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start();
    let end = input.find(char::is_whitespace)?;
    Some((&input[..end], input[end..].trim_start()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed(raw: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(raw).expect("test observation instant")
    }

    fn unix_ms(raw: &str) -> Option<i64> {
        Some(
            DateTime::parse_from_rfc3339(raw)
                .expect("test timestamp")
                .timestamp_millis(),
        )
    }

    fn collect(frames: ParsedFrames) -> Vec<LogFrame> {
        frames.into_iter().flatten().collect()
    }

    fn parse_one(kind: CoreKind, stream: LogStream, line: &str, at: &str) -> LogFrame {
        let mut parser = LogParser::new(kind, 7);
        let mut frames = collect(parser.push_at(stream, line.to_owned(), observed(at)));
        frames.extend(parser.finish().into_iter().flatten());
        assert_eq!(frames.len(), 1, "expected exactly one frame for {line:?}");
        frames.remove(0)
    }

    #[test]
    fn strip_ansi_keeps_plain_lines_and_drops_truncated_sequences() {
        let line = "plain line".to_owned();
        let allocation = line.as_ptr();
        let stripped = strip_ansi(line);
        assert_eq!(stripped, "plain line");
        assert_eq!(stripped.as_ptr(), allocation);

        assert_eq!(strip_ansi("prefix\u{1b}[31".to_owned()), "prefix");
        assert_eq!(strip_ansi("\u{1b}[2m配置\u{1b}[0m".to_owned()), "配置");
    }

    #[test]
    fn parses_mihomo_logfmt_including_escapes() {
        let info = parse_one(
            CoreKind::Mihomo,
            LogStream::Stdout,
            r#"time="2026-07-29T00:16:22.646059400+08:00" level=info msg="Mixed(http+socks) proxy listening at: 127.0.0.1:17890""#,
            "2026-07-29T00:16:23+08:00",
        );
        assert_eq!(info.level, LogLevel::Info);
        assert_eq!(info.target, None);
        assert_eq!(
            info.message,
            "Mixed(http+socks) proxy listening at: 127.0.0.1:17890"
        );
        let timestamp = info.timestamp.expect("mihomo prints a timestamp");
        assert!(!timestamp.inferred);
        assert_eq!(
            timestamp.unix_ms,
            unix_ms("2026-07-29T00:16:22.646059400+08:00")
        );

        let fatal = parse_one(
            CoreKind::Mihomo,
            LogStream::Stdout,
            r#"time="2026-07-29T00:17:26.518376100+08:00" level=fatal msg="Parse config error: yaml: line 2: did not find expected node content""#,
            "2026-07-29T00:17:27+08:00",
        );
        assert_eq!(fatal.level, LogLevel::Fatal);
        assert_eq!(
            fatal.message,
            "Parse config error: yaml: line 2: did not find expected node content"
        );

        let escaped = parse_one(
            CoreKind::Mihomo,
            LogStream::Stdout,
            r#"time="2026-07-29T00:17:26+08:00" level=warning msg="say \"hello\" on \\path" request=7"#,
            "2026-07-29T00:17:27+08:00",
        );
        assert_eq!(escaped.level, LogLevel::Warning);
        assert_eq!(escaped.message, r#"say "hello" on \path"#);
        assert_eq!(
            escaped.fields,
            [LogField {
                key: "request".into(),
                value: "7".into(),
            }]
        );
    }

    #[test]
    fn colored_mihomo_layout_degrades_instead_of_being_dropped() {
        let frame = parse_one(
            CoreKind::Mihomo,
            LogStream::Stdout,
            "\u{1b}[36mINFO\u{1b}[0m[2026-07-29T00:16:22.646059400+08:00] proxy listening",
            "2026-07-29T00:16:23+08:00",
        );
        assert_eq!(frame.level, LogLevel::Info);
        assert_eq!(frame.timestamp, None);
        assert_eq!(
            frame.raw,
            "INFO[2026-07-29T00:16:22.646059400+08:00] proxy listening"
        );
        assert_eq!(frame.message, frame.raw);
    }

    #[test]
    fn parses_premium_pretty_print() {
        let mmdb = parse_one(
            CoreKind::ClashPremium,
            LogStream::Stdout,
            "00:16:30 INF [MMDB] can't find DB, start download path=C:/.../Country.mmdb",
            "2026-07-29T00:16:31+08:00",
        );
        assert_eq!(mmdb.level, LogLevel::Info);
        assert_eq!(mmdb.target.as_deref(), Some("MMDB"));
        assert_eq!(
            mmdb.message,
            "can't find DB, start download path=C:/.../Country.mmdb"
        );
        assert!(mmdb.fields.is_empty());
        let timestamp = mmdb.timestamp.expect("premium prints a time of day");
        assert!(timestamp.inferred);
        assert_eq!(timestamp.raw, "00:16:30");
        assert_eq!(timestamp.unix_ms, unix_ms("2026-07-29T00:16:30+08:00"));

        let inbound = parse_one(
            CoreKind::ClashPremium,
            LogStream::Stdout,
            "00:16:33 INF inbound create success inbound=mixed addr=127.0.0.1:17890 network=tcp",
            "2026-07-29T00:16:34+08:00",
        );
        assert_eq!(inbound.target, None);
        assert_eq!(
            inbound.message,
            "inbound create success inbound=mixed addr=127.0.0.1:17890 network=tcp"
        );

        let fatal = parse_one(
            CoreKind::ClashPremium,
            LogStream::Stdout,
            "00:17:26 FTL [Config] parse config failed error=yaml: line 2: did not find expected node content",
            "2026-07-29T00:17:27+08:00",
        );
        assert_eq!(fatal.level, LogLevel::Fatal);
        assert_eq!(fatal.target.as_deref(), Some("Config"));
        assert_eq!(
            fatal.message,
            "parse config failed error=yaml: line 2: did not find expected node content"
        );

        let unknown = parse_one(
            CoreKind::ClashPremium,
            LogStream::Stdout,
            "00:17:26 ??? unlabelled record",
            "2026-07-29T00:17:27+08:00",
        );
        assert_eq!(unknown.level, LogLevel::Info);
        assert_eq!(unknown.message, "unlabelled record");
    }

    #[test]
    fn premium_rolls_over_midnight_but_ignores_small_backward_drift() {
        let mut parser = LogParser::new(CoreKind::ClashPremium, 1);
        let before = collect(parser.push_at(
            LogStream::Stdout,
            "23:59:59 INF before".to_owned(),
            observed("2026-07-29T23:59:59+08:00"),
        ))
        .remove(0);
        let after = collect(parser.push_at(
            LogStream::Stdout,
            "00:00:01 INF after".to_owned(),
            observed("2026-07-30T00:00:01+08:00"),
        ))
        .remove(0);
        assert_eq!(
            before.timestamp.unwrap().unix_ms,
            unix_ms("2026-07-29T23:59:59+08:00")
        );
        assert_eq!(
            after.timestamp.unwrap().unix_ms,
            unix_ms("2026-07-30T00:00:01+08:00")
        );

        let mut parser = LogParser::new(CoreKind::ClashPremium, 1);
        parser.push_at(
            LogStream::Stdout,
            "10:00:00 INF first".to_owned(),
            observed("2026-07-29T10:00:00+08:00"),
        );
        let drifted = collect(parser.push_at(
            LogStream::Stdout,
            "09:59:59 INF drifted".to_owned(),
            observed("2026-07-29T10:00:01+08:00"),
        ))
        .remove(0);
        assert_eq!(
            drifted.timestamp.unwrap().unix_ms,
            unix_ms("2026-07-29T09:59:59+08:00")
        );
    }

    #[test]
    fn premium_first_line_from_the_previous_day() {
        let frame = parse_one(
            CoreKind::ClashPremium,
            LogStream::Stdout,
            "23:59:59 INF late",
            "2026-07-30T00:00:02+08:00",
        );
        assert_eq!(
            frame.timestamp.unwrap().unix_ms,
            unix_ms("2026-07-29T23:59:59+08:00")
        );
    }

    #[test]
    fn strips_ansi_from_meow_before_parsing() {
        let info = parse_one(
            CoreKind::Meow,
            LogStream::Stdout,
            "\u{1b}[2m2026-07-28T16:16:26.616489Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m \u{1b}[2mmeow\u{1b}[0m\u{1b}[2m:\u{1b}[0m meow-rs starting...",
            "2026-07-29T00:16:27+08:00",
        );
        assert_eq!(info.level, LogLevel::Info);
        assert_eq!(info.target.as_deref(), Some("meow"));
        assert_eq!(info.message, "meow-rs starting...");
        assert_eq!(
            info.raw,
            "2026-07-28T16:16:26.616489Z  INFO meow: meow-rs starting..."
        );
        assert_eq!(
            info.timestamp.unwrap().unix_ms,
            unix_ms("2026-07-28T16:16:26.616489Z")
        );

        let error = parse_one(
            CoreKind::Meow,
            LogStream::Stdout,
            "\u{1b}[2m2026-07-28T16:17:26.719459Z\u{1b}[0m \u{1b}[31mERROR\u{1b}[0m \u{1b}[2mmeow\u{1b}[0m\u{1b}[2m:\u{1b}[0m meow-rs stopped with an error \u{1b}[3merror\u{1b}[0m\u{1b}[2m=\u{1b}[0mdid not find expected node content at line 3 column 1",
            "2026-07-29T00:17:27+08:00",
        );
        assert_eq!(error.level, LogLevel::Error);
        assert_eq!(error.target.as_deref(), Some("meow"));
        assert_eq!(
            error.message,
            "meow-rs stopped with an error error=did not find expected node content at line 3 column 1"
        );
        assert!(!error.raw.contains('\u{1b}'));
    }

    #[test]
    fn infers_levels_for_plain_stderr_text() {
        let warning = parse_one(
            CoreKind::Meow,
            LogStream::Stderr,
            "warning: --geodata-mode is not supported and will be ignored",
            "2026-07-29T00:17:27+08:00",
        );
        assert_eq!(warning.level, LogLevel::Warning);
        assert_eq!(warning.timestamp, None);

        let error = parse_one(
            CoreKind::ClashRust,
            LogStream::Stderr,
            "Error: invalid config",
            "2026-07-29T00:17:27+08:00",
        );
        assert_eq!(error.level, LogLevel::Error);

        let noise = parse_one(
            CoreKind::ClashRust,
            LogStream::Stderr,
            "using env log level: debug",
            "2026-07-29T00:17:27+08:00",
        );
        assert_eq!(noise.level, LogLevel::Warning);
    }

    #[test]
    fn parses_clash_rs_release_and_debug_layouts() {
        let debug = parse_one(
            CoreKind::ClashRust,
            LogStream::Stdout,
            r"26-07-29 00:16:35:0919421 DEBUG clash-lib\src\lib.rs:445: initializing cache store",
            "2026-07-29T00:16:36+08:00",
        );
        assert_eq!(debug.level, LogLevel::Debug);
        assert_eq!(debug.target.as_deref(), Some(r"clash-lib\src\lib.rs:445"));
        assert_eq!(debug.message, "initializing cache store");
        let timestamp = debug.timestamp.expect("clash-rs prints a timestamp");
        assert!(timestamp.inferred);
        assert_eq!(timestamp.raw, "26-07-29 00:16:35:0919421");
        assert_eq!(
            timestamp.unix_ms,
            unix_ms("2026-07-29T00:16:35.0919421+08:00")
        );

        let warning = parse_one(
            CoreKind::ClashRust,
            LogStream::Stdout,
            r"26-07-29 00:16:35:0926205  WARN clash-lib\src\app\profile\mod.rs:153: failed to read cache file: stream did not contain valid UTF-8",
            "2026-07-29T00:16:36+08:00",
        );
        assert_eq!(warning.level, LogLevel::Warning);
        assert_eq!(
            warning.target.as_deref(),
            Some(r"clash-lib\src\app\profile\mod.rs:153")
        );
        assert_eq!(
            warning.message,
            "failed to read cache file: stream did not contain valid UTF-8"
        );

        let six_digits = parse_one(
            CoreKind::ClashRust,
            LogStream::Stdout,
            r"26-07-29 00:16:35:093078 INFO clash-lib\src\lib.rs:446: six digit subsecond",
            "2026-07-29T00:16:36+08:00",
        );
        assert_eq!(
            six_digits.timestamp.unwrap().unix_ms,
            unix_ms("2026-07-29T00:16:35.093078+08:00")
        );

        let instrumented = parse_one(
            CoreKind::ClashRust,
            LogStream::Stdout,
            r"26-07-29 00:16:35:093078 DEBUG ThreadId(1) clash_lib::app clash-lib\src\lib.rs:445: debug build shape",
            "2026-07-29T00:16:36+08:00",
        );
        assert_eq!(
            instrumented.target.as_deref(),
            Some(r"clash-lib\src\lib.rs:445")
        );
        assert_eq!(instrumented.message, "debug build shape");

        // A `file:line`-shaped reference inside the message must not win over the
        // real source anchor that closes the header.
        let quoted_source = parse_one(
            CoreKind::ClashRust,
            LogStream::Stdout,
            r"26-07-29 00:16:35:093078 ERROR clash-lib\src\lib.rs:445: failed at config.yaml:3: invalid value",
            "2026-07-29T00:16:36+08:00",
        );
        assert_eq!(
            quoted_source.target.as_deref(),
            Some(r"clash-lib\src\lib.rs:445")
        );
        assert_eq!(
            quoted_source.message,
            "failed at config.yaml:3: invalid value"
        );
    }

    #[test]
    fn clash_rs_header_shape_alone_does_not_make_a_root() {
        let mut parser = LogParser::new(CoreKind::ClashRust, 1);
        let at = observed("2026-07-29T00:17:27+08:00");
        parser.push_at(LogStream::Stderr, "Error: invalid config".to_owned(), at);
        for line in [
            // Separators line up but the numeric slots do not hold digits.
            "xx-xx-xx xx:xx:xx:1 ERROR fake.rs:1: not a header",
            // No space between the subsecond and the level.
            "26-07-29 00:16:35:1ERROR fake.rs:1: no separator",
        ] {
            assert!(collect(parser.push_at(LogStream::Stderr, line.to_owned(), at)).is_empty());
        }
        let frame = collect(parser.finish()).remove(0);
        assert_eq!(frame.level, LogLevel::Error);
        assert_eq!(
            frame.message,
            "Error: invalid config\n\
             xx-xx-xx xx:xx:xx:1 ERROR fake.rs:1: not a header\n\
             26-07-29 00:16:35:1ERROR fake.rs:1: no separator"
        );
    }

    #[test]
    fn finish_releases_pending_roots_oldest_first() {
        let mut parser = LogParser::new(CoreKind::Meow, 1);
        let at = observed("2026-07-29T00:17:27+08:00");
        parser.push_at(LogStream::Stderr, "Error: stderr first".to_owned(), at);
        parser.push_at(
            LogStream::Stdout,
            "2026-07-28T16:17:26.719459Z ERROR meow: stdout second".to_owned(),
            at,
        );
        let frames = collect(parser.finish());
        assert_eq!(frames[0].message, "Error: stderr first");
        assert_eq!(frames[1].message, "stdout second");
        assert_eq!(error_summary(&frames).as_deref(), Some("stdout second"));
    }

    #[test]
    fn aggregates_multi_line_records_within_one_stream() {
        let mut parser = LogParser::new(CoreKind::ClashRust, 1);
        let at = observed("2026-07-29T00:17:27+08:00");
        for line in [
            "Error: invalid config: couldn't not parse config content mixed-port: not-a-port",
            "proxies: [[[",
            ": did not find expected node content at line 3 column 1, while parsing a flow node",
        ] {
            assert!(collect(parser.push_at(LogStream::Stderr, line.to_owned(), at)).is_empty());
        }
        let frame = collect(parser.finish()).remove(0);
        assert_eq!(frame.level, LogLevel::Error);
        assert!(!frame.truncated);
        assert_eq!(
            frame.message,
            "Error: invalid config: couldn't not parse config content mixed-port: not-a-port\nproxies: [[[\n: did not find expected node content at line 3 column 1, while parsing a flow node"
        );
    }

    #[test]
    fn premium_stack_stays_attached_to_its_record() {
        let mut parser = LogParser::new(CoreKind::ClashPremium, 1);
        let at = observed("2026-07-29T00:17:27+08:00");
        for line in [
            "00:17:26 FTL [Config] parse config failed error=bad",
            "goroutine 1 [running]:",
            "main.main()",
        ] {
            parser.push_at(LogStream::Stdout, line.to_owned(), at);
        }
        let frame = collect(parser.finish()).remove(0);
        assert_eq!(
            frame.message,
            "parse config failed error=bad\ngoroutine 1 [running]:\nmain.main()"
        );
    }

    #[test]
    fn oversized_stacks_keep_the_root_and_mark_truncation() {
        let mut parser = LogParser::new(CoreKind::Mihomo, 1);
        let at = observed("2026-07-29T00:17:27+08:00");
        parser.push_at(
            LogStream::Stdout,
            r#"time="2026-07-29T00:17:26+08:00" level=panic msg="boom""#.to_owned(),
            at,
        );
        for index in 0..MAX_CONTINUATION_LINES + 4 {
            parser.push_at(LogStream::Stdout, format!("stack frame {index}"), at);
        }
        let frame = collect(parser.finish()).remove(0);
        assert_eq!(frame.level, LogLevel::Fatal);
        assert!(frame.truncated);
        assert!(frame.message.starts_with("boom\nstack frame 0\n"));
        assert!(frame.message.contains("stack frame 15"));
        assert!(!frame.message.contains("stack frame 16"));
    }

    #[test]
    fn continuations_never_cross_streams() {
        let mut parser = LogParser::new(CoreKind::ClashRust, 1);
        let at = observed("2026-07-29T00:17:27+08:00");
        parser.push_at(LogStream::Stderr, "Error: invalid config".to_owned(), at);
        let stdout =
            collect(parser.push_at(LogStream::Stdout, "unrelated stdout noise".to_owned(), at));
        assert_eq!(stdout.len(), 1);
        assert_eq!(stdout[0].message, "unrelated stdout noise");
        let pending = collect(parser.finish()).remove(0);
        assert_eq!(pending.message, "Error: invalid config");
    }

    #[test]
    fn summarizes_bad_config_output_for_every_kind() {
        assert_eq!(
            summarize_output(
                CoreKind::Mihomo,
                r#"time="2026-07-29T00:17:26.518376100+08:00" level=fatal msg="Parse config error: yaml: line 2: did not find expected node content""#,
                "",
            ),
            "Parse config error: yaml: line 2: did not find expected node content"
        );

        assert_eq!(
            summarize_output(
                CoreKind::ClashPremium,
                "00:16:30 INF [MMDB] can't find DB\n00:17:26 FTL [Config] parse config failed error=yaml: line 2: did not find expected node content",
                "",
            ),
            "parse config failed error=yaml: line 2: did not find expected node content"
        );

        let meow = summarize_output(
            CoreKind::Meow,
            "\u{1b}[2m2026-07-28T16:17:26.719459Z\u{1b}[0m \u{1b}[31mERROR\u{1b}[0m \u{1b}[2mmeow\u{1b}[0m\u{1b}[2m:\u{1b}[0m meow-rs stopped with an error \u{1b}[3merror\u{1b}[0m\u{1b}[2m=\u{1b}[0mdid not find expected node content at line 3 column 1",
            "warning: --geodata-mode is not supported and will be ignored\nError: did not find expected node content at line 3 column 1, while parsing a flow node",
        );
        assert!(
            meow.contains("did not find expected node content"),
            "{meow}"
        );

        let clash_rs = summarize_output(
            CoreKind::ClashRust,
            "",
            "Error: invalid config: couldn't not parse config content mixed-port: not-a-port\nproxies: [[[\n: did not find expected node content at line 3 column 1, while parsing a flow node",
        );
        assert!(clash_rs.contains("invalid config"), "{clash_rs}");
        assert!(clash_rs.contains("proxies: [[["), "{clash_rs}");
    }

    #[test]
    fn summary_falls_back_to_verbatim_output() {
        assert_eq!(
            summarize_output(CoreKind::Mihomo, "  ", ""),
            "core reported no output"
        );
        assert_eq!(
            summarize_output(CoreKind::Mihomo, "unstructured note", ""),
            "unstructured note"
        );
    }
}
