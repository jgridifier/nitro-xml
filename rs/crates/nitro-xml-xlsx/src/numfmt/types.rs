// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/types.ts and the CellValue/isCellError slice of src/core/xlsx/model.ts; changes listed in PROVENANCE.md.

use std::fmt;

/// A cell value the number formatter accepts.
///
/// Dates are numbers (serial days). This is the slice of the workbook `CellValue` that formatting
/// needs; the workbook model itself is not part of this crate yet.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Blank,
    Number(f64),
    Text(String),
    Bool(bool),
    /// Display text of an error cell (`#DIV/0!`, `#N/A`, ...).
    Error(String),
}

/// Display result of formatting a value with a number format code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedValue {
    pub text: String,
    /// `#RRGGBB` from a colour token (`[Red]`, `[Color10]`) of the section that was used.
    pub color: Option<String>,
}

impl FormattedValue {
    pub(crate) fn new(text: impl Into<String>, color: Option<String>) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
}

/// Options for [`crate::numfmt::format_value`].
#[derive(Default)]
pub struct FormatOptions<'a> {
    /// Workbook uses the 1904 date system (serial 0 is 1904-01-01).
    pub date1904: bool,
    /// Room the cell has for a number. Without `measure` it counts characters; with `measure` it
    /// is in the units `measure` returns. Absent: no fitting (the 11-character General).
    pub width: Option<f64>,
    /// Measures a candidate text in the same units as `width`.
    pub measure: Option<&'a dyn Fn(&str) -> f64>,
}

impl fmt::Debug for FormatOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FormatOptions")
            .field("date1904", &self.date1904)
            .field("width", &self.width)
            .field("measure", &self.measure.is_some())
            .finish()
    }
}

pub(crate) type DigitChar = char;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DatePart {
    Y,
    Yyyy,
    E,
    G,
    B,
    Bbbb,
    M,
    Mm,
    Mmm,
    Mmmm,
    Mmmmm,
    D,
    Dd,
    Ddd,
    Dddd,
    H,
    Hh,
    Min,
    Mmin,
    S,
    Ss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AmpmKind {
    AmPm,
    Upper,
    Lower,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    Lit(String),
    Digit(DigitChar),
    Num(String),
    Point,
    Comma,
    Percent,
    Exp { plus: bool },
    Slash,
    Text,
    General,
    Date(DatePart),
    Elapsed { unit: char, width: usize },
    Ampm(AmpmKind),
    Subsec { digits: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConditionOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Condition {
    pub op: ConditionOp,
    pub value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SectionKind {
    Number,
    Date,
    Text,
    General,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Section {
    pub tokens: Vec<Token>,
    pub kind: SectionKind,
    pub color: Option<String>,
    pub condition: Option<Condition>,
}
