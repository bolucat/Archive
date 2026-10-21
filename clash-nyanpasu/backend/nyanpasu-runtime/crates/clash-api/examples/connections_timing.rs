//! Parse/drop timing with the system allocator; run separately from DHAT probes.
use std::{hint::black_box, time::Instant};

use clash_api::ConnectionsSnapshot;
use serde_json::json;

fn main() {
    const FRAMES: usize = 500;
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
        for _ in 0..20 {
            black_box(serde_json::from_slice::<ConnectionsSnapshot>(&frame).unwrap());
        }
        let mut samples = [0.0_f64; 9];
        for sample in &mut samples {
            let start = Instant::now();
            for _ in 0..FRAMES {
                black_box(
                    serde_json::from_slice::<ConnectionsSnapshot>(black_box(&frame)).unwrap(),
                );
            }
            *sample = start.elapsed().as_secs_f64() * 1_000_000.0 / FRAMES as f64;
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "extras={extras} frame_bytes={} median_us={:.2} min_us={:.2} max_us={:.2}",
            frame.len(),
            samples[4],
            samples[0],
            samples[8]
        );
    }
}
