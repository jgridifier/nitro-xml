// SPDX-License-Identifier: MIT
//! Prints structural replay status for every edits/*.json script (#63).
//! Test/dev aid only — not shipped.

fn main() {
    use nitro_xml_opc::edit::{parse_edit_script, replay_script_json};
    use std::fs;
    use std::path::PathBuf;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../edits");
    let mut rows = Vec::new();
    for format in ["xlsx", "docx", "pptx"] {
        let dir = root.join(format);
        for ent in fs::read_dir(&dir).expect("edits dir") {
            let path = ent.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap();
            let script =
                parse_edit_script(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let report = replay_script_json(&text).unwrap();
            let ops: Vec<&str> = script.ops.iter().map(|o| o.command_id()).collect();
            rows.push((
                script.id,
                script.format.as_str().to_owned(),
                ops.join(","),
                report.status,
            ));
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    println!("scripts={}", rows.len());
    for (id, fmt, ops, status) in &rows {
        assert_ne!(status, "ok", "{id} must not pass without handlers");
        println!("{id}\t{fmt}\t{status}\t{ops}");
    }
}
