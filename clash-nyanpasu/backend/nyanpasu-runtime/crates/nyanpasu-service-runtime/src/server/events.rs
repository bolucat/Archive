use std::sync::Arc;

use nyanpasu_core_manager::LogFrame;
use nyanpasu_ipc::api::ws::events::Event;
use tokio::sync::broadcast;

/// Events buffered per subscriber. A connection that falls further behind than
/// this is told how many it lost instead of stalling the broadcast.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Core log frames buffered per subscriber. Four times the status ring, and the
/// reason is not a traffic estimate: the two streams fail differently. Dropping
/// a status frame costs a full snapshot resend, so that ring is sized to be
/// cheap to resynchronise; dropping a log line costs nothing, so this one is
/// sized to swallow a core's start-up burst outright. At roughly 200-400 bytes
/// a record the resident ceiling is a few hundred KiB.
const LOG_EVENT_CHANNEL_CAPACITY: usize = 1024;

/// Fan-out point for ws events. Cloning shares both channels.
///
/// Two rings, not one, because a subscriber that falls behind must be able to
/// lose a log line without paying for a status resynchronisation. Sharing one
/// ring makes the two indistinguishable, so every loss has to be handled as if
/// it were the expensive kind.
#[derive(Clone)]
pub struct EventHub {
    tx: broadcast::Sender<Event>,
    /// Frames, not events: the ring holds what the manager produced, and the
    /// `Event` wrapper is built per connection at send time.
    log_tx: broadcast::Sender<Arc<LogFrame>>,
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHub {
    pub fn new() -> Self {
        Self {
            tx: broadcast::channel(EVENT_CHANNEL_CAPACITY).0,
            log_tx: broadcast::channel(LOG_EVENT_CHANNEL_CAPACITY).0,
        }
    }

    /// Fan out an event: synchronous and never awaits; only brief internal
    /// channel locking. It is unaffected by slow subscribers. `send` fails only
    /// when nobody is subscribed, which is the normal idle state, so the result
    /// is dropped.
    pub fn send(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Fan out one core log frame. Same contract as [`Self::send`] —
    /// synchronous, never awaits, and a failure just means nobody is listening.
    pub fn send_log(&self, frame: Arc<LogFrame>) {
        let _ = self.log_tx.send(frame);
    }

    pub(crate) fn has_log_subscribers(&self) -> bool {
        self.log_tx.receiver_count() > 0
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<Arc<LogFrame>> {
        self.log_tx.subscribe()
    }

    #[cfg(test)]
    fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }

    #[cfg(test)]
    fn log_receiver_count(&self) -> usize {
        self.log_tx.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyanpasu_core_manager::{CoreKind, LogField, LogLevel, LogStream, LogTimestamp};
    use nyanpasu_ipc::api::status::{CoreInfos, CoreState, CoreStateDetail};
    use tokio::sync::broadcast::error::TryRecvError;

    fn state_event(state: CoreState) -> Event {
        Event::new_core_state_changed(state)
    }

    #[tokio::test]
    async fn every_subscriber_receives_a_sent_event() {
        let hub = EventHub::new();
        let mut first = hub.subscribe();
        let mut second = hub.subscribe();
        hub.send(state_event(CoreState::Running));
        assert!(matches!(
            first.recv().await.unwrap(),
            Event::CoreStateChanged(CoreState::Running)
        ));
        assert!(matches!(
            second.recv().await.unwrap(),
            Event::CoreStateChanged(CoreState::Running)
        ));
    }

    #[tokio::test]
    async fn a_stalled_subscriber_does_not_stall_the_broadcast() {
        let hub = EventHub::new();
        let mut stalled = hub.subscribe();
        let mut healthy = hub.subscribe();
        for _ in 0..(EVENT_CHANNEL_CAPACITY * 2) {
            hub.send(state_event(CoreState::Running));
            assert!(matches!(
                healthy.try_recv().unwrap(),
                Event::CoreStateChanged(CoreState::Running)
            ));
        }
        assert!(matches!(stalled.try_recv(), Err(TryRecvError::Lagged(_))));
    }

    #[tokio::test]
    async fn lag_recovery_with_feedback_reaches_the_tail() {
        // Models a subscriber that fell behind: the resubscribe jump must land
        // at the tail with no residual Lagged and no spin. The feedback this
        // once guarded against — a lag warning re-entering the ring it had just
        // overflowed — is structurally gone, because no tracing output becomes
        // an event any more.
        let hub = EventHub::new();
        let mut lagged = hub.subscribe();
        for _ in 0..(EVENT_CHANNEL_CAPACITY * 2) {
            hub.send(state_event(CoreState::Running));
        }
        assert!(matches!(lagged.try_recv(), Err(TryRecvError::Lagged(_))));
        // One more event arrives during recovery…
        hub.send(state_event(CoreState::Running)); // stand-in for the warn log event
        // …then jump to the tail.
        let mut recovered = lagged.resubscribe();
        assert!(matches!(recovered.try_recv(), Err(TryRecvError::Empty)));
        // The connection is live again: the next event arrives normally.
        hub.send(state_event(CoreState::Running));
        assert!(matches!(
            recovered.try_recv().unwrap(),
            Event::CoreStateChanged(CoreState::Running)
        ));
    }

    #[test]
    fn dropping_a_subscriber_unsubscribes_it() {
        let hub = EventHub::new();
        let events = hub.subscribe();
        assert_eq!(hub.receiver_count(), 1);
        drop(events);
        assert_eq!(hub.receiver_count(), 0);
    }

    #[test]
    fn sending_without_subscribers_is_not_an_error() {
        EventHub::new().send(state_event(CoreState::Running));
    }

    /// The ws handler frames events with simd_json; the bytes on the socket
    /// must match the shape the client's serde_json decoder expects.
    #[test]
    fn ws_frames_are_pinned() {
        let frame = simd_json::to_vec(&state_event(CoreState::Running)).unwrap();
        assert_eq!(
            String::from_utf8(frame).unwrap(),
            r#"{"CoreStateChanged":"Running"}"#
        );
    }

    /// The status snapshot frame as `simd_json` writes it. Pinned separately
    /// from the ipc crate's golden because this is the serializer that actually
    /// feeds the socket, and the client decodes with `serde_json`: the two must
    /// agree.
    #[test]
    fn ws_status_frames_are_pinned() {
        let event = Event::new_core_status_changed(CoreInfos {
            r#type: None,
            state: CoreState::Stopped(None),
            state_changed_at: 42,
            config_path: None,
            controller: None,
            health: None,
            revision: None,
            detail: Some(CoreStateDetail::Stopped { reason: None }),
        });
        let frame = simd_json::to_vec(&event).unwrap();
        assert_eq!(
            String::from_utf8(frame).unwrap(),
            concat!(
                r#"{"CoreStatusChanged":{"type":null,"state":{"Stopped":null},"#,
                r#""state_changed_at":42,"config_path":null,"#,
                r#""detail":{"Stopped":{"reason":null}}}}"#
            )
        );
    }

    fn core_log_frame() -> Arc<LogFrame> {
        Arc::new(LogFrame {
            at: 1_700_000_000_000,
            epoch: 1,
            kind: CoreKind::Mihomo,
            stream: LogStream::Stdout,
            level: LogLevel::Info,
            timestamp: Some(LogTimestamp {
                raw: "2026-07-29T00:16:22.646059400+08:00".to_owned(),
                unix_ms: Some(1_753_719_382_646),
                inferred: false,
            }),
            target: None,
            message: "hello core".to_owned(),
            fields: vec![LogField {
                key: "request".to_owned(),
                value: "7".to_owned(),
            }],
            raw: "time=\"2026-07-29T00:16:22.646059400+08:00\" level=info msg=\"hello core\" request=7"
                .to_owned(),
            truncated: false,
        })
    }

    /// Byte-for-byte the literal `nyanpasu_ipc`'s `the_core_log_event_is_pinned`
    /// asserts against `serde_json`. The service writes the socket with
    /// `simd_json` and the client decodes with `serde_json`; the two must agree.
    #[test]
    fn ws_core_log_frames_are_pinned() {
        let payload = simd_json::to_vec(&Event::new_core_log(core_log_frame())).unwrap();
        assert_eq!(
            String::from_utf8(payload).unwrap(),
            concat!(
                r#"{"CoreLog":{"at":1700000000000,"epoch":1,"kind":"mihomo","stream":"stdout","#,
                r#""level":"info","timestamp":{"raw":"2026-07-29T00:16:22.646059400+08:00","#,
                r#""unix_ms":1753719382646,"inferred":false},"target":null,"#,
                r#""message":"hello core","fields":[{"key":"request","value":"7"}],"#,
                r#""raw":"time=\"2026-07-29T00:16:22.646059400+08:00\" level=info "#,
                r#"msg=\"hello core\" request=7","truncated":false}}"#
            )
        );
    }

    /// The two rings are separate objects, so a subscriber to one is not a
    /// subscriber to the other. Without this, `send_log` would reach the status
    /// stream and the split would be decorative.
    #[test]
    fn the_two_rings_have_independent_subscribers() {
        let hub = EventHub::new();
        let _status = hub.subscribe();
        assert_eq!(hub.receiver_count(), 1);
        assert_eq!(hub.log_receiver_count(), 0);
        assert!(!hub.has_log_subscribers());
        let mut logs = hub.subscribe_logs();
        assert_eq!(hub.receiver_count(), 1);
        assert_eq!(hub.log_receiver_count(), 1);
        assert!(hub.has_log_subscribers());

        // The ring shares the manager's allocation rather than copying it.
        let frame = core_log_frame();
        hub.send_log(Arc::clone(&frame));
        assert!(Arc::ptr_eq(
            &frame,
            &logs.try_recv().expect("the frame arrives")
        ));
    }

    /// The point of the whole stage: a log burst big enough to overrun its own
    /// ring several times over leaves the status subscriber untouched. On one
    /// ring this same burst would have forced a `Lagged`, and the ws handler
    /// answers a status `Lagged` with a full snapshot resend — so a chatty core
    /// would have driven a resynchronisation loop.
    #[tokio::test]
    async fn a_log_burst_never_lags_the_status_ring() {
        let hub = EventHub::new();
        let mut status = hub.subscribe();
        let mut logs = hub.subscribe_logs();
        hub.send(state_event(CoreState::Running));
        // Twice the ring, matching the convention the two tests above use.
        for _ in 0..(LOG_EVENT_CHANNEL_CAPACITY * 2) {
            hub.send_log(core_log_frame());
        }

        // The status subscriber still sees its one event, in order, un-lagged.
        assert!(matches!(
            status.try_recv().unwrap(),
            Event::CoreStateChanged(CoreState::Running)
        ));
        assert!(matches!(status.try_recv(), Err(TryRecvError::Empty)));
        // The log subscriber is the only one that paid, and it paid in log
        // frames — which is free, by design.
        assert!(matches!(logs.try_recv(), Err(TryRecvError::Lagged(_))));
    }
}
