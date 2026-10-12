// SPDX-License-Identifier: MIT
//! Fuzz the bounded zip reader: open, then try to read every part name.
#![no_main]

use libfuzzer_sys::fuzz_target;
use nitro_xml_opc::zip::ZipPackage;

fuzz_target!(|data: &[u8]| {
    let Ok(mut pkg) = ZipPackage::open(data) else {
        return;
    };
    let names: Vec<String> = pkg.part_names().map(str::to_owned).collect();
    for name in names {
        let _ = pkg.read(&name);
    }
});
