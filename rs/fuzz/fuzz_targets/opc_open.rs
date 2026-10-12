// SPDX-License-Identifier: MIT
//! Fuzz OPC `Package::open` (format sniff + content types + relationships).
#![no_main]

use libfuzzer_sys::fuzz_target;
use nitro_xml_opc::opc::Package;

fuzz_target!(|data: &[u8]| {
    let _ = Package::open(data);
});
