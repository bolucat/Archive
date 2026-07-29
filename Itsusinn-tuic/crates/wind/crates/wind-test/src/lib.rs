pub mod socks5;
pub mod tuic;

pub mod benches {
	use std::sync::Arc;

	use criterion::{Criterion, black_box};

	pub fn bench_arc_comparison(c: &mut Criterion) {
		let mut group = c.benchmark_group("Arc Creation");

		group.bench_function("Arc::from(Vec<u8>)", |b| {
			b.iter(|| {
				let s = String::from("hello rusthello rusthello rusthello rusthello rusthello rust");
				let vec = s.into_bytes();
				black_box(Arc::<[u8]>::from(vec));
			})
		});

		group.bench_function("Arc::from(Box<[u8]>)", |b| {
			b.iter(|| {
				let s = String::from("hello rusthello rusthello rusthello rusthello rusthello rust");
				let vec = s.into_bytes();
				black_box(Arc::<[u8]>::from(vec.into_boxed_slice()));
			})
		});

		group.finish();
	}
}
