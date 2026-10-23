// SPDX-License-Identifier: MIT
//! Native ensure_edit package-copy cost (viewer deck loaded, soft-share host build).
//!
//! Reproduce:
//!   # branch (Arc-shared `original`)
//!   cargo run --release --manifest-path perf/large/Cargo.toml --bin pptx-ensure-edit-mem -- --runs 21
//!   # main baseline (`original` = `bytes.to_vec()`)
//!   NITRO_PPTX_MAIN_BENCH=1 cargo run --release --manifest-path perf/large/Cargo.toml --bin pptx-ensure-edit-mem -- --runs 21

use std::env;
use std::fs;
use std::sync::Arc;
use std::time::Instant;

use nitro_xml_pptx::edit::host::PptxHost;
use nitro_xml_pptx::editor::PptxEditor;
use nitro_xml_pptx::Presentation;

const FIXTURES: &[(&str, &str)] = &[
    ("pptx S", "fixtures/generated/pptx/slides_10.pptx"),
    ("pptx M", "fixtures/generated/pptx/slides_50.pptx"),
    ("pptx L", "fixtures/generated/pptx/slides_200.pptx"),
];

fn main() {
    let args: Vec<String> = env::args().collect();
    let runs: usize = flag(&args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(21);
    let mode = if cfg!(pptx_main_bench) {
        "main_to_vec"
    } else {
        "branch_arc"
    };
    println!("pptx_ensure_edit_mem mode={mode} runs={runs}");

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
    let package: Arc<[u8]> = Arc::from(bytes);
    let file_bytes = package.len();

    let mut ms = Vec::with_capacity(runs);
    let mut hwm_delta = Vec::with_capacity(runs);
    let mut copy_bytes = Vec::with_capacity(runs);
    let mut arc_refs = Vec::with_capacity(runs);

    for _ in 0..runs {
        let refs_before = Arc::strong_count(&package);
        let (_, hwm_before) = rss_kib();

        let mut pres = Presentation::open(package.as_ref()).expect("Presentation::open");
        pres.ensure_all_slides().expect("ensure_all_slides");
        assert!(pres.all_slides_loaded());

        let t0 = Instant::now();
        let host = PptxHost::from_presentation(&mut pres).expect("from_presentation");
        let (editor, copied) = attach_editor(host, &package);
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        let (_, hwm_after) = rss_kib();

        ms.push(elapsed);
        hwm_delta.push(hwm_after.saturating_sub(hwm_before));
        copy_bytes.push(copied);
        arc_refs.push((Arc::strong_count(&package) - refs_before) as u64);

        drop(editor);
        drop(pres);
    }

    println!(
        "\n## {tier} fixture={rel} file_kib={} mode={}",
        file_bytes / 1024,
        if cfg!(pptx_main_bench) {
            "main_to_vec"
        } else {
            "branch_arc"
        }
    );
    println!(
        "ensure_edit_ms_median={:.3} ensure_edit_ms_p95={:.3}",
        percentile(&ms, 0.5),
        percentile(&ms, 0.95)
    );
    println!(
        "hwm_delta_kib_median={} hwm_delta_kib_p95={}",
        percentile_u64(&hwm_delta, 0.5),
        percentile_u64(&hwm_delta, 0.95)
    );
    println!(
        "retained_copy_bytes_median={} arc_ref_delta_median={}",
        percentile_u64(&copy_bytes, 0.5),
        percentile_u64(&arc_refs, 0.5)
    );
}

#[cfg(not(pptx_main_bench))]
fn attach_editor(host: PptxHost, package: &Arc<[u8]>) -> (PptxEditor, u64) {
    let editor = PptxEditor::from_opened_host(host, Arc::clone(package));
    (editor, 0)
}

#[cfg(pptx_main_bench)]
fn attach_editor(host: PptxHost, package: &Arc<[u8]>) -> (PptxEditor, u64) {
    let copied = package.len() as u64;
    let editor = PptxEditor::from_opened_host(host, package.as_ref());
    (editor, copied)
}

fn percentile(xs: &[f64], p: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let i = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[i.min(v.len() - 1)]
}

fn percentile_u64(xs: &[u64], p: f64) -> u64 {
    if xs.is_empty() {
        return 0;
    }
    let mut v = xs.to_vec();
    v.sort_unstable();
    let i = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[i.min(v.len() - 1)]
}

fn rss_kib() -> (u64, u64) {
    let Ok(s) = fs::read_to_string("/proc/self/status") else {
        return (0, 0);
    };
    let mut rss = 0u64;
    let mut hwm = 0u64;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            rss = parse_kib(rest);
        } else if let Some(rest) = line.strip_prefix("VmHWM:") {
            hwm = parse_kib(rest);
        }
    }
    (rss, hwm)
}

fn parse_kib(s: &str) -> u64 {
    s.split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}
