// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/operators.ts; changes listed in PROVENANCE.md.
//! Scalar operators and Excel's array broadcasting.

use crate::formula::ast::BinaryOp;

use super::coerce::{compare_scalars, to_number, to_text};
use super::value::{ErrorCode, Fx, Matrix, Scalar};

fn signal(result: Fx<Scalar>) -> Scalar {
    match result {
        Ok(value) => value,
        Err(code) => Scalar::Error(code),
    }
}

/// `Math.round`: half toward +∞, and the open interval (-0.5, 0) stays `-0`.
pub fn js_round(n: f64) -> f64 {
    if !n.is_finite() {
        return n;
    }
    if n < 0.5 && n > 0.0 {
        return 0.0;
    }
    if (-0.5..0.0).contains(&n) {
        return -0.0;
    }
    (n + 0.5).floor()
}

pub fn js_is_integer(n: f64) -> bool {
    n.is_finite() && n.fract() == 0.0
}

/// `Math.sign`, preserving `-0` and `NaN`.
pub fn js_sign(n: f64) -> f64 {
    if n.is_nan() || n == 0.0 {
        n
    } else if n > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// A non-zero, non-NaN number is truthy, matching JavaScript.
pub fn flag(n: f64) -> bool {
    n != 0.0 && !n.is_nan()
}

/// `(-8)^(1/3)` is -2 in Excel: odd roots of negatives are allowed. `POWER` shares this.
pub fn power(base: f64, exponent: f64) -> Fx<f64> {
    if base == 0.0 && exponent == 0.0 {
        return Err(ErrorCode::Num);
    }
    if base == 0.0 && exponent < 0.0 {
        return Err(ErrorCode::Div0);
    }
    if base < 0.0 && !js_is_integer(exponent) {
        let inverse = 1.0 / exponent;
        let odd = js_round(inverse);
        if (inverse - odd).abs() < 1e-10 && odd.abs() % 2.0 == 1.0 {
            return Ok(-(-base).powf(exponent));
        }
        return Err(ErrorCode::Num);
    }
    Ok(base.powf(exponent))
}

/// Excel snaps a final addition or subtraction that cancels within 15 digits to 0.
pub fn snap_to_zero(result: f64, a: f64, b: f64) -> f64 {
    if result != 0.0 && result.abs() <= a.abs().max(b.abs()) * 1e-15 {
        0.0
    } else {
        result
    }
}

fn arithmetic(op: BinaryOp, x: &Scalar, y: &Scalar, snap: bool, date1904: bool) -> Scalar {
    if let Scalar::Error(code) = x {
        return Scalar::Error(*code);
    }
    if let Scalar::Error(code) = y {
        return Scalar::Error(*code);
    }
    signal((|| {
        let a = to_number(x, date1904)?;
        let b = to_number(y, date1904)?;
        let mut r = match op {
            BinaryOp::Add => {
                let mut r = a + b;
                if snap {
                    r = snap_to_zero(r, a, b);
                }
                r
            }
            BinaryOp::Sub => {
                let mut r = a - b;
                if snap {
                    r = snap_to_zero(r, a, b);
                }
                r
            }
            BinaryOp::Mul => a * b,
            BinaryOp::Div => {
                if b == 0.0 {
                    return Ok(Scalar::Error(ErrorCode::Div0));
                }
                a / b
            }
            BinaryOp::Pow => power(a, b)?,
            _ => 0.0,
        };
        if r == 0.0 {
            r = 0.0;
        }
        if r.is_finite() {
            Ok(Scalar::Number(r))
        } else {
            Ok(Scalar::Error(ErrorCode::Num))
        }
    })())
}

fn comparison(op: BinaryOp, x: &Scalar, y: &Scalar) -> Scalar {
    if let Scalar::Error(code) = x {
        return Scalar::Error(*code);
    }
    if let Scalar::Error(code) = y {
        return Scalar::Error(*code);
    }
    let c = match compare_scalars(x, y) {
        Ok(c) => c,
        Err(code) => return Scalar::Error(code),
    };
    let bit = match op {
        BinaryOp::Eq => c == 0,
        BinaryOp::Ne => c != 0,
        BinaryOp::Lt => c < 0,
        BinaryOp::Gt => c > 0,
        BinaryOp::Le => c <= 0,
        _ => c >= 0,
    };
    Scalar::Bool(bit)
}

/// Applies a binary operator to two scalars.
pub fn scalar_binary(op: BinaryOp, x: &Scalar, y: &Scalar, snap: bool, date1904: bool) -> Scalar {
    if op == BinaryOp::Concat {
        if let Scalar::Error(code) = x {
            return Scalar::Error(*code);
        }
        if let Scalar::Error(code) = y {
            return Scalar::Error(*code);
        }
        return signal((|| {
            Ok(Scalar::Text(format!("{}{}", to_text(x)?, to_text(y)?)))
        })());
    }
    if matches!(
        op,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow
    ) {
        return arithmetic(op, x, y, snap, date1904);
    }
    comparison(op, x, y)
}

/// Unary minus and percent.
pub fn scalar_unary(percent: bool, x: &Scalar, date1904: bool) -> Scalar {
    if let Scalar::Error(code) = x {
        return Scalar::Error(*code);
    }
    signal((|| {
        let n = to_number(x, date1904)?;
        let mut r = if percent { n / 100.0 } else { -n };
        if r == 0.0 {
            r = 0.0;
        }
        Ok(Scalar::Number(r))
    })())
}

/// The element of a broadcast operand (`#N/A` outside a non-repeated side).
pub fn pick(matrix: &Matrix, row: usize, col: usize) -> Scalar {
    let r = if matrix.rows() == 1 { 0 } else { row };
    let c = if matrix.cols() == 1 { 0 } else { col };
    if r >= matrix.rows() || c >= matrix.cols() {
        Scalar::Error(ErrorCode::Na)
    } else {
        matrix.get(r, c)
    }
}

/// Elementwise result over broadcast operands. Padded operands keep a uniform tail.
pub fn broadcast_shape(
    operands: &[Matrix],
    mut at: impl FnMut(usize, usize) -> Scalar,
) -> Fx<Matrix> {
    let mut rows = 1;
    let mut cols = 1;
    for matrix in operands {
        rows = rows.max(matrix.rows());
        cols = cols.max(matrix.cols());
    }
    if operands.iter().all(|matrix| !matrix.is_padded()) {
        return Matrix::build(rows, cols, |r, c| Ok(at(r, c)));
    }
    let mut block_rows = 1;
    let mut block_cols = 1;
    for matrix in operands {
        block_rows = block_rows.max(if matrix.is_padded() {
            matrix.block_rows()
        } else {
            matrix.rows()
        });
        block_cols = block_cols.max(if matrix.is_padded() {
            matrix.block_cols()
        } else {
            matrix.cols()
        });
    }
    block_rows = block_rows.min(rows);
    block_cols = block_cols.min(cols);
    let varies = operands.iter().any(|matrix| {
        (block_rows < rows && matrix.rows() == 1 && matrix.cols() > 1)
            || (block_cols < cols && matrix.cols() == 1 && matrix.rows() > 1)
    });
    if varies {
        return Matrix::build(rows, cols, |r, c| Ok(at(r, c)));
    }
    let block = Matrix::build(block_rows, block_cols, |r, c| Ok(at(r, c)))?;
    let fill = at(rows.saturating_sub(1), cols.saturating_sub(1));
    Ok(Matrix::padded(block, rows, cols, fill))
}

/// Broadcasts a binary scalar function over matrices.
pub fn broadcast2(
    a: &ScalarOrMatrix,
    b: &ScalarOrMatrix,
    mut func: impl FnMut(&Scalar, &Scalar) -> Scalar,
) -> Fx<ScalarOrMatrix> {
    match (a, b) {
        (ScalarOrMatrix::Scalar(x), ScalarOrMatrix::Scalar(y)) => {
            Ok(ScalarOrMatrix::Scalar(func(x, y)))
        }
        _ => {
            let ma = a.as_matrix();
            let mb = b.as_matrix();
            let out = broadcast_shape(&[ma.clone(), mb.clone()], |r, c| {
                func(&pick(&ma, r, c), &pick(&mb, r, c))
            })?;
            Ok(ScalarOrMatrix::Matrix(out))
        }
    }
}

/// A scalar or a matrix, the two shapes arithmetic broadcasts.
#[derive(Clone, Debug)]
pub enum ScalarOrMatrix {
    Scalar(Scalar),
    Matrix(Matrix),
}

impl ScalarOrMatrix {
    fn as_matrix(&self) -> Matrix {
        match self {
            Self::Matrix(matrix) => matrix.clone(),
            Self::Scalar(scalar) => Matrix::from_rows(vec![vec![scalar.clone()]]),
        }
    }
}
