// SPDX-License-Identifier: MIT
//! `JSON.stringify`-compatible number text for gate dumps.

use crate::numfmt::js_int_string;

/// Integer-valued finite numbers as `JSON.stringify` / `String(n)` (see #17 `js_int_string`).
pub fn js_number_string(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 {
        return js_int_string(n);
    }
    let mut buf = ryu::Buffer::new();
    buf.format_finite(n).to_owned()
}
