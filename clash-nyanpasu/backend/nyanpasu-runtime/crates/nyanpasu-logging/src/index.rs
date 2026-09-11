use std::collections::VecDeque;

use gxhash::{GxBuildHasher, HashMap};
use lasso::{Rodeo, Spur};
use serde::{Deserialize, Serialize};

pub const MAX_WINDOW: u64 = 32 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 250_000;
pub const MAX_LINE: usize = 256 * 1024;
pub const MAX_BATCH: usize = 1024 * 1024;
pub const MAX_TARGET_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PAGE: usize = 200;
pub const MAX_SCAN: usize = 2_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    #[default]
    Unknown,
}
impl Level {
    fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warn" | "warning" => Self::Warn,
            "error" => Self::Error,
            "fatal" => Self::Fatal,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Filter {
    pub levels: Vec<Level>,
    pub target: Option<String>,
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
    pub text: Option<String>,
}
impl Filter {
    pub fn valid(&self) -> bool {
        self.levels.len() <= 7
            && self.target.as_ref().is_none_or(|s| s.len() <= 4096)
            && self.text.as_ref().is_none_or(|s| s.chars().count() <= 256)
            && !matches!((self.from_ms, self.to_ms), (Some(a), Some(b)) if a > b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Latest,
    Before,
    After,
}

/// Internal IDs are physical offsets, never timestamps or interned symbols.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub start: u64,
    pub end: u64,
    pub timestamp: Option<i64>,
    pub level: Level,
    target: Spur,
    pub unparsed: bool,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct Scan {
    pub entries: Vec<Entry>,
    /// Exclusive physical boundary for the next Before/After request.
    pub boundary: u64,
    pub more: bool,
}

/// Owned by one actor. The pool is rebuilt with retained entries after eviction.
pub struct Index {
    entries: VecDeque<Entry>,
    targets: Rodeo<Spur, GxBuildHasher>,
    postings: HashMap<Spur, Vec<u64>>,
    levels: [Vec<u64>; 7],
    target_bytes: usize,
    pending: Vec<u8>,
    line_start: u64,
    next_read: u64,
    committed: u64,
    skipping: bool,
    pub malformed: u64,
    pub truncated: u64,
    pub partial: bool,
}

impl Index {
    /// If a file window starts inside a record, discard its leading fragment.
    pub fn new(start: u64) -> Self {
        Self {
            entries: VecDeque::new(),
            targets: Rodeo::with_hasher(Default::default()),
            postings: HashMap::default(),
            levels: Default::default(),
            target_bytes: 0,
            pending: Vec::new(),
            line_start: start,
            next_read: start,
            committed: start,
            skipping: start != 0,
            malformed: 0,
            truncated: 0,
            partial: start != 0,
        }
    }
    pub fn next_read(&self) -> u64 {
        self.next_read
    }
    pub fn committed(&self) -> u64 {
        self.committed
    }
    pub fn start(&self) -> u64 {
        self.entries.front().map_or(self.committed, |e| e.start)
    }
    pub fn target(&self, entry: &Entry) -> &str {
        self.targets.resolve(&entry.target)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn append(&mut self, offset: u64, bytes: &[u8]) -> Result<(), &'static str> {
        if offset != self.next_read || bytes.len() > MAX_BATCH {
            return Err("noncontiguous or oversized batch");
        }
        for &byte in bytes {
            self.next_read += 1;
            if byte == b'\n' {
                if !self.skipping {
                    let line = std::mem::take(&mut self.pending);
                    self.insert(&line);
                    self.pending = line;
                }
                self.pending.clear();
                self.skipping = false;
                self.committed = self.next_read;
                self.line_start = self.next_read;
            } else if !self.skipping {
                if self.pending.len() == MAX_LINE {
                    self.pending.clear();
                    self.skipping = true;
                    self.truncated += 1;
                    self.partial = true;
                } else {
                    self.pending.push(byte);
                }
            }
        }
        self.evict();
        Ok(())
    }

    fn insert(&mut self, line: &[u8]) {
        let parsed = serde_json::from_slice::<serde_json::Value>(line).ok();
        let object = parsed.as_ref().filter(|v| v.is_object());
        let timestamp = object
            .and_then(|v| v["timestamp"].as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.timestamp_millis());
        let level = object
            .and_then(|v| v["level"].as_str())
            .map(Level::parse)
            .unwrap_or_default();
        let raw_target = object.and_then(|v| v["target"].as_str()).unwrap_or("");
        let mut truncated = raw_target.len() > 4096;
        let mut target = if truncated { "" } else { raw_target };
        if self.targets.get(target).is_none() && self.target_bytes + target.len() > MAX_TARGET_BYTES
        {
            self.evict();
            if self.target_bytes + target.len() > MAX_TARGET_BYTES {
                target = "";
                truncated = true;
            }
        }
        let key = self.targets.get(target).unwrap_or_else(|| {
            self.target_bytes += target.len();
            self.targets.get_or_intern(target)
        });
        let unparsed = object.is_none();
        self.malformed += u64::from(unparsed);
        self.truncated += u64::from(truncated);
        self.entries.push_back(Entry {
            start: self.line_start,
            end: self.next_read,
            timestamp,
            level,
            target: key,
            unparsed,
            truncated,
        });
        self.postings.entry(key).or_default().push(self.line_start);
        self.levels[level as usize].push(self.line_start);
    }

    fn evict(&mut self) {
        let floor = self.next_read.saturating_sub(MAX_WINDOW);
        let mut removed = false;
        while self.entries.len() > MAX_ENTRIES
            || self.entries.front().is_some_and(|e| e.start < floor)
        {
            self.entries.pop_front();
            removed = true;
        }
        if removed {
            self.partial = true;
            let mut pool = Rodeo::with_hasher(GxBuildHasher::default());
            let mut postings: HashMap<Spur, Vec<u64>> = HashMap::default();
            let mut bytes = 0;
            self.levels = Default::default();
            for entry in &mut self.entries {
                let name = self.targets.resolve(&entry.target);
                let key = pool.get(name).unwrap_or_else(|| {
                    bytes += name.len();
                    pool.get_or_intern(name)
                });
                entry.target = key;
                postings.entry(key).or_default().push(entry.start);
                self.levels[entry.level as usize].push(entry.start);
            }
            self.targets = pool;
            self.postings = postings;
            self.target_bytes = bytes;
        }
    }

    /// Scan bounded candidates in physical order. Text matching happens after bounded file reads.
    pub fn scan(
        &self,
        filter: &Filter,
        direction: Direction,
        boundary: Option<u64>,
        limit: usize,
    ) -> Scan {
        let reverse = direction != Direction::After;
        let boundary = boundary.unwrap_or(if reverse {
            self.committed
        } else {
            self.start()
        });
        let mut result = Scan {
            entries: Vec::new(),
            boundary,
            more: false,
        };
        let target_key = filter.target.as_ref().and_then(|t| self.targets.get(t));
        if filter.target.is_some() && target_key.is_none() {
            result.boundary = if reverse {
                self.start()
            } else {
                self.committed
            };
            return result;
        }
        let by_target = target_key
            .and_then(|key| self.postings.get(&key))
            .map(Vec::as_slice);
        let by_level =
            (filter.levels.len() == 1).then(|| self.levels[filter.levels[0] as usize].as_slice());
        let posting = match (by_target, by_level) {
            (Some(a), Some(b)) => Some(if a.len() < b.len() { a } else { b }),
            (a, b) => a.or(b),
        };
        let positions: Box<dyn Iterator<Item = usize> + '_> = if let Some(posting) = posting {
            let split = posting.partition_point(|p| *p < boundary);
            let ids: Box<dyn Iterator<Item = &u64>> = if reverse {
                Box::new(posting[..split].iter().rev())
            } else {
                Box::new(posting[split..].iter())
            };
            Box::new(ids.map(|id| {
                self.entries
                    .binary_search_by_key(id, |e| e.start)
                    .expect("retained posting")
            }))
        } else {
            let split = self.entries.partition_point(|e| e.start < boundary);
            if reverse {
                Box::new((0..split).rev())
            } else {
                Box::new(split..self.entries.len())
            }
        };
        for (scanned, position) in positions.enumerate() {
            if scanned >= MAX_SCAN || result.entries.len() >= limit.clamp(1, MAX_PAGE) {
                result.more = true;
                break;
            }
            let entry = self.entries[position];
            result.boundary = if reverse { entry.start } else { entry.end };
            if !filter.levels.is_empty() && !filter.levels.contains(&entry.level) {
                continue;
            }
            if target_key.is_some_and(|key| key != entry.target) {
                continue;
            }
            if filter
                .from_ms
                .is_some_and(|from| entry.timestamp.is_none_or(|t| t < from))
            {
                continue;
            }
            if filter
                .to_ms
                .is_some_and(|to| entry.timestamp.is_none_or(|t| t >= to))
            {
                continue;
            }
            result.entries.push(entry);
        }
        if !result.more {
            result.boundary = if reverse {
                self.start()
            } else {
                self.committed
            };
        }
        if reverse {
            result.entries.reverse();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn line(time: &str, level: &str, target: &str) -> String {
        format!(
            "{}\n",
            serde_json::json!({"timestamp":time,"level":level,"target":target,"fields":{"message":"中文 \\ \" escaped"}})
        )
    }
    #[test]
    fn duplicate_and_backwards_timestamps_are_exact() {
        let mut index = Index::new(0);
        let rows = [
            "2026-01-01T00:00:02Z",
            "2026-01-01T00:00:01Z",
            "2026-01-01T00:00:02Z",
        ];
        for t in rows {
            index
                .append(index.next_read(), line(t, "TRACE", "mod").as_bytes())
                .unwrap();
        }
        let t = chrono::DateTime::parse_from_rfc3339(rows[0])
            .unwrap()
            .timestamp_millis();
        let scan = index.scan(
            &Filter {
                from_ms: Some(t),
                to_ms: Some(t + 1),
                ..Default::default()
            },
            Direction::Latest,
            None,
            200,
        );
        assert_eq!(scan.entries.len(), 2);
        assert!(scan.entries.iter().all(|e| e.level == Level::Trace));
        assert!(scan.entries[0].start < scan.entries[1].start);
    }
    #[test]
    fn partial_utf8_and_bad_lines_do_not_reset_offsets() {
        let mut index = Index::new(0);
        let row = line("2026-01-01T00:00:00Z", "WARNING", "目标");
        for byte in row.as_bytes() {
            index.append(index.next_read(), &[*byte]).unwrap();
        }
        assert_eq!(index.len(), 1);
        assert_eq!(index.committed(), row.len() as u64);
        index
            .append(index.next_read(), b"invalid\n{\"level\":")
            .unwrap();
        assert_eq!(index.len(), 2);
        assert_eq!(index.malformed, 1);
        let offset = index.committed();
        index.append(index.next_read(), b"\"INFO\"}\n").unwrap();
        assert_eq!(index.len(), 3);
        assert!(index.committed() > offset);
        assert_eq!(index.entries[0].level, Level::Warn);
    }
    #[test]
    fn no_match_advances_and_pages_do_not_overlap() {
        let mut index = Index::new(0);
        for _ in 0..(MAX_SCAN + 5) {
            index.append(index.next_read(), b"{}\n").unwrap();
        }
        let filter = Filter {
            from_ms: Some(0),
            ..Default::default()
        };
        let page = index.scan(&filter, Direction::After, Some(0), 200);
        assert!(page.entries.is_empty());
        assert!(page.more);
        assert!(page.boundary > 0);
        let last = index.scan(&filter, Direction::After, Some(page.boundary), 200);
        assert!(!last.more);
        assert_eq!(last.boundary, index.committed());
        let latest = index.scan(&Filter::default(), Direction::Latest, None, 2);
        let older = index.scan(
            &Filter::default(),
            Direction::Before,
            Some(latest.boundary),
            2,
        );
        assert!(older.entries.last().unwrap().start < latest.entries[0].start);
    }
    #[test]
    fn oversized_record_recovers_and_bounds_memory() {
        let mut index = Index::new(0);
        index.append(0, &vec![b'x'; MAX_LINE + 1]).unwrap();
        index.append(index.next_read(), b"\n{}\n").unwrap();
        assert_eq!(index.len(), 1);
        assert_eq!(index.truncated, 1);
        assert!(index.pending.len() <= MAX_LINE);
        assert!(index.append(0, b"oops").is_err());
    }
    #[test]
    fn filtered_pagination_matches_sequential_oracle() {
        let mut index = Index::new(0);
        for i in 0..1000 {
            let row = line(
                &format!("2026-01-01T00:00:{:02}Z", (i * 17) % 60),
                if i % 3 == 0 { "ERROR" } else { "INFO" },
                if i % 7 == 0 { "rare" } else { "common" },
            );
            index.append(index.next_read(), row.as_bytes()).unwrap();
        }
        for level in [vec![], vec![Level::Error], vec![Level::Error, Level::Info]] {
            for target in [None, Some("rare".to_string()), Some("absent".to_string())] {
                let filter = Filter {
                    levels: level.clone(),
                    target: target.clone(),
                    ..Default::default()
                };
                let expected: Vec<_> = index
                    .entries
                    .iter()
                    .filter(|e| {
                        (level.is_empty() || level.contains(&e.level))
                            && target.as_ref().is_none_or(|s| index.target(e) == s)
                    })
                    .map(|e| e.start)
                    .collect();
                let mut actual = Vec::new();
                let mut cursor = 0;
                loop {
                    let page = index.scan(&filter, Direction::After, Some(cursor), 17);
                    actual.extend(page.entries.iter().map(|e| e.start));
                    cursor = page.boundary;
                    if !page.more {
                        break;
                    }
                }
                assert_eq!(actual, expected);
            }
        }
    }
    #[test]
    fn send_owned_state_and_target_pool_reclamation() {
        fn send<T: Send + 'static>() {}
        send::<Index>();
        let mut index = Index::new(0);
        index
            .append(0, line("2026-01-01T00:00:00Z", "INFO", "old").as_bytes())
            .unwrap();
        index.next_read = MAX_WINDOW + 1000;
        index.evict();
        assert!(index.is_empty());
        assert_eq!(index.target_bytes, 0);
        assert!(index.targets.is_empty());
    }
}
