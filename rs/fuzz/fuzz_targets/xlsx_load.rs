// SPDX-License-Identifier: MIT
//! Fuzz the xlsx workbook entry point.
#![no_main]

use libfuzzer_sys::fuzz_target;
use nitro_xml_xlsx::load_workbook;

fuzz_target!(|data: &[u8]| {
    let _ = load_workbook(data);
});
