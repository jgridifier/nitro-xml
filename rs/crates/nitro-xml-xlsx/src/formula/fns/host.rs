// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/context.ts, call.ts (`forEach` /
// `toMatrix` / `toScalar`) and references.ts (`areaToMatrix`, implicit intersection);
// changes listed in PROVENANCE.md.
//! The slice of the calc host that math and statistical functions read.

use super::value::{
    check_array_size, Area, ErrorCode, Fx, Matrix, Scalar, Value, VisitKind, MAX_ARRAY_CELLS,
};

/// Cells a function can read. The grid behind this is `TODO(#72)`'s calc engine; the harness
/// supplies a minimal sheet.
pub trait EvalHost {
    fn read_cell(&self, sheet: usize, row: i32, col: i32) -> Scalar;
    fn for_each_stored(&self, sheet: usize, area: Area, visit: &mut dyn FnMut(Scalar, i32, i32));
    /// One past the last stored row and column (`0` when the sheet is empty).
    fn bounds(&self, sheet: usize) -> (i32, i32);
    fn cell_formula(&self, sheet: usize, row: i32, col: i32) -> Option<String>;
    fn sheet_count(&self) -> usize;
    fn sheet_name(&self, index: usize) -> Option<&str>;
    fn find_sheet(&self, name: &str) -> Option<usize>;
    fn random(&self) -> f64;
    fn row_hidden(&self, sheet: usize, row: i32) -> bool;
    fn occupied(&self, sheet: usize, row: i32, col: i32) -> bool;
}

/// What a function implementation sees: position, date system, and the host.
pub struct CallCtx<'a> {
    pub host: &'a dyn EvalHost,
    pub sheet: usize,
    pub row: i32,
    pub col: i32,
    /// Dynamic-array intersection. The harness leaves this false (`TODO(#72)` legacy formulas).
    pub legacy: bool,
    pub date1904: bool,
    pub depth: u32,
}

impl<'a> CallCtx<'a> {
    pub fn read_cell(&self, sheet: usize, row: i32, col: i32) -> Scalar {
        self.host.read_cell(sheet, row, col)
    }

    pub fn random(&self) -> f64 {
        self.host.random()
    }

    pub fn for_each(&self, value: &Value, visit: &mut dyn FnMut(&Scalar, VisitKind)) {
        match value {
            Value::Ref(reference) => {
                for area in &reference.areas {
                    self.host
                        .for_each_stored(area.sheet, *area, &mut |value, _, _| {
                            visit(&value, VisitKind::Ref);
                        });
                }
            }
            Value::Matrix(matrix) => matrix.for_each_value(|value| visit(value, VisitKind::Array)),
            Value::Scalar(scalar) => visit(scalar, VisitKind::Direct),
        }
    }

    pub fn to_matrix(&self, value: &Value) -> Fx<Matrix> {
        match value {
            Value::Matrix(matrix) => Ok(matrix.clone()),
            Value::Scalar(scalar) => Ok(Matrix::from_rows(vec![vec![scalar.clone()]])),
            Value::Ref(reference) => {
                let Some(area) = reference.areas.first() else {
                    return Err(ErrorCode::Value);
                };
                if reference.areas.len() != 1 {
                    return Err(ErrorCode::Value);
                }
                area_to_matrix(self.host, *area)
            }
        }
    }

    pub fn to_scalar(&self, value: &Value) -> Scalar {
        implicit_intersection(self, value)
    }
}

const CLIP_THRESHOLD: i64 = 100_000;
const MAX_MATRIX_CELLS: i64 = 4_000_000;

fn span(start: i32, end: i32) -> i64 {
    (end as i64 - start as i64 + 1).max(0)
}

/// Reads an area. Large areas keep only the used block and pad the rest with blanks.
pub fn area_to_matrix(host: &dyn EvalHost, area: Area) -> Fx<Matrix> {
    let rows = span(area.start_row, area.end_row);
    let cols = span(area.start_col, area.end_col);
    let mut end_row = area.end_row;
    let mut end_col = area.end_col;
    if rows * cols > CLIP_THRESHOLD {
        let (bound_rows, bound_cols) = host.bounds(area.sheet);
        end_row = area
            .start_row
            .max(end_row.min(bound_rows.saturating_sub(1)));
        end_col = area
            .start_col
            .max(end_col.min(bound_cols.saturating_sub(1)));
    }
    let block_rows = span(area.start_row, end_row);
    let block_cols = span(area.start_col, end_col);
    if block_rows * block_cols > MAX_MATRIX_CELLS {
        return Err(ErrorCode::Num);
    }
    let block_rows_us = usize::try_from(block_rows).unwrap_or(0);
    let block_cols_us = usize::try_from(block_cols).unwrap_or(0);
    let block = Matrix::build(block_rows_us, block_cols_us, |r, c| {
        Ok(host.read_cell(
            area.sheet,
            area.start_row + r as i32,
            area.start_col + c as i32,
        ))
    })?;
    if rows * cols > MAX_ARRAY_CELLS as i64 {
        return Ok(block);
    }
    let rows_us = usize::try_from(rows).unwrap_or(0);
    let cols_us = usize::try_from(cols).unwrap_or(0);
    check_array_size(rows_us, cols_us)?;
    Ok(Matrix::padded(block, rows_us, cols_us, Scalar::Blank))
}

/// Implicit intersection with the formula cell. Arrays contribute their first element.
pub fn implicit_intersection(ctx: &CallCtx<'_>, value: &Value) -> Scalar {
    match value {
        Value::Matrix(matrix) => matrix.get(0, 0),
        Value::Scalar(scalar) => scalar.clone(),
        Value::Ref(reference) => {
            let Some(area) = reference.areas.first().copied() else {
                return Scalar::Error(ErrorCode::Value);
            };
            if reference.areas.len() != 1 {
                return Scalar::Error(ErrorCode::Value);
            }
            if area.is_cell() {
                return ctx.read_cell(area.sheet, area.start_row, area.start_col);
            }
            let row_inside = ctx.row >= area.start_row && ctx.row <= area.end_row;
            let col_inside = ctx.col >= area.start_col && ctx.col <= area.end_col;
            if area.start_col == area.end_col && row_inside {
                ctx.read_cell(area.sheet, ctx.row, area.start_col)
            } else if area.start_row == area.end_row && col_inside {
                ctx.read_cell(area.sheet, area.start_row, ctx.col)
            } else if row_inside && col_inside {
                ctx.read_cell(area.sheet, ctx.row, ctx.col)
            } else {
                Scalar::Error(ErrorCode::Value)
            }
        }
    }
}
