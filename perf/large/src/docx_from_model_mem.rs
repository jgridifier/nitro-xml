// SPDX-License-Identifier: MIT
//! from_document_model cost only (soft-share unlock clone).
use std::env;
use std::fs;
use std::time::Instant;
use nitro_xml_docx::read_package;
use nitro_xml_docx::edit::host::DocxHost;

const FIXTURES: &[(&str, &str)] = &[
    ("docx S", "fixtures/generated/docx/pages_10.docx"),
    ("docx M", "fixtures/generated/docx/pages_100.docx"),
    ("docx L", "fixtures/generated/docx/pages_500.docx"),
];

fn main() {
    let args: Vec<String> = env::args().collect();
    let runs: usize = args.windows(2).find(|w| w[0]=="--runs").and_then(|w| w[1].parse().ok()).unwrap_or(21);
    println!("docx_from_model_mem runs={runs}");
    for (tier, rel) in FIXTURES {
        let bytes = fs::read(rel).unwrap();
        let loaded = read_package(&bytes).expect("read");
        let xml = loaded.model.source_document_xml.clone().unwrap_or_default();
        let mut ms = Vec::with_capacity(runs);
        for _ in 0..runs {
            let t0 = Instant::now();
            let _h = DocxHost::from_document_model(&loaded.model, xml.clone());
            ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        ms.sort_by(|a,b| a.partial_cmp(b).unwrap());
        println!("{tier}: from_document_model median={:.3} ms file={} KiB", ms[ms.len()/2], bytes.len()/1024);
    }
}
