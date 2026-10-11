// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/geometry/guide-formula-api.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

use super::guide_eval::{evaluate_formula, parse_formula, resolve_operand, ANGLE_SCALE};

#[derive(Debug, Clone)]
pub struct GeometryGuide {
    pub name: String,
    pub formula: String,
}

#[derive(Debug, Clone, Copy)]
pub struct GeometryContext {
    pub w: f64,
    pub h: f64,
}

pub fn create_builtin_variables(ctx: GeometryContext) -> HashMap<String, f64> {
    let w = ctx.w;
    let h = ctx.h;
    let mut vars = HashMap::new();
    vars.insert("w".into(), w);
    vars.insert("h".into(), h);
    vars.insert("l".into(), 0.0);
    vars.insert("t".into(), 0.0);
    vars.insert("r".into(), w);
    vars.insert("b".into(), h);
    vars.insert("hc".into(), w / 2.0);
    vars.insert("vc".into(), h / 2.0);

    for d in [2, 3, 4, 5, 6, 8, 10, 12, 16, 32] {
        vars.insert(format!("wd{d}"), w / d as f64);
        vars.insert(format!("hd{d}"), h / d as f64);
    }

    let ss = w.min(h);
    let ls = w.max(h);
    vars.insert("ss".into(), ss);
    vars.insert("ls".into(), ls);
    for d in [2, 4, 6, 8, 16, 32] {
        vars.insert(format!("ssd{d}"), ss / d as f64);
    }

    vars.insert("cd2".into(), 180.0 * ANGLE_SCALE);
    vars.insert("cd4".into(), 90.0 * ANGLE_SCALE);
    vars.insert("cd8".into(), 45.0 * ANGLE_SCALE);
    vars.insert("3cd4".into(), 270.0 * ANGLE_SCALE);
    vars.insert("3cd8".into(), 135.0 * ANGLE_SCALE);
    vars.insert("5cd8".into(), 225.0 * ANGLE_SCALE);
    vars.insert("7cd8".into(), 315.0 * ANGLE_SCALE);
    vars
}

pub fn evaluate_guides(
    guides: &[GeometryGuide],
    ctx: GeometryContext,
    adjustments: Option<&HashMap<String, f64>>,
) -> HashMap<String, f64> {
    let mut vars = create_builtin_variables(ctx);
    if let Some(adj) = adjustments {
        for (name, value) in adj {
            vars.insert(name.clone(), *value);
        }
    }
    for guide in guides {
        let parsed = parse_formula(&guide.formula);
        let value = evaluate_formula(&parsed, &vars);
        vars.insert(
            guide.name.clone(),
            if value.is_finite() { value } else { 0.0 },
        );
    }
    vars
}

#[allow(dead_code)]
pub fn resolve_coordinate(value: &str, variables: &HashMap<String, f64>) -> f64 {
    let trimmed = value.trim();
    if let Ok(num) = trimmed.parse::<f64>() {
        if num.is_finite() {
            return num;
        }
    }
    resolve_operand(trimmed, variables)
}
