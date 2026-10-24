// SPDX-License-Identifier: MIT
//! Native docx soft-share ensure_edit: cached `word/document.xml` vs second zip read.

use std::env;
use std::fs;
use std::time::Instant;

use nitro_xml_docx::edit::host::DocxHost;
use nitro_xml_docx::PrintSession;

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
    println!("docx_ensure_edit_mem runs={runs}");

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

    let mut cached_ms = Vec::with_capacity(runs);
    let mut zip_ms = Vec::with_capacity(runs);

    for _ in 0..runs {
        let session = PrintSession::open(&bytes).expect("PrintSession::open");
        let model = &session.model;
        let t0 = Instant::now();
        let xml = model.source_document_xml.clone();
        let _ = DocxHost::from_document_model_with_xml(model, xml, &bytes).expect("cached");
        cached_ms.push(t0.elapsed().as_secs_f64() * 1000.0);

        let session2 = PrintSession::open(&bytes).expect("PrintSession::open");
        let model2 = &session2.model;
        let t0 = Instant::now();
        let _ =
            DocxHost::from_document_model_with_xml(model2, None, &bytes).expect("zip fallback");
        zip_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }

    println!(
        "\n## {tier} fixture={rel} cached_ms_median={:.3} zip_read_ms_median={:.3} delta={:.1}%",
        percentile(&cached_ms, 0.5),
        percentile(&zip_ms, 0.5),
        100.0 * (1.0 - percentile(&cached_ms, 0.5) / percentile(&zip_ms, 0.5))
    );
}

fn percentile(xs: &[f64], p: f64) -> f64 {
    let mut a = xs.to_vec();
    a.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let i = ((a.len() - 1) as f64 * p).round() as usize;
    a[i]
}
