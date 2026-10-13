// SPDX-License-Identifier: MIT
//! Save-path fuzz: xlsx random edits, undo-to-start, save/reopen (#89).
//! Build with `--features libfuzzer` (cargo-fuzz enables this).
#![cfg_attr(feature = "libfuzzer", no_main)]

#[cfg(feature = "libfuzzer")]
mod fuzz {
    use libfuzzer_sys::fuzz_target;
    use nitro_xml_edit_fuzz::{fuzz_format, Format};

    const FIXTURE: &[u8] = include_bytes!("../../../fixtures/generated/xlsx/cells_1k.xlsx");

    fuzz_target!(|data: &[u8]| {
        fuzz_format(Format::Xlsx, FIXTURE, data);
    });
}

#[cfg(not(feature = "libfuzzer"))]
fn main() {
    eprintln!("edit_xlsx: build with --features libfuzzer (or use bin edit_ci)");
    std::process::exit(2);
}
