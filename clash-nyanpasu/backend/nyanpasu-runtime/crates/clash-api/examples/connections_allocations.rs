//! Synthetic parse/drop allocation probe; not a process-memory benchmark.
//! Run with `cargo run -p clash-api --example connections_allocations`.
use std::hint::black_box;

use clash_api::ConnectionsSnapshot;
use serde_json::json;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    for extras in [false, true] {
        let mut connection: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/connection.json")).unwrap();
        if extras {
            connection["extension"] = json!({"nested": [1, "two", {"flag": true}]});
            connection["metadata"]["extension"] = json!({"nested": [null, "text", [1, 2, 3]]});
        }
        let frame = serde_json::to_vec(&json!({
            "downloadTotal": 456, "uploadTotal": 123, "memory": 1024,
            "connections": vec![connection; 150]
        }))
        .unwrap();
        // Initialize parser machinery before measuring; exclude fixture construction.
        black_box(serde_json::from_slice::<ConnectionsSnapshot>(&frame).unwrap());
        let profiler = dhat::Profiler::builder()
            .testing()
            .trim_backtraces(Some(0))
            .build();
        for _ in 0..100 {
            black_box(serde_json::from_slice::<ConnectionsSnapshot>(&frame).unwrap());
        }
        let stats = dhat::HeapStats::get();
        drop(profiler);
        println!(
            "extras={extras} frame_bytes={} allocations/frame={} requested_bytes/frame={} live_after_drop={}",
            frame.len(),
            stats.total_blocks / 100,
            stats.total_bytes / 100,
            stats.curr_bytes
        );
    }
}
