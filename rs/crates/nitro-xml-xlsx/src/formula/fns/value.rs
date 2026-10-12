// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/values.ts; changes listed in PROVENANCE.md.
//! Scalars, errors, and matrices as Excel formula functions see them.

use serde_json::{json, Value as Json};

/// Excel error codes formulas return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    Null,
    Div0,
    Value,
    Ref,
    Name,
    Num,
    Na,
    Spill,
    Calc,
}

impl ErrorCode {
    pub fn as_excel(self) -> &'static str {
        match self {
            Self::Null => "#NULL!",
            Self::Div0 => "#DIV/0!",
            Self::Value => "#VALUE!",
            Self::Ref => "#REF!",
            Self::Name => "#NAME?",
            Self::Num => "#NUM!",
            Self::Na => "#N/A",
            Self::Spill => "#SPILL!",
            Self::Calc => "#CALC!",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        Some(match code {
            "#NULL!" => Self::Null,
            "#DIV/0!" => Self::Div0,
            "#VALUE!" => Self::Value,
            "#REF!" => Self::Ref,
            "#NAME?" => Self::Name,
            "#NUM!" => Self::Num,
            "#N/A" => Self::Na,
            "#SPILL!" => Self::Spill,
            "#CALC!" => Self::Calc,
            _ => return None,
        })
    }
}

pub type Fx<T> = Result<T, ErrorCode>;

/// A single value. `Blank` is an empty cell or an omitted argument.
#[derive(Clone, Debug)]
pub enum Scalar {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(ErrorCode),
    Blank,
}

impl PartialEq for Scalar {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Error(a), Self::Error(b)) => a == b,
            (Self::Blank, Self::Blank) => true,
            _ => false,
        }
    }
}

impl Scalar {
    pub fn error(code: ErrorCode) -> Self {
        Self::Error(code)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    pub fn json(&self) -> Json {
        match self {
            Self::Number(n) => {
                if !n.is_finite() {
                    return json!({ "error": ErrorCode::Num.as_excel() });
                }
                serde_json::Number::from_f64(*n)
                    .map(Json::Number)
                    .unwrap_or(Json::Null)
            }
            Self::Text(text) => Json::String(text.clone()),
            Self::Bool(value) => Json::Bool(*value),
            Self::Error(code) => json!({ "error": code.as_excel() }),
            Self::Blank => Json::Null,
        }
    }
}

/// The value a formula cell shows: blanks become 0, non-finite numbers `#NUM!`, `-0` becomes 0.
pub fn clean(value: Scalar) -> Scalar {
    match value {
        Scalar::Blank => Scalar::Number(0.0),
        Scalar::Number(n) if !n.is_finite() => Scalar::Error(ErrorCode::Num),
        Scalar::Number(n) => Scalar::Number(if n == 0.0 { 0.0 } else { n }),
        other => other,
    }
}

/// Excel's longest array side.
pub const MAX_ARRAY_SIDE: usize = 1_048_576;
/// The most cells one array may hold (`#NUM!` past this).
pub const MAX_ARRAY_CELLS: usize = 16_777_216;

pub fn check_array_size(rows: usize, cols: usize) -> Fx<()> {
    if rows > MAX_ARRAY_SIDE || cols > MAX_ARRAY_SIDE {
        return Err(ErrorCode::Value);
    }
    if rows.saturating_mul(cols) > MAX_ARRAY_CELLS {
        return Err(ErrorCode::Num);
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Padding {
    rows: usize,
    cols: usize,
    fill: Scalar,
}

/// A two-dimensional array. A padded matrix stores a top-left block and reports a larger size.
#[derive(Clone, Debug)]
pub struct Matrix {
    dense: Vec<Vec<Scalar>>,
    padding: Option<Padding>,
}

impl Matrix {
    pub fn from_rows(dense: Vec<Vec<Scalar>>) -> Self {
        Self {
            dense,
            padding: None,
        }
    }

    pub fn padded(block: Matrix, rows: usize, cols: usize, fill: Scalar) -> Self {
        let stored_rows = block.dense.len();
        let stored_cols = block.dense.first().map(|row| row.len()).unwrap_or(0);
        let padding = if rows > stored_rows || cols > stored_cols {
            Some(Padding { rows, cols, fill })
        } else {
            None
        };
        Self {
            dense: block.dense,
            padding,
        }
    }

    pub fn build(
        rows: usize,
        cols: usize,
        mut fill: impl FnMut(usize, usize) -> Fx<Scalar>,
    ) -> Fx<Self> {
        check_array_size(rows, cols)?;
        let mut dense = Vec::with_capacity(rows);
        for r in 0..rows {
            let mut line = Vec::with_capacity(cols);
            for c in 0..cols {
                line.push(fill(r, c)?);
            }
            dense.push(line);
        }
        Ok(Self::from_rows(dense))
    }

    pub fn rows(&self) -> usize {
        self.padding
            .as_ref()
            .map(|pad| pad.rows)
            .unwrap_or(self.dense.len())
    }

    pub fn cols(&self) -> usize {
        self.padding
            .as_ref()
            .map(|pad| pad.cols)
            .unwrap_or(self.dense.first().map(|row| row.len()).unwrap_or(0))
    }

    pub fn is_padded(&self) -> bool {
        self.padding.is_some()
    }

    pub fn block_rows(&self) -> usize {
        self.dense.len()
    }

    pub fn block_cols(&self) -> usize {
        self.dense.first().map(|row| row.len()).unwrap_or(0)
    }

    pub fn get(&self, row: usize, col: usize) -> Scalar {
        if let Some(line) = self.dense.get(row) {
            if let Some(value) = line.get(col) {
                return value.clone();
            }
        }
        if let Some(pad) = &self.padding {
            if row < pad.rows && col < pad.cols {
                return pad.fill.clone();
            }
        }
        Scalar::Blank
    }

    pub fn block(&self) -> Matrix {
        if self.padding.is_some() {
            Matrix::from_rows(self.dense.clone())
        } else {
            self.clone()
        }
    }

    pub fn data(&self) -> Fx<Vec<Vec<Scalar>>> {
        let Some(pad) = &self.padding else {
            return Ok(self.dense.clone());
        };
        check_array_size(pad.rows, pad.cols)?;
        let mut out = Vec::with_capacity(pad.rows);
        for r in 0..pad.rows {
            let mut line = Vec::with_capacity(pad.cols);
            for c in 0..pad.cols {
                line.push(self.get(r, c));
            }
            out.push(line);
        }
        Ok(out)
    }

    pub fn map(&self, mut func: impl FnMut(&Scalar, usize, usize) -> Scalar) -> Fx<Matrix> {
        let data = self.data()?;
        let mut rows = Vec::with_capacity(data.len());
        for (r, line) in data.into_iter().enumerate() {
            let mut out = Vec::with_capacity(line.len());
            for (c, value) in line.into_iter().enumerate() {
                out.push(func(&value, r, c));
            }
            rows.push(out);
        }
        Ok(Matrix::from_rows(rows))
    }

    pub fn map_values(&self, mut func: impl FnMut(&Scalar) -> Scalar) -> Matrix {
        let dense = self
            .dense
            .iter()
            .map(|line| line.iter().map(&mut func).collect())
            .collect();
        let padding = self.padding.as_ref().map(|pad| Padding {
            rows: pad.rows,
            cols: pad.cols,
            fill: func(&pad.fill),
        });
        Matrix { dense, padding }
    }

    pub fn for_each_value(&self, mut visit: impl FnMut(&Scalar)) {
        let Some(pad) = &self.padding else {
            for line in &self.dense {
                for value in line {
                    visit(value);
                }
            }
            return;
        };
        for r in 0..pad.rows {
            let stored = self.dense.get(r);
            let n = stored.map(|line| line.len()).unwrap_or(0);
            for c in 0..n {
                visit(
                    stored
                        .and_then(|line| line.get(c))
                        .unwrap_or(&Scalar::Blank),
                );
            }
            for _c in n..pad.cols {
                visit(&pad.fill);
            }
        }
    }

    pub fn flat(&self) -> Fx<Vec<Scalar>> {
        if self.padding.is_none() {
            return Ok(self.dense.iter().flatten().cloned().collect());
        }
        check_array_size(self.rows(), self.cols())?;
        let mut out = Vec::new();
        self.for_each_value(|value| out.push(value.clone()));
        Ok(out)
    }
}

/// One rectangular area of a reference. Rows and columns are zero-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub sheet: usize,
    pub start_row: i32,
    pub start_col: i32,
    pub end_row: i32,
    pub end_col: i32,
}

impl Area {
    pub fn normalize(sheet: usize, r1: i32, c1: i32, r2: i32, c2: i32) -> Self {
        Self {
            sheet,
            start_row: r1.min(r2),
            start_col: c1.min(c2),
            end_row: r1.max(r2),
            end_col: c1.max(c2),
        }
    }

    pub fn rows(self) -> usize {
        (self.end_row - self.start_row + 1).max(0) as usize
    }

    pub fn cols(self) -> usize {
        (self.end_col - self.start_col + 1).max(0) as usize
    }

    pub fn is_cell(self) -> bool {
        self.start_row == self.end_row && self.start_col == self.end_col
    }
}

/// A reference to one or more areas.
#[derive(Clone, Debug)]
pub struct RefValue {
    pub areas: Vec<Area>,
}

impl RefValue {
    pub fn is_cell(&self) -> bool {
        self.areas.len() == 1 && self.areas[0].is_cell()
    }
}

/// Anything a formula expression evaluates to. Lambdas are `#CALC!` (`TODO(#74)`).
#[derive(Clone, Debug)]
pub enum Value {
    Scalar(Scalar),
    Matrix(Matrix),
    Ref(RefValue),
}

impl Value {
    pub fn number(n: f64) -> Self {
        Self::Scalar(Scalar::Number(n))
    }

    pub fn error(code: ErrorCode) -> Self {
        Self::Scalar(Scalar::Error(code))
    }

    pub fn blank() -> Self {
        Self::Scalar(Scalar::Blank)
    }

    pub fn as_scalar(&self) -> Option<&Scalar> {
        match self {
            Self::Scalar(scalar) => Some(scalar),
            _ => None,
        }
    }
}

/// How an argument was supplied to an aggregate (`SUM`, `COUNT`, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisitKind {
    Direct,
    Ref,
    Array,
}
