// SPDX-License-Identifier: MIT
//! CI-sized save-path edit check (#89): short seeded sequences + negative control.
//! Not a long fuzz campaign — see `cargo fuzz run edit_*` for overnight runs.

use nitro_xml_edit_fuzz::{ci_run_format, negative_control, Format, CI_SEQUENCES};

const XLSX: &[u8] = include_bytes!("../../../fixtures/generated/xlsx/cells_1k.xlsx");
const DOCX: &[u8] = include_bytes!("../../../fixtures/generated/docx/painter_line_numbers.docx");
const PPTX: &[u8] = include_bytes!("../../../fixtures/generated/pptx/slides_10.pptx");

fn main() {
    let mut failed = false;

    for (format, fixture) in [
        (Format::Xlsx, XLSX),
        (Format::Docx, DOCX),
        (Format::Pptx, PPTX),
    ] {
        print!("edit_ci {}: negative control … ", format.as_str());
        match negative_control(format, fixture) {
            Ok(()) => println!("ok (detected breakage)"),
            Err(e) => {
                println!("FAIL: {e}");
                failed = true;
            }
        }

        print!(
            "edit_ci {}: {CI_SEQUENCES} seeded sequences … ",
            format.as_str()
        );
        match ci_run_format(format, fixture) {
            Ok(reports) => {
                let applied: usize = reports.iter().map(|r| r.ops_applied).sum();
                let undos: usize = reports.iter().map(|r| r.undos).sum();
                println!("ok (applied={applied} undos={undos})");
            }
            Err(e) => {
                println!("FAIL: {e}");
                failed = true;
            }
        }
    }

    if failed {
        std::process::exit(1);
    }
    println!("edit_ci: all formats clean");
}
