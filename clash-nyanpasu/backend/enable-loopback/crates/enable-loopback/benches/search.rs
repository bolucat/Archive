//! Release comparison of the actual cached matcher, including query parsing,
//! result collection and relevance ordering. No local AppContainer data is used.
use enable_loopback::search::{Query, SearchFields};
use rayon::prelude::*;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn synthetic(count: usize) -> Vec<SearchFields> {
    (0..count).map(|i| {
        let name = ["Windows Store", "Microsoft Photos", "Windows Calculator", "游戏中心", "Contoso Music", "Runtime Broker"][i % 6];
        SearchFields::new([
            (i % 13 != 0).then(|| format!("{name} {}", i % 23)),
            (i % 4 != 0).then(|| format!("{name} application and desktop integration service for Windows")),
            (i % 7 != 0).then(|| format!("Microsoft.{}_22410.1401.2.0_x64__8wekyb3d8bbwe", name.replace(' ', ""))),
            Some(format!("microsoft.{}_8wekyb3d8bbwe!app", name.replace(' ', ""))),
            Some(format!("S-1-15-2-{}-{}-3500123456-1900123456-2400123456-1200123456-3100123456", 1000000000 + i, 2000000000 + i)),
            Some(if i % 9 == 0 { "DESKTOP-SYNTHETIC\\alice; DESKTOP-SYNTHETIC\\bob" } else { "DESKTOP-SYNTHETIC\\alice" }.into()),
            (i % 3 == 0).then(|| format!(r"C:\Program Files\WindowsApps\Microsoft.{}_22410.1401.2.0_x64__8wekyb3d8bbwe\app.exe", name.replace(' ', ""))),
        ])
    }).collect()
}

fn run(rows: &[SearchFields], text: &str, parallel: bool) -> Vec<(usize, u32)> {
    let query = Query::new(black_box(text));
    let score =
        |(index, fields): (usize, &SearchFields)| query.score(fields).map(|score| (index, score));
    let mut matches: Vec<_> = if parallel {
        rows.par_iter().enumerate().filter_map(score).collect()
    } else {
        rows.iter().enumerate().filter_map(score).collect()
    };
    matches.sort_unstable_by_key(|&(index, score)| (std::cmp::Reverse(score), index));
    matches
}

fn measure(mut work: impl FnMut()) -> f64 {
    let warmup = Instant::now();
    while warmup.elapsed() < Duration::from_millis(100) {
        work();
    }
    let mut samples = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        let mut iterations = 0;
        while start.elapsed() < Duration::from_millis(100) {
            work();
            iterations += 1;
        }
        samples.push(start.elapsed().as_secs_f64() * 1000.0 / f64::from(iterations));
    }
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn main() {
    println!("rayon threads: {}", rayon::current_num_threads());
    println!("rows,query,matches,sequential_ms,rayon_ms");
    for count in [260, 1_000, 3_000, 10_000, 100_000] {
        let rows = synthetic(count);
        // Common one-token, cross-column two-token, and three-token query.
        for text in ["store", "microsoft alice", "windows store 3500123456"] {
            let sequential = run(&rows, text, false);
            assert_eq!(sequential, run(&rows, text, true));
            let seq = measure(|| {
                black_box(run(black_box(&rows), text, false));
            });
            let par = measure(|| {
                black_box(run(black_box(&rows), text, true));
            });
            println!("{count},{text},{},{seq:.6},{par:.6}", sequential.len());
        }
    }
}
