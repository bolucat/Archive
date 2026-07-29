use criterion::{criterion_group, criterion_main};
use wind_test::benches::bench_arc_comparison;

criterion_group!(benches, bench_arc_comparison);
criterion_main!(benches);
