// SPDX-License-Identifier: MIT
//! Native `DocxHost::open` cost (single vs double zip on main).
//!
//!   cargo run --release --manifest-path perf/large/Cargo.toml --bin docx-host-open-mem -- --runs 21

use std::env;
use std::fs;
use std::time::Instant;

use nitro_xml_docx::edit::host::DocxHost;

const FIXTURES: &[(&str, &str)] = &[
    ("docx S", "fixtures/generated/docx/pages_10.docx"),
    ("docx M", "fixtures/generated/docx/pages_100.docx"),
    ("docx L", "fixtures/generated/docx/pages_500.docx"),
];

fn main() {
    let args: Vec<String> = env::args().collect();
    let runs: usize = flag(&args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(21);
    println!("docx_host_open_mem runs={runs}");

    for (tier, rel) in FIXTURES {
        bench_tier(tier, rel, runs);
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
}

fn bench_tier(tier: &str, rel: &str, runs: usize) {
    let bytes = fs::read(rel).unwrap_or_else(|e| panic!("read {rel}: {e}"));
    let file_kib = bytes.len() / 1024;

    let mut ms = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t0 = Instant::now();
        let _host = DocxHost::open(&bytes).expect("DocxHost::open");
        ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = ms[ms.len() / 2];
    println!(
        "{tier}: file={file_kib} KiB DocxHost::open median={median:.3} ms (n={runs})"
    );
}
