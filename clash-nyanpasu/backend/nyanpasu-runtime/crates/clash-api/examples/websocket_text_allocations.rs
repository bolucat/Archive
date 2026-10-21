//! Compare conversion + decoding of an already received text payload.
//! Excludes network IO and tungstenite read-buffer allocations.
//! Run: cargo run -p clash-api --release --example websocket_text_allocations
use std::hint::black_box;
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;
fn measure(name: &str, mut f: impl FnMut()) {
    let profiler = dhat::Profiler::builder()
        .testing()
        .trim_backtraces(Some(0))
        .build();
    for _ in 0..100 {
        f();
    }
    let stats = dhat::HeapStats::get();
    drop(profiler);
    println!(
        "{name}: blocks={} bytes={} live={}",
        stats.total_blocks / 100,
        stats.total_bytes / 100,
        stats.curr_bytes
    );
}
fn main() {
    for extras in [false, true] {
        let input = String::from_utf8({
            let mut connection: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/connection.json")).unwrap();
            if extras {
                connection["extension"] = serde_json::json!({"nested": [1, "two", {"flag": true}]});
                connection["metadata"]["extension"] = serde_json::json!({"nested": [null, "text", [1,2,3]]});
            }
            serde_json::to_vec(&serde_json::json!({"downloadTotal":456,"uploadTotal":123,"memory":1024,"connections":vec![connection;150]})).unwrap()
        }).unwrap();
        let text = tungstenite::protocol::frame::Utf8Bytes::from(input);
        measure(&format!("wrapped extras={extras}"), || {
            let wrapped: reqwest_websocket::Message =
                tungstenite::Message::Text(text.clone()).try_into().unwrap();
            let reqwest_websocket::Message::Text(s) = wrapped else {
                unreachable!()
            };
            black_box(serde_json::from_str::<clash_api::ConnectionsSnapshot>(&s).unwrap());
        });
        measure(&format!("native extras={extras}"), || {
            let native = tungstenite::Message::Text(text.clone());
            let tungstenite::Message::Text(s) = native else {
                unreachable!()
            };
            black_box(serde_json::from_str::<clash_api::ConnectionsSnapshot>(&s).unwrap());
        });
    }
}
