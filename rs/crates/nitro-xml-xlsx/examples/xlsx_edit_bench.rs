// SPDX-License-Identifier: MIT
//! Per-edit latency of the live xlsx editor path (value edit → stale marks).
//!
//!   xlsx_edit_bench <fixture.xlsx> [formulas=2000] [edits=200]

use std::env;
use std::fs;
use std::time::Instant;

use nitro_xml_xlsx::editor::EditorSession;

fn main() {
    let args: Vec<String> = env::args().collect();
    let bytes = fs::read(&args[1]).expect("read fixture");
    let formulas: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let edits: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(200);
    let mut session = EditorSession::open(&bytes).expect("open");
    for r in 0..formulas {
        session
            .apply_raw(
                "setCellInput",
                serde_json::json!({"sheet": 0, "row": r, "col": 30, "text": format!("=SUM(B{0}:E{0})", r + 1)}),
            )
            .expect("formula");
    }
    let mut times = Vec::new();
    for i in 0..edits {
        let row = (i * 7) % formulas.max(1);
        let t = Instant::now();
        session
            .apply_raw(
                "setCellInput",
                serde_json::json!({"sheet": 0, "row": row, "col": 2, "text": format!("{}", 1000 + i)}),
            )
            .expect("edit");
        times.push(t.elapsed().as_secs_f64() * 1e3);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "formulas={formulas} edits={edits} median_ms={:.3} p95_ms={:.3} stale={}",
        times[times.len() / 2],
        times[times.len() * 95 / 100],
        session.stale_cells(0).len()
    );
}
