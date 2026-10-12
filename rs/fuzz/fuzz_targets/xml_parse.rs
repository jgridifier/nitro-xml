// SPDX-License-Identifier: MIT
//! Fuzz the streaming XML parser (`parse_xml_bytes`). Errors are success; panics are bugs.
#![no_main]

use libfuzzer_sys::fuzz_target;
use nitro_xml_opc::xml::parse_xml_bytes;

fuzz_target!(|data: &[u8]| {
    let _ = parse_xml_bytes(data);
});
