// SPDX-License-Identifier: MIT
//! Large-file time + memory baseline (native Rust path).
//!
//! Fixtures are generated on demand (do not commit L/XL binaries):
//!   python3 fixtures/gen/generate.py --id gen-xlsx-cells-1m \
//!     --id gen-docx-pages-500 --id gen-pptx-slides-200
//!
//! Reproduce:
//!   cargo run --release --manifest-path perf/large/Cargo.toml -- \
//!     --xlsx fixtures/generated/xlsx/cells_1m.xlsx \
//!     --xlsx-small fixtures/generated/xlsx/cells_100k.xlsx \
//!     --docx fixtures/generated/docx/pages_500.docx \
//!     --pptx fixtures/generated/pptx/slides_200.pptx \
//!     --runs 7

use std::env;
use std::fs;
use std::time::Instant;

use nitro_xml_docx::PrintSession;
use nitro_xml_pptx::Presentation;
use nitro_xml_xlsx::{load_workbook, open_first_view, ViewportRect};

fn main() {
    let args: Vec<String> = env::args().collect();
    let runs: usize = flag(&args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);

    println!("baseline runs={runs}");
    println!("host_rss_kib={} host_hwm_kib={}", rss_kib().0, rss_kib().1);

    if let Some(p) = flag(&args, "--xlsx-small") {
        bench_xlsx("xlsx_100k", &p, runs);
    }
    if let Some(p) = flag(&args, "--xlsx") {
        bench_xlsx("xlsx_1m", &p, runs);
    }
    if let Some(p) = flag(&args, "--docx") {
        bench_docx(&p, runs);
    }
    if let Some(p) = flag(&args, "--pptx") {
        bench_pptx(&p, runs);
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
}

fn bench_xlsx(label: &str, path: &str, runs: usize) {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let file_kib = bytes.len() / 1024;
    println!("\n## {label} path={path} file_kib={file_kib}");

    let view = ViewportRect {
        r0: 0,
        c0: 0,
        r1: 40,
        c1: 16,
    };
    let scroll = ViewportRect {
        r0: 2_000,
        c0: 0,
        r1: 2_040,
        c1: 16,
    };

    // --- open_first_view (TTFV proxy) ---
    let mut ms = Vec::with_capacity(runs);
    let mut hwm = Vec::with_capacity(runs);
    let mut rows_visited = 0u32;
    let mut sst = 0usize;
    let mut cells = 0usize;
    for i in 0..(runs + 1) {
        let t0 = Instant::now();
        let mut fv = open_first_view(&bytes, view).expect("open_first_view");
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        let (_, peak) = rss_kib();
        rows_visited = fv.rows_visited(fv.workbook().active_sheet as usize);
        sst = fv.shared_strings_resolved();
        cells = fv
            .sheet(fv.workbook().active_sheet as usize)
            .map(|s| s.cells.len())
            .unwrap_or(0);
        if i > 0 {
            ms.push(elapsed);
            hwm.push(peak);
        }
        if i == runs {
            // scroll while FirstView is live (clones package bytes today)
            let t1 = Instant::now();
            let changed = fv
                .ensure_viewport(fv.workbook().active_sheet as usize, scroll)
                .expect("scroll");
            let scroll_ms = t1.elapsed().as_secs_f64() * 1000.0;
            let (_, peak_scroll) = rss_kib();
            let cells_after = fv
                .sheet(fv.workbook().active_sheet as usize)
                .map(|s| s.cells.len())
                .unwrap_or(0);
            println!(
                "{label}_scroll_ensure_viewport_ms={scroll_ms:.2} changed={changed} cells_after={cells_after} rss_hwm_kib={peak_scroll}"
            );
        }
        drop(fv);
    }
    report(&format!("{label}_open_first_view"), &ms, &hwm);
    println!(
        "{label}_open_first_view_rows_visited={rows_visited} sst_resolved={sst} viewport_cells={cells}"
    );
    println!(
        "{label}_retained_copies_note=FirstView.bytes owns a full Vec copy of the package; Handle also keeps bytes → ≥2× file in WASM viewer path"
    );

    // --- full load_workbook ---
    let mut full_ms = Vec::with_capacity(runs);
    let mut full_hwm = Vec::with_capacity(runs);
    let mut full_cells = 0usize;
    for i in 0..(runs + 1) {
        let t0 = Instant::now();
        let wb = load_workbook(&bytes).expect("load_workbook");
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        let (_, peak) = rss_kib();
        full_cells = wb
            .active_worksheet
            .as_ref()
            .map(|s| s.cell_count())
            .unwrap_or(0);
        if i > 0 {
            full_ms.push(elapsed);
            full_hwm.push(peak);
        }
        drop(wb);
    }
    report(&format!("{label}_load_workbook_full"), &full_ms, &full_hwm);
    println!("{label}_load_workbook_full_cells={full_cells}");
}

fn bench_docx(path: &str, runs: usize) {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    println!("\n## docx path={path} file_kib={}", bytes.len() / 1024);

    // Full-body open (oracle / ensure path).
    let mut ms = Vec::with_capacity(runs);
    let mut hwm = Vec::with_capacity(runs);
    let mut full_blocks = 0usize;
    for i in 0..(runs + 1) {
        let t0 = Instant::now();
        let session = PrintSession::open(&bytes).expect("PrintSession::open");
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        let (_, peak) = rss_kib();
        full_blocks = session.blocks_parsed;
        if i > 0 {
            ms.push(elapsed);
            hwm.push(peak);
        }
        drop(session);
    }
    report("docx_print_session_open", &ms, &hwm);

    // First-view open (viewer TTFV — first hard page break only).
    let mut ms = Vec::with_capacity(runs);
    let mut hwm = Vec::with_capacity(runs);
    let mut first_blocks = 0usize;
    let mut truncated = false;
    for i in 0..(runs + 1) {
        let t0 = Instant::now();
        let session = PrintSession::open_first_view(&bytes, 1).expect("open_first_view");
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        let (_, peak) = rss_kib();
        first_blocks = session.blocks_parsed;
        truncated = session.body_truncated;
        if i > 0 {
            ms.push(elapsed);
            hwm.push(peak);
        }
        drop(session);
    }
    report("docx_open_first_view", &ms, &hwm);
    println!(
        "docx_full_blocks={full_blocks} docx_first_view_blocks={first_blocks} docx_body_truncated={truncated}"
    );
}

fn bench_pptx(path: &str, runs: usize) {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    println!("\n## pptx path={path} file_kib={}", leaked.len() / 1024);
    let mut ms = Vec::with_capacity(runs);
    let mut hwm = Vec::with_capacity(runs);
    let mut slides = 0usize;
    for i in 0..(runs + 1) {
        let t0 = Instant::now();
        let pres = Presentation::open(leaked).expect("Presentation::open");
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        let (_, peak) = rss_kib();
        slides = pres.slide_count();
        if i > 0 {
            ms.push(elapsed);
            hwm.push(peak);
        }
        drop(pres);
    }
    report("pptx_presentation_open", &ms, &hwm);
    println!("pptx_slides={slides}");
}

fn report(label: &str, ms: &[f64], hwm_kib: &[u64]) {
    let med_ms = percentile(ms, 0.5);
    let p95_ms = percentile(ms, 0.95);
    let med_hwm = percentile_u64(hwm_kib, 0.5);
    let max_hwm = hwm_kib.iter().copied().max().unwrap_or(0);
    println!(
        "{label}_ms_median={med_ms:.2} {label}_ms_p95={p95_ms:.2} {label}_rss_hwm_kib_median={med_hwm} {label}_rss_hwm_kib_max={max_hwm}"
    );
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
