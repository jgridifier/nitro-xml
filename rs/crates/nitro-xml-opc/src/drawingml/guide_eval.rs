// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/geometry/guide-formula-eval.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

pub const ANGLE_SCALE: f64 = 60_000.0;

pub fn angle_to_radians(ooxml_angle: f64) -> f64 {
    (ooxml_angle / ANGLE_SCALE) * (std::f64::consts::PI / 180.0)
}

#[derive(Debug, Clone)]
pub struct ParsedFormula {
    pub op: String,
    pub args: Vec<String>,
}

pub fn parse_formula(fmla: &str) -> ParsedFormula {
    let parts: Vec<&str> = fmla.split_whitespace().collect();
    let op = parts.first().map(|s| s.to_string()).unwrap_or_default();
    let args = parts.iter().skip(1).map(|s| s.to_string()).collect();
    ParsedFormula { op, args }
}

pub fn resolve_operand(token: &str, variables: &HashMap<String, f64>) -> f64 {
    if let Ok(num) = token.parse::<f64>() {
        if num.is_finite() {
            return num;
        }
    }
    variables.get(token).copied().unwrap_or(0.0)
}

pub fn evaluate_formula(parsed: &ParsedFormula, vars: &HashMap<String, f64>) -> f64 {
    let r = |idx: usize| {
        resolve_operand(
            parsed.args.get(idx).map(|s| s.as_str()).unwrap_or("0"),
            vars,
        )
    };

    match parsed.op.as_str() {
        "val" => r(0),
        "abs" => r(0).abs(),
        "sqrt" => r(0).max(0.0).sqrt(),
        "+-" => r(0) + r(1) - r(2),
        "*/" => {
            let z = r(2);
            if z == 0.0 {
                0.0
            } else {
                (r(0) * r(1)) / z
            }
        }
        "+/" => {
            let z = r(2);
            if z == 0.0 {
                0.0
            } else {
                (r(0) + r(1)) / z
            }
        }
        "?:" | "if" => {
            if r(0) > 0.0 {
                r(1)
            } else {
                r(2)
            }
        }
        "min" => r(0).min(r(1)),
        "max" => r(0).max(r(1)),
        "mod" => {
            let a = r(0);
            let b = r(1);
            let c = r(2);
            (a * a + b * b + c * c).sqrt()
        }
        "pin" => {
            let lo = r(0);
            let val = r(1);
            let hi = r(2);
            val.max(lo).min(hi)
        }
        "sin" => {
            let sin_val = (angle_to_radians(r(1))).sin();
            if !sin_val.is_finite() {
                0.0
            } else {
                r(0) * sin_val
            }
        }
        "cos" => {
            let cos_val = (angle_to_radians(r(1))).cos();
            if !cos_val.is_finite() {
                0.0
            } else {
                r(0) * cos_val
            }
        }
        "tan" => {
            let tan_val = (angle_to_radians(r(1))).tan();
            if !tan_val.is_finite() {
                0.0
            } else {
                r(0) * tan_val
            }
        }
        "atan" => {
            let radians = r(0).atan();
            (radians * 180.0 * ANGLE_SCALE) / std::f64::consts::PI
        }
        "at2" | "atan2" => {
            let radians = r(1).atan2(r(0));
            (radians * 180.0 * ANGLE_SCALE) / std::f64::consts::PI
        }
        "cat2" => {
            let angle = r(2).atan2(r(1));
            r(0) * angle.cos()
        }
        "sat2" => {
            let angle = r(2).atan2(r(1));
            r(0) * angle.sin()
        }
        _ => parsed.op.parse::<f64>().unwrap_or(0.0),
    }
}
