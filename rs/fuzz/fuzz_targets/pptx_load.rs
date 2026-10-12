// SPDX-License-Identifier: MIT
//! Fuzz the pptx presentation entry point.
#![no_main]

use libfuzzer_sys::fuzz_target;
use nitro_xml_pptx::Presentation;

fuzz_target!(|data: &[u8]| {
    let _ = Presentation::open(data);
});
