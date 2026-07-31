use nyanpasu_ipc::api::ws::events::Event;
use tokio::sync::broadcast;

/// Events buffered per subscriber. A connection that falls further behind than
/// this is told how many it lost instead of stalling the broadcast.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Tracing target for ws-lag diagnostics. Log lines with this target must
/// never be forwarded into the EventHub: a lag warning that re-enters the
/// ring it just overflowed would let many slow connections feed each other
/// a log storm (feedback ratio ~ connections / capacity).
pub(crate) const WS_LAG_LOG_TARGET: &str = "nyanpasu_service::ws::lag";

/// Filter applied by the log-forwarding subscriber before `EventHub::send`.
pub(crate) fn should_forward_to_hub(target: &str) -> bool {
    target != WS_LAG_LOG_TARGET
}

/// Fan-out point for ws events. Cloning shares the one channel.
#[derive(Clone)]
pub struct EventHub {
    tx: broadcast::Sender<Event>,
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
        }
    }

    /// Fan out an event: synchronous and never awaits; only brief internal
    /// channel locking. It is unaffected by slow subscribers. `send` fails only
    /// when nobody is subscribed, which is the normal idle state, so the result
    /// is dropped. It must never be logged: the log-forwarding subscriber calls
    /// this from inside the tracing writer.
    pub fn send(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    #[cfg(test)]
    fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        // Models the production hazard: recovering from Lagged itself feeds one
        // more event back into a full ring (the warn log). The resubscribe jump
        // must land at the tail with no residual Lagged and no spin.
        let hub = EventHub::new();
        let mut lagged = hub.subscribe();
        for _ in 0..(EVENT_CHANNEL_CAPACITY * 2) {
            hub.send(state_event(CoreState::Running));
        }
        assert!(matches!(lagged.try_recv(), Err(TryRecvError::Lagged(_))));
        // The ws handler's recovery sequence: warn (feeds back into the hub)…
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

    #[test]
    fn lag_diagnostics_are_never_forwarded_to_the_hub() {
        assert!(!should_forward_to_hub(WS_LAG_LOG_TARGET));
        assert!(should_forward_to_hub("nyanpasu_service::server"));
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
}
