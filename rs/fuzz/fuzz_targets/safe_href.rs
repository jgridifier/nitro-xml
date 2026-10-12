// SPDX-License-Identifier: MIT
//! Fuzz hyperlink safety policy (store/open/reject). Never panics on any UTF-8-ish input.
#![no_main]

use libfuzzer_sys::fuzz_target;
use nitro_xml_opc::opc::{hyperlink_policy, is_safe_hyperlink_href};

fuzz_target!(|data: &[u8]| {
    let s = String::from_utf8_lossy(data);
    let _ = is_safe_hyperlink_href(&s);
    let _ = hyperlink_policy(Some(&s));
    let _ = hyperlink_policy(None);
});
