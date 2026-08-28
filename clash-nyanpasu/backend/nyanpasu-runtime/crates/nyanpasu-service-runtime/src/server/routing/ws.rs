use std::sync::Arc;

use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
    routing::any,
};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use nyanpasu_core_manager::LogFrame;
use nyanpasu_ipc::api::ws::events::{EVENT_URI, Event};
use tokio::sync::broadcast::error::RecvError;

use super::AppState;
use crate::server::{CoreManager, events::EventHub};

pub fn setup() -> Router<AppState> {
    let router = Router::new();
    router.route(EVENT_URI, any(ws_handler))
}

/// One turn of the sender loop. The two rings are read inside `select!` and
/// every mutation happens after it, so no receiver is reassigned while the
/// other branch's future is still alive.
#[allow(clippy::large_enum_variant)]
enum Next {
    Send(Event),
    Log(Arc<LogFrame>),
    StatusLag(u64),
    LogLag(u64),
    Closed,
}

/// One protocol, no negotiation: the service binary ships with the program that
/// consumes it, so there is no client to shield from a variant it cannot decode.
/// The query string is not extracted at all — routing ignores it, and so do we.
async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state.hub, state.core_manager))
}

async fn handle_socket(socket: WebSocket, hub: EventHub, core_manager: CoreManager) {
    // The subscriptions live and die with this task; there is no registry to
    // insert into and no id to collide with. Subscribing *before* the snapshot
    // is read is deliberate: a transition landing in between is then delivered
    // twice rather than lost.
    let mut events = hub.subscribe();
    let mut logs = hub.subscribe_logs();
    let (mut sink, mut stream) = socket.split();

    let handler = async { while let Some(Ok(_)) = stream.next().await {} };

    let sender = async {
        // Snapshot-on-connect, for everyone: the socket's first frame is the
        // current status, so a client never has to poll `/status` to find out
        // what it reconnected to. There is no equivalent for logs — a log
        // stream has no "current value", and the history lives in the JSONL
        // archive whose directory `/status` reports.
        if !send_snapshot(&mut sink, &core_manager).await {
            return;
        }
        loop {
            // Unbiased on purpose: neither stream may starve the other.
            let next = tokio::select! {
                received = events.recv() => match received {
                    Ok(event) => Next::Send(event),
                    Err(RecvError::Lagged(skipped)) => Next::StatusLag(skipped),
                    Err(RecvError::Closed) => Next::Closed,
                },
                received = logs.recv() => match received {
                    Ok(frame) => Next::Log(frame),
                    Err(RecvError::Lagged(skipped)) => Next::LogLag(skipped),
                    Err(RecvError::Closed) => Next::Closed,
                },
            };
            match next {
                Next::Send(event) => {
                    if !send_event(&mut sink, &event).await {
                        break;
                    }
                }
                // The ring carries frames, so the envelope is built here, once
                // per connection that is actually listening.
                Next::Log(frame) => {
                    if !send_event(&mut sink, &Event::new_core_log(frame)).await {
                        break;
                    }
                }
                // Only this connection pays for being slow. Warn once, then
                // jump to the live tail: the receiver skips the backlog, so a
                // full ring cannot spin us in a Lagged loop. Until L3 this line
                // needed a dedicated tracing target, because it would otherwise
                // have re-entered the very ring it had just overflowed. No
                // tracing output becomes an event now, so it is an ordinary log
                // line.
                Next::StatusLag(skipped) => {
                    tracing::warn!("ws subscriber dropped {skipped} events");
                    events = events.resubscribe();
                    // The gap may have swallowed a transition, so the client is
                    // resynchronised exactly as it was on connect. This is what
                    // the snapshot variant is for: nobody has to poll `/status`
                    // after a lag.
                    if !send_snapshot(&mut sink, &core_manager).await {
                        break;
                    }
                }
                // Deliberately no snapshot. This is the whole reason the log
                // ring is separate: a dropped log line is a dropped log line,
                // and making it cost a full status resend would turn a busy
                // core into a resynchronisation loop.
                Next::LogLag(skipped) => {
                    tracing::debug!("ws subscriber dropped {skipped} core log frames");
                    logs = logs.resubscribe();
                }
                // Both rings belong to the same hub, so either closing means
                // the service is going away.
                Next::Closed => break,
            }
        }
    };

    tokio::select! {
        _ = handler => (),
        _ = sender => (),
    }
}

/// Push the current status as one frame. `false` means the socket is gone.
async fn send_snapshot(
    sink: &mut SplitSink<WebSocket, Message>,
    core_manager: &CoreManager,
) -> bool {
    let event = Event::new_core_status_changed(core_manager.status().await);
    send_event(sink, &event).await
}

/// Frame and write one event. `false` means the socket is gone and the sender
/// must stop; a payload this service cannot serialize is a bug in the payload,
/// not a broken socket, so it is logged and skipped exactly as before.
async fn send_event(sink: &mut SplitSink<WebSocket, Message>, event: &Event) -> bool {
    let Ok(payload) = simd_json::to_vec(event) else {
        tracing::error!("Failed to serialize event: {:?}", event);
        return true;
    };
    match sink.send(Message::binary(payload)).await {
        Ok(()) => true,
        Err(error) => {
            tracing::error!("Failed to send event: {:?}", error);
            false
        }
    }
}
