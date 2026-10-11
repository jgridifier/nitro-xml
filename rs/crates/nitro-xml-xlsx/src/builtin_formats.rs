// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/builtin-formats.ts; changes listed
// in PROVENANCE.md.
//! Built-in number format id → code map for style resolution (thin wrapper over [`crate::numfmt`]).

use crate::numfmt::BUILTIN_NUMBER_FORMATS;

/// Built-in number formats by id (en-US forms).
pub fn builtin_number_format(id: u32) -> Option<&'static str> {
    BUILTIN_NUMBER_FORMATS
        .iter()
        .find(|(known, _)| u32::from(*known) == id)
        .map(|(_, code)| *code)
}

/// Locale-dependent ids (27-36, 50-81) fall back to a date format.
pub fn builtin_format(id: u32) -> Option<&'static str> {
    if let Some(known) = builtin_number_format(id) {
        return Some(known);
    }
    if (27..=36).contains(&id) || (50..=81).contains(&id) {
        return Some("m/d/yyyy");
    }
    None
}
