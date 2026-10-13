// SPDX-License-Identifier: MIT
//! CLI for the #90 long-session correctness harness.

mod session;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use session::{run_all, run_negative_controls, Fixtures, HarnessReport};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn load_fixtures() -> Result<Fixtures, String> {
    let root = repo_root();
    let read = |rel: &str| -> Result<Vec<u8>, String> {
        let path = root.join(rel);
        fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))
    };
    Ok(Fixtures {
        xlsx: read("fixtures/generated/xlsx/cells_1k.xlsx")?,
        docx: read("fixtures/generated/docx/painter_line_numbers.docx")?,
        pptx: read("fixtures/generated/pptx/slides_10.pptx")?,
    })
}

fn print_summary(report: &HarnessReport) {
    let ok = report.negative_control_ok && report.formats.iter().all(|f| f.ok);
    let parts: Vec<String> = report
        .formats
        .iter()
        .map(|f| {
            if f.ok {
                format!(
                    "{} ok (applied {}/{}, undos {}, redos {})",
                    f.format, f.ops_applied, f.ops_attempted, f.undos, f.redos
                )
            } else {
                format!("{} FAIL: {}", f.format, f.error.as_deref().unwrap_or("?"))
            }
        })
        .collect();
    let neg = if report.negative_control_ok {
        "negative-control ok"
    } else {
        "negative-control FAIL"
    };
    eprintln!(
        "long-session gate: {} | ops={} history_limit={} | {} | {}",
        if ok { "PASS" } else { "FAIL" },
        report.ops,
        report.history_limit,
        neg,
        parts.join(" · ")
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let negative_only = args.iter().any(|a| a == "--negative-control");

    let fixtures = match load_fixtures() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("long-session: {e}");
            return ExitCode::from(1);
        }
    };

    if negative_only {
        match run_negative_controls(&fixtures) {
            Ok(()) => {
                eprintln!("long-session negative-control: PASS");
                return ExitCode::from(0);
            }
            Err(e) => {
                eprintln!("long-session negative-control: FAIL ({e})");
                return ExitCode::from(1);
            }
        }
    }

    let report = run_all(&fixtures);
    let json = serde_json::to_string_pretty(&report).unwrap_or_else(|e| {
        eprintln!("json: {e}");
        String::new()
    });
    println!("{json}");
    print_summary(&report);

    let ok = report.negative_control_ok && report.formats.iter().all(|f| f.ok);
    ExitCode::from(if ok { 0 } else { 1 })
}
