//! Times the code that counts bits, for comparing builds for different x86-64 baselines.
//!
//! Portable x86-64 builds lack POPCNT, the instruction that counts the 1 bits in a word, so
//! `count_ones` compiles to a sequence of bit operations. Only `Graph::from_words` uses it in a hot
//! loop: one exclusive or and one bit count per pair of words. This program times it on three word
//! sets, plus an ordinary search as a control that should not change. The
//! `.github/workflows/popcount.yml` workflow builds it twice on an x86-64 runner, for the baseline
//! and for x86-64-v2 (which has POPCNT), and compares them.
//!
//!     cargo run --release --example popcount_bench -- 5
//!
//! Prints one `RESULT <workload> <median seconds>` line per workload.

use std::time::Instant;

use subgraphsearch_core::generate::generate;
use subgraphsearch_core::rng::Xoshiro;
use subgraphsearch_core::{Graph, Model, SearchParams, search};

fn median(mut times: Vec<f64>) -> f64 {
    times.sort_by(|a, b| a.total_cmp(b));
    times[times.len() / 2]
}

fn time<T>(rounds: usize, mut f: impl FnMut() -> T) -> f64 {
    std::hint::black_box(f());
    median(
        (0..rounds)
            .map(|_| {
                let start = Instant::now();
                std::hint::black_box(f());
                start.elapsed().as_secs_f64()
            })
            .collect(),
    )
}

fn main() {
    let rounds: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(5);
    let all15: Vec<u64> = (0..1u64 << 15).collect();
    let all16: Vec<u64> = (0..1u64 << 16).collect();
    let weight5: Vec<u64> = (0..1u64 << 18).filter(|w| w.count_ones() == 5).collect();
    let workloads: [(&str, &[u64], &[u32]); 3] = [
        ("from_words_all15_d1-3", &all15, &[1, 2, 3]),
        ("from_words_weight5_len18_d1-5", &weight5, &[1, 2, 3, 4, 5]),
        ("from_words_all16_d1-3", &all16, &[1, 2, 3]),
    ];
    for (name, words, distances) in workloads {
        let t = time(rounds, || Graph::from_words(words, distances).num_edges());
        println!("RESULT {name} {t:.4}");
    }
    // the control: a search, which counts no bits in its hot loop
    let g = generate(Model::Exponential, 1000, 500, &mut Xoshiro::new(1)).expect("a random graph");
    let mut p = SearchParams::new(100, 4950); // a 100-clique: unreachable, so every experiment makes all its moves
    p.experiments = 32;
    p.max_moves = 2000;
    let t = time(rounds, || search(&g, &p).expect("a search").successes());
    println!("RESULT search_control {t:.4}");
}
