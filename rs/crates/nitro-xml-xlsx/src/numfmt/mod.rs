// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/index.ts; changes listed in PROVENANCE.md.
//! Excel number formats: `format_value` and the pieces it is built from.
//!
//! `display-text.ts` and input parsing are follow-ups (#130); they need the workbook model.

mod builtins;
mod compile;
mod date;
mod decimal;
mod decimals;
mod format;
mod fraction;
mod general;
mod number;
mod tokenizer;
mod types;

pub use builtins::{builtin_format_id, FormatPreset, BUILTIN_NUMBER_FORMATS, FORMAT_PRESETS};
pub use date::{date_to_serial, serial_to_date, UtcDateTime, MAX_DATE_SERIAL};
pub use decimals::step_decimals;
pub use format::{format_value, is_date_format, OVERFLOW_TEXT};
pub use general::format_general;
pub use types::{CellValue, FormatOptions, FormattedValue};
