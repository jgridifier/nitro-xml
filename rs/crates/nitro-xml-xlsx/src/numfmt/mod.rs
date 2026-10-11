// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/index.ts; changes listed in PROVENANCE.md.
//! Excel number formats: `format_value`, typed input parsing, and the pieces they are built from.
//!
//! Display text for a workbook cell lives in [`crate::display_text`].

mod builtins;
mod compile;
mod date;
mod decimal;
mod decimals;
mod format;
mod fraction;
mod general;
mod input;
mod input_date;
mod number;
mod tokenizer;
mod types;

pub use builtins::{builtin_format_id, FormatPreset, BUILTIN_NUMBER_FORMATS, FORMAT_PRESETS};
pub use date::{date_to_serial, serial_to_date, UtcDateTime, MAX_DATE_SERIAL};
pub use decimal::js_int_string;
pub use decimals::step_decimals;
pub use format::{format_value, is_date_format, OVERFLOW_TEXT};
pub use general::format_general;
pub use input::{parse_cell_input, ParsedInput};
pub use input_date::{parse_date_time_input, serial_of, DateInput};
pub use types::{CellValue, FormatOptions, FormattedValue};
