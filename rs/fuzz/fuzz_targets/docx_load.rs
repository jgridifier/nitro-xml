// SPDX-License-Identifier: MIT
//! Fuzz the docx package reader entry point.
#![no_main]

use libfuzzer_sys::fuzz_target;
use nitro_xml_docx::read_package;

fuzz_target!(|data: &[u8]| {
    let _ = read_package(data);
});
