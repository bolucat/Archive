use crate::*;
use gxhash::HashMap;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort, rpc::CallResult};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const LEASE: u64 = 45_000;
const GRACE: u64 = 30_000;
pub trait Clock: Send + Sync + 'static {
    fn millis(&self) -> u64;
}
pub struct MonotonicClock(Instant);
impl Default for MonotonicClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl Clock for MonotonicClock {
    fn millis(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

#[derive(Clone)]
pub struct LogsClient(Arc<Inner>);
struct Inner {
    actor: ActorRef<Message>,
    files: Arc<dyn LogFiles>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.actor.stop(None);
    }
}
impl LogsClient {
    pub async fn start(files: Arc<dyn LogFiles>, clock: Arc<dyn Clock>) -> LogResult<Self> {
        let (actor, _) = Actor::spawn(None, LogsActor, (files.clone(), clock))
            .await
            .map_err(|_| LogError::Unavailable)?;
        Ok(Self(Arc::new(Inner { actor, files })))
    }
    async fn call<T: Send + 'static>(
        &self,
        msg: impl FnOnce(RpcReplyPort<LogResult<T>>) -> Message,
    ) -> LogResult<T> {
        match self
            .0
            .actor
            .call(msg, Some(Duration::from_secs(5)))
            .await
            .map_err(|_| LogError::Unavailable)?
        {
            CallResult::Success(value) => value,
            _ => Err(LogError::Unavailable),
        }
    }
    pub async fn catalog(&self) -> LogResult<Vec<LogFileInfo>> {
        let files = self.0.files.clone();
        tokio::task::spawn_blocking(move || files.catalog())
            .await
            .map_err(|_| LogError::Unavailable)?
    }
    pub async fn open(&self, owner: String, request: OpenLogs) -> LogResult<LogSession> {
        if owner.is_empty()
            || owner.len() > 128
            || request.request_id.is_empty()
            || request.request_id.len() > 128
        {
            return Err(LogError::InvalidRequest);
        }
        if let Some(file) = &request.file
            && !self.catalog().await?.iter().any(|f| &f.id == file)
        {
            return Err(LogError::FileGone);
        }
        self.call(|reply| Message::Open(owner, request, reply))
            .await
    }
    pub async fn query(&self, owner: String, request: QueryLogs) -> LogResult<LogPage> {
        if !request.filter.valid() || request.limit == 0 || request.limit > MAX_PAGE as u32 {
            return Err(LogError::InvalidRequest);
        }
        self.call(|reply| Message::Query(owner, request, reply))
            .await
    }
    pub async fn close(&self, owner: String, session: String) -> LogResult<()> {
        self.call(|reply| Message::Close(owner, session, reply))
            .await
    }
    pub async fn shutdown(&self) -> LogResult<()> {
        self.0
            .actor
            .stop_and_wait(None, Some(Duration::from_secs(5)))
            .await
            .map_err(|_| LogError::Unavailable)
    }
}
struct Session {
    owner: String,
    request: String,
    file: String,
    expires: u64,
}
struct Work {
    index: Index,
    identity: String,
    name: String,
    prefix: Vec<u8>,
    generation: String,
}
impl Work {
    fn new() -> Self {
        Self {
            index: Index::new(0),
            identity: String::new(),
            name: String::new(),
            prefix: vec![],
            generation: uuid::Uuid::new_v4().to_string(),
        }
    }
}
struct Slot {
    work: Option<Work>,
    task: Option<tokio::task::JoinHandle<()>>,
    page: LogPage,
    idle: Option<u64>,
}
fn empty_page() -> LogPage {
    let cursor = LogCursor {
        generation: String::new(),
        offset: "0".into(),
    };
    LogPage {
        rows: vec![],
        cursor: cursor.clone(),
        head: cursor,
        start: "0".into(),
        file: String::new(),
        building: true,
        more: false,
        partial: false,
        malformed: "0".into(),
        truncated: "0".into(),
        indexed_bytes: "0".into(),
        file_bytes: "0".into(),
    }
}
struct State {
    files: Arc<dyn LogFiles>,
    clock: Arc<dyn Clock>,
    sessions: HashMap<String, Session>,
    slots: HashMap<String, Slot>,
    timer: Option<tokio::task::JoinHandle<Result<(), ractor::MessagingErr<Message>>>>,
}
enum Message {
    Open(String, OpenLogs, RpcReplyPort<LogResult<LogSession>>),
    Query(String, QueryLogs, RpcReplyPort<LogResult<LogPage>>),
    Close(String, String, RpcReplyPort<LogResult<()>>),
    Done(
        String,
        Box<LogResult<(Work, LogResult<LogPage>)>>,
        Option<RpcReplyPort<LogResult<LogPage>>>,
    ),
    Tick,
    #[cfg(test)]
    Inspect(RpcReplyPort<LogResult<(usize, usize)>>),
}
struct LogsActor;
impl State {
    fn reap(&mut self) {
        let now = self.clock.millis();
        let mut expired = HashMap::<String, u64>::default();
        self.sessions.retain(|_, session| {
            if session.expires <= now {
                expired
                    .entry(session.file.clone())
                    .and_modify(|e| *e = (*e).max(session.expires))
                    .or_insert(session.expires);
                false
            } else {
                true
            }
        });
        for (file, slot) in &mut self.slots {
            if self.sessions.values().any(|s| &s.file == file) {
                slot.idle = None;
            } else if slot.idle.is_none() {
                slot.idle = Some(expired.get(file).copied().unwrap_or(now));
            }
        }
        self.slots
            .retain(|_, slot| slot.task.is_some() || slot.idle.is_none_or(|at| now < at + GRACE));
    }
    fn schedule(&mut self, actor: &ActorRef<Message>) {
        if let Some(timer) = self.timer.take() {
            timer.abort();
        }
        let next = self
            .sessions
            .values()
            .map(|s| s.expires)
            .chain(
                self.slots
                    .values()
                    .filter(|s| s.task.is_none())
                    .filter_map(|s| s.idle.map(|t| t + GRACE)),
            )
            .min();
        if let Some(at) = next {
            self.timer = Some(actor.send_after(
                Duration::from_millis(at.saturating_sub(self.clock.millis()).max(1)),
                || Message::Tick,
            ));
        }
    }
    fn launch(
        &mut self,
        actor: &ActorRef<Message>,
        file: String,
        query: QueryLogs,
        reply: Option<RpcReplyPort<LogResult<LogPage>>>,
    ) {
        let slot = self.slots.get_mut(&file).expect("session slot");
        let Some(work) = slot.work.take() else {
            if let Some(reply) = reply {
                let mut page = slot.page.clone();
                page.rows.clear();
                page.building = true;
                let _ = reply.send(Ok(page));
            }
            return;
        };
        let files = self.files.clone();
        let actor = actor.clone();
        let key = file.clone();
        slot.task = Some(tokio::spawn(async move {
            let result =
                tokio::task::spawn_blocking(move || refresh(files.as_ref(), &key, work, &query))
                    .await
                    .unwrap_or(Err(LogError::Unavailable));
            let _ = actor.cast(Message::Done(file, Box::new(result), reply));
        }));
    }
}
impl Actor for LogsActor {
    type Msg = Message;
    type State = State;
    type Arguments = (Arc<dyn LogFiles>, Arc<dyn Clock>);
    async fn pre_start(
        &self,
        _: ActorRef<Message>,
        (files, clock): Self::Arguments,
    ) -> Result<State, ActorProcessingErr> {
        Ok(State {
            files,
            clock,
            sessions: HashMap::default(),
            slots: HashMap::default(),
            timer: None,
        })
    }
    async fn handle(
        &self,
        actor: ActorRef<Message>,
        message: Message,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        state.reap();
        let now = state.clock.millis();
        match message {
            Message::Open(owner, request, reply) => {
                let file = request.file.unwrap_or_default();
                if let Some((id, session)) = state
                    .sessions
                    .iter_mut()
                    .find(|(_, s)| s.owner == owner && s.request == request.request_id)
                {
                    let result = if session.file != file {
                        Err(LogError::InvalidRequest)
                    } else {
                        session.expires = now + LEASE;
                        Ok(LogSession {
                            id: id.clone(),
                            lease_ms: LEASE as u32,
                        })
                    };
                    let _ = reply.send(result);
                } else if state.sessions.len() >= 16
                    || (!state.slots.contains_key(&file) && state.slots.len() >= 4)
                {
                    let _ = reply.send(Err(LogError::Limit));
                } else {
                    let id = uuid::Uuid::new_v4().to_string();
                    state.sessions.insert(
                        id.clone(),
                        Session {
                            owner,
                            request: request.request_id,
                            file: file.clone(),
                            expires: now + LEASE,
                        },
                    );
                    let slot = state.slots.entry(file.clone()).or_insert_with(|| Slot {
                        work: Some(Work::new()),
                        task: None,
                        page: empty_page(),
                        idle: None,
                    });
                    slot.idle = None;
                    let _ = reply.send(Ok(LogSession {
                        id: id.clone(),
                        lease_ms: LEASE as u32,
                    }));
                    if slot.task.is_none() {
                        state.launch(
                            &actor,
                            file,
                            QueryLogs {
                                session: id,
                                filter: Filter::default(),
                                direction: Direction::Latest,
                                cursor: None,
                                limit: 200,
                            },
                            None,
                        );
                    }
                }
            }
            Message::Query(owner, request, reply) => {
                if let Some(session) = state
                    .sessions
                    .get_mut(&request.session)
                    .filter(|s| s.owner == owner)
                {
                    session.expires = now + LEASE;
                    let file = session.file.clone();
                    state.launch(&actor, file, request, Some(reply));
                } else {
                    let _ = reply.send(Err(LogError::SessionExpired));
                }
            }
            Message::Close(owner, id, reply) => {
                if state.sessions.get(&id).is_some_and(|s| s.owner != owner) {
                    let _ = reply.send(Err(LogError::SessionExpired));
                } else {
                    state.sessions.remove(&id);
                    state.reap();
                    let _ = reply.send(Ok(()));
                }
            }
            Message::Done(file, result, reply) => {
                if let Some(slot) = state.slots.get_mut(&file) {
                    slot.task = None;
                    match *result {
                        Ok((work, page)) => {
                            slot.work = Some(work);
                            let page = match page {
                                Ok(page) => page,
                                Err(error) => {
                                    if let Some(reply) = reply {
                                        let _ = reply.send(Err(error));
                                    }
                                    state.schedule(&actor);
                                    return Ok(());
                                }
                            };
                            slot.page = page.clone();
                            slot.page.rows.clear();
                            if let Some(reply) = reply {
                                let _ = reply.send(Ok(page.clone()));
                            }
                            if page.building && state.sessions.values().any(|s| s.file == file) {
                                state.launch(
                                    &actor,
                                    file,
                                    QueryLogs {
                                        session: String::new(),
                                        filter: Filter::default(),
                                        direction: Direction::Latest,
                                        cursor: None,
                                        limit: 200,
                                    },
                                    None,
                                );
                            }
                        }
                        Err(error) => {
                            slot.work = Some(Work::new());
                            slot.page = empty_page();
                            if let Some(reply) = reply {
                                let _ = reply.send(Err(error));
                            }
                        }
                    }
                }
                state.reap();
            }
            Message::Tick => {}
            #[cfg(test)]
            Message::Inspect(reply) => {
                let _ = reply.send(Ok((state.sessions.len(), state.slots.len())));
            }
        }
        state.schedule(&actor);
        Ok(())
    }
    async fn post_stop(
        &self,
        _: ActorRef<Message>,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        if let Some(timer) = state.timer.take() {
            timer.abort();
        }
        for slot in state.slots.values_mut() {
            if let Some(task) = slot.task.take() {
                let _ = task.await;
            }
        }
        state.slots.clear();
        state.sessions.clear();
        Ok(())
    }
}

fn refresh(
    files: &dyn LogFiles,
    key: &str,
    mut work: Work,
    query: &QueryLogs,
) -> LogResult<(Work, LogResult<LogPage>)> {
    let page = refresh_page(files, key, &mut work, query);
    Ok((work, page))
}

fn refresh_page(
    files: &dyn LogFiles,
    key: &str,
    work: &mut Work,
    query: &QueryLogs,
) -> LogResult<LogPage> {
    let name = if key.is_empty() {
        files
            .catalog()?
            .first()
            .ok_or(LogError::FileGone)?
            .id
            .clone()
    } else {
        key.to_string()
    };
    let mut read = files.read(&name, work.index.next_read(), MAX_BATCH)?;
    let shared_prefix = work.prefix.len().min(read.prefix.len());
    if work.identity != read.identity
        || work.name != name
        || read.len < work.index.next_read()
        || work.prefix[..shared_prefix] != read.prefix[..shared_prefix]
    {
        *work = Work::new();
        work.identity = read.identity.clone();
        work.name = name.clone();
        work.index = Index::new(read.len.saturating_sub(MAX_WINDOW));
        read = files.read(&name, work.index.next_read(), MAX_BATCH)?;
    }
    work.prefix = read.prefix;
    work.index
        .append(work.index.next_read(), &read.bytes)
        .map_err(|_| LogError::InvalidRequest)?;
    let building = work.index.next_read() < read.len;
    let cursor = |offset: u64| LogCursor {
        generation: work.generation.clone(),
        offset: offset.to_string(),
    };
    let mut page = LogPage {
        rows: vec![],
        cursor: cursor(work.index.committed()),
        head: cursor(work.index.committed()),
        start: work.index.start().to_string(),
        file: name.clone(),
        building,
        more: false,
        partial: work.index.partial,
        malformed: work.index.malformed.to_string(),
        truncated: work.index.truncated.to_string(),
        indexed_bytes: work.index.next_read().to_string(),
        file_bytes: read.len.to_string(),
    };
    if building {
        return Ok(page);
    }
    let offset = if let Some(cursor) = &query.cursor {
        let offset = cursor
            .offset
            .parse::<u64>()
            .map_err(|_| LogError::InvalidRequest)?;
        if cursor.generation != work.generation
            || (query.direction == Direction::After && offset < work.index.start())
            || offset > work.index.committed()
        {
            return Err(LogError::CursorReset);
        }
        Some(offset)
    } else {
        None
    };
    // Historical pages use a bounded temporary index; they never replace the live window.
    let history = if query.direction == Direction::Before
        && offset.is_some_and(|offset| offset > 0 && offset <= work.index.start())
    {
        let end = offset.unwrap();
        let start = end.saturating_sub(MAX_BATCH as u64);
        let content = files.read(&name, start, (end - start) as usize)?;
        if content.identity != work.identity || content.len < end {
            return Err(LogError::CursorReset);
        }
        let mut index = Index::new(start);
        index
            .append(start, &content.bytes)
            .map_err(|_| LogError::InvalidRequest)?;
        Some((index, start))
    } else {
        None
    };
    let index = history.as_ref().map_or(&work.index, |(index, _)| index);
    let oldest = history.as_ref().map_or(index.start(), |(_, start)| {
        if index.is_empty() {
            *start
        } else {
            index.start()
        }
    });
    let scan = index.scan(&query.filter, query.direction, offset, query.limit as usize);
    page.cursor = cursor(scan.boundary);
    page.more = scan.more || (query.direction != Direction::After && oldest > 0);
    let text = query.filter.text.as_ref().map(|s| s.to_lowercase());
    // Read at most one response budget. Stop at the last consumed row, not the end of the scan.
    let mut bytes = 0;
    let mut response_bytes = 4096;
    let mut stopped = false;
    let reverse = query.direction != Direction::After;
    let entries: Box<dyn Iterator<Item = &Entry>> = if reverse {
        Box::new(scan.entries.iter().rev())
    } else {
        Box::new(scan.entries.iter())
    };
    page.cursor = cursor(offset.unwrap_or(if reverse {
        work.index.committed()
    } else {
        work.index.start()
    }));
    for entry in entries {
        if bytes + (entry.end - entry.start) as usize > MAX_BATCH {
            page.more = true;
            stopped = true;
            break;
        }
        let content = files.read(&name, entry.start, (entry.end - entry.start) as usize)?;
        if content.identity != work.identity || content.len < entry.end {
            return Err(LogError::CursorReset);
        }
        bytes += content.bytes.len();
        let raw = String::from_utf8_lossy(&content.bytes)
            .trim_end()
            .to_string();
        let target = index.target(entry).to_string();
        let value = serde_json::from_str::<serde_json::Value>(&raw).ok();
        let message = value
            .as_ref()
            .and_then(|v| v["fields"]["message"].as_str())
            .unwrap_or(&raw)
            .to_string();
        if text.as_ref().is_some_and(|text| {
            !message.to_lowercase().contains(text) && !target.to_lowercase().contains(text)
        }) {
            page.cursor = cursor(if reverse { entry.start } else { entry.end });
            continue;
        }
        let mut row = LogRow {
            id: format!("{}:{}", work.generation, entry.start),
            timestamp: entry.timestamp.map(|t| t.to_string()),
            level: entry.level,
            target,
            message,
            raw,
            unparsed: entry.unparsed,
            truncated: entry.truncated,
        };
        // Invalid UTF-8 expands during lossy decoding; escaping can expand it again.
        // Bound each field so even one pathological record fits a response page.
        for field in [&mut row.raw, &mut row.message] {
            if field.len() > MAX_LINE / 4 {
                let mut end = MAX_LINE / 4;
                while !field.is_char_boundary(end) {
                    end -= 1;
                }
                field.truncate(end);
                row.truncated = true;
            }
        }
        let row_bytes = serde_json::to_vec(&row)
            .map_err(|_| LogError::Unavailable)?
            .len()
            + 1;
        if response_bytes + row_bytes > MAX_BATCH {
            page.more = true;
            stopped = true;
            break;
        }
        response_bytes += row_bytes;
        page.cursor = cursor(if reverse { entry.start } else { entry.end });
        page.rows.push(row);
    }
    if !stopped {
        page.cursor = cursor(if reverse && !scan.more {
            oldest
        } else {
            scan.boundary
        });
    }
    if reverse {
        page.rows.reverse();
    }
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::atomic::{AtomicU64, Ordering},
    };
    #[derive(Default)]
    struct FakeClock(AtomicU64);
    impl Clock for FakeClock {
        fn millis(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }
    fn fixture() -> (tempfile::TempDir, Arc<FsLogFiles>) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("test.2026-09-11.app.log"),
            b"{\"level\":\"INFO\",\"fields\":{\"message\":\"hello\"}}\n",
        )
        .unwrap();
        let files = Arc::new(FsLogFiles::new(dir.path().into(), "test".into()));
        (dir, files)
    }
    fn query(id: &str) -> QueryLogs {
        QueryLogs {
            session: id.into(),
            filter: Filter::default(),
            direction: Direction::Latest,
            cursor: None,
            limit: 200,
        }
    }
    async fn ready(client: &LogsClient, id: &str) -> LogPage {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let page = client.query("window".into(), query(id)).await.unwrap();
                if !page.building {
                    return page;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn shared_leases_idle_reopen_and_expiry() {
        let (_dir, files) = fixture();
        let clock = Arc::new(FakeClock::default());
        let client = LogsClient::start(files, clock.clone()).await.unwrap();
        assert_eq!(client.call(Message::Inspect).await.unwrap(), (0, 0));
        let a = client
            .open(
                "window".into(),
                OpenLogs {
                    request_id: "a".into(),
                    file: None,
                },
            )
            .await
            .unwrap();
        let b = client
            .open(
                "window".into(),
                OpenLogs {
                    request_id: "b".into(),
                    file: None,
                },
            )
            .await
            .unwrap();
        let duplicate = client
            .open(
                "window".into(),
                OpenLogs {
                    request_id: "a".into(),
                    file: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(a.id, duplicate.id);
        let page = ready(&client, &a.id).await;
        assert_eq!(page.rows.len(), 1);
        assert_eq!(client.call(Message::Inspect).await.unwrap(), (2, 1));
        client.close("window".into(), a.id.clone()).await.unwrap();
        assert_eq!(client.call(Message::Inspect).await.unwrap(), (1, 1));
        client.close("window".into(), b.id).await.unwrap();
        clock.0.store(GRACE - 1, Ordering::SeqCst);
        assert_eq!(client.call(Message::Inspect).await.unwrap(), (0, 1));
        let c = client
            .open(
                "window".into(),
                OpenLogs {
                    request_id: "c".into(),
                    file: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            ready(&client, &c.id).await.head.generation,
            page.head.generation
        );
        clock.0.store(GRACE - 1 + LEASE, Ordering::SeqCst);
        assert_eq!(client.call(Message::Inspect).await.unwrap(), (0, 1));
        clock.0.store(GRACE - 1 + LEASE + GRACE, Ordering::SeqCst);
        assert_eq!(client.call(Message::Inspect).await.unwrap(), (0, 0));
        assert_eq!(
            client
                .query("window".into(), query(&c.id))
                .await
                .unwrap_err(),
            LogError::SessionExpired
        );
        client.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn repeated_sessions_release_and_reject_other_owner() {
        let (_dir, files) = fixture();
        let clock = Arc::new(FakeClock::default());
        let client = LogsClient::start(files, clock.clone()).await.unwrap();
        for n in 0..100 {
            let session = client
                .open(
                    "window".into(),
                    OpenLogs {
                        request_id: n.to_string(),
                        file: None,
                    },
                )
                .await
                .unwrap();
            ready(&client, &session.id).await;
            assert_eq!(
                client
                    .query("other".into(), query(&session.id))
                    .await
                    .unwrap_err(),
                LogError::SessionExpired
            );
            client.close("window".into(), session.id).await.unwrap();
            clock.0.fetch_add(GRACE, Ordering::SeqCst);
            assert_eq!(client.call(Message::Inspect).await.unwrap(), (0, 0));
        }
        client.shutdown().await.unwrap();
    }
    #[test]
    fn append_truncate_and_bad_cursor_preserve_shared_work() {
        let (dir, files) = fixture();
        let path = dir.path().join("test.2026-09-11.app.log");
        let (mut work, page) = refresh(files.as_ref(), "", Work::new(), &query("s")).unwrap();
        let page = page.unwrap();
        let mut next = query("s");
        next.direction = Direction::After;
        next.cursor = Some(page.head.clone());
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"{\"fields\":{\"message\":\"next\"}}")
            .unwrap();
        let partial = refresh_page(files.as_ref(), "", &mut work, &next).unwrap();
        assert!(partial.rows.is_empty());
        file.write_all(b"\n").unwrap();
        drop(file);
        let appended = refresh_page(files.as_ref(), "", &mut work, &next).unwrap();
        assert_eq!(appended.rows[0].message, "next");
        let generation = work.generation.clone();
        next.cursor.as_mut().unwrap().generation = "wrong".into();
        assert_eq!(
            refresh_page(files.as_ref(), "", &mut work, &next).unwrap_err(),
            LogError::CursorReset
        );
        assert_eq!(work.generation, generation);
        std::fs::write(&path, b"{}\n").unwrap();
        let reset = refresh_page(files.as_ref(), "", &mut work, &query("s")).unwrap();
        assert_ne!(reset.head.generation, generation);
        assert_eq!(reset.rows.len(), 1);
        assert_eq!(
            files.read("../outside.app.log", 0, 1).err(),
            Some(LogError::InvalidRequest)
        );
    }
    #[test]
    fn response_budget_continuation_does_not_skip_records() {
        let (dir, files) = fixture();
        let path = dir.path().join("test.2026-09-11.app.log");
        let mut file = std::fs::File::create(path).unwrap();
        for _ in 0..10 {
            writeln!(
                file,
                "{}",
                serde_json::json!({"fields":{"message":"x".repeat(120_000)}})
            )
            .unwrap();
        }
        drop(file);
        let mut work = Work::new();
        let mut request = query("s");
        request.direction = Direction::After;
        let mut ids = Vec::new();
        loop {
            let page = refresh_page(files.as_ref(), "", &mut work, &request).unwrap();
            if page.building {
                continue;
            }
            assert!(serde_json::to_vec(&page).unwrap().len() <= MAX_BATCH);
            ids.extend(page.rows.into_iter().map(|r| r.id));
            request.cursor = Some(page.cursor);
            if !page.more {
                break;
            }
        }
        assert_eq!(ids.len(), 10);
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 10);
    }
    #[test]
    fn invalid_utf8_record_always_fits_and_advances() {
        let (dir, files) = fixture();
        let mut bytes = vec![0x80; MAX_LINE];
        bytes.push(b'\n');
        std::fs::write(dir.path().join("test.2026-09-11.app.log"), &bytes).unwrap();
        let (_, page) = refresh(files.as_ref(), "", Work::new(), &query("s")).unwrap();
        let page = page.unwrap();
        assert_eq!(page.rows.len(), 1);
        assert!(page.rows[0].truncated);
        assert!(!page.more);
        assert!(serde_json::to_vec(&page).unwrap().len() < MAX_BATCH);
    }

    #[test]
    fn historical_pages_do_not_replace_the_live_index() {
        let (dir, files) = fixture();
        let bytes = (0..10)
            .map(|n| format!("{{\"target\":\"history\",\"fields\":{{\"message\":\"{n}\"}}}}\n"))
            .collect::<String>();
        std::fs::write(dir.path().join("test.2026-09-11.app.log"), &bytes).unwrap();
        let (mut work, _) = refresh(files.as_ref(), "", Work::new(), &query("s")).unwrap();
        let start = bytes.match_indices('\n').nth(3).unwrap().0 as u64 + 1;
        work.index = Index::new(start);
        work.index
            .append(start, &bytes.as_bytes()[start as usize..])
            .unwrap();
        let latest = refresh_page(files.as_ref(), "", &mut work, &query("s")).unwrap();
        assert_eq!(latest.rows.len(), 5);
        assert!(latest.more);
        let mut request = query("s");
        request.direction = Direction::Before;
        request.cursor = Some(latest.cursor);
        let older = refresh_page(files.as_ref(), "", &mut work, &request).unwrap();
        assert_eq!(
            older
                .rows
                .iter()
                .map(|row| row.message.as_str())
                .collect::<Vec<_>>(),
            ["0", "1", "2", "3", "4"]
        );
        assert!(!older.more);
        assert_eq!(older.head.generation, latest.head.generation);
        assert_eq!(older.head.offset, latest.head.offset);
        assert_eq!(work.index.len(), 5);
    }
}
