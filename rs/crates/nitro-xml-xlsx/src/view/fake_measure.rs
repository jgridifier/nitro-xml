// SPDX-License-Identifier: MIT
//! Deterministic text measure matching `parity/oracle/xlsx.ts` `fakeXlsxMeasure`.

pub fn fake_xlsx_measure(text: &str) -> f64 {
    let mut w = 0.0;
    for ch in text.chars() {
        let cp = ch as u32;
        w += if cp > 255 { 14.0 } else { 7.0 };
    }
    w
}
