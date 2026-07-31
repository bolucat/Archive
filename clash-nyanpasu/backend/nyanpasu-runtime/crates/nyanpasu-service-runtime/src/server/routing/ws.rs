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
use nyanpasu_ipc::api::ws::events::{EVENT_URI, Event};
use tokio::sync::broadcast::error::RecvError;

use super::AppState;
use crate::server::{
    CoreManager,
    events::{EventHub, WS_LAG_LOG_TARGET},
};

pub fn setup() -> Router<AppState> {
    let router = Router::new();
    router.route(EVENT_URI, any(ws_handler))
}

/// One protocol, no negotiation: the service binary ships with the program that
/// consumes it, so there is no client to shield from a variant it cannot decode.
/// The query string is not extracted at all — routing ignores it, and so do we.
async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state.hub, state.core_manager))
}

async fn handle_socket(socket: WebSocket, hub: EventHub, core_manager: CoreManager) {
    // The subscription lives and dies with this task; there is no registry to
    // insert into and no id to collide with. Subscribing *before* the snapshot
    // is read is deliberate: a transition landing in between is then delivered
    // twice rather than lost.
    let mut events = hub.subscribe();
    let (mut sink, mut stream) = socket.split();

    let handler = async { while let Some(Ok(_)) = stream.next().await {} };

    let sender = async {
        // Snapshot-on-connect, for everyone: the socket's first frame is the
        // current status, so a client never has to poll `/status` to find out
        // what it reconnected to.
        if !send_snapshot(&mut sink, &core_manager).await {
            return;
        }
        loop {
            match events.recv().await {
                Ok(event) => {
                    if !send_event(&mut sink, &event).await {
                        break;
                    }
                }
                // Only this connection pays for being slow. Warn once, then
                // jump to the live tail: the receiver skips the backlog, so a
                // full ring cannot spin us in a Lagged loop. The warn itself
                // is tagged with WS_LAG_LOG_TARGET, which the log-forwarding
                // subscriber filters out before it ever reaches the hub —
                // otherwise many lagging connections could collectively
                // refill the ring and re-lag each other.
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(
                        target: WS_LAG_LOG_TARGET,
                        "ws subscriber dropped {skipped} events"
                    );
                    events = events.resubscribe();
                    // The gap may have swallowed a transition, so the client is
                    // resynchronised exactly as it was on connect. This is what
                    // the snapshot variant is for: nobody has to poll `/status`
                    // after a lag.
                    if !send_snapshot(&mut sink, &core_manager).await {
                        break;
                    }
                }
                Err(RecvError::Closed) => break,
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
