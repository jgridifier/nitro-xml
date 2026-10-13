// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/registry.ts
// and src/core/xlsx/formula/call.ts; changes listed in PROVENANCE.md.
//! Formula functions. Math & Trig and Statistical are #73. Logical, text, information
//! and date & time are #74. Lookup & Reference (including dynamic arrays), Financial
//! and Engineering are #75. The 12 Database functions (`DAVERAGE` … `DVARP`) are the
//! only catalog gap; the next follow-up registers them. See `docs/formula-coverage.md`.

mod array;
mod coerce;
mod criteria;
mod datetime;
mod dist;
mod engineering;
mod eval;
mod financial;
mod helpers;
mod host;
mod info;
mod logical;
mod lookup;
mod math;
mod ops;
mod spec;
mod stat;
mod stats_core;
mod text;
mod text_num;
mod value;

pub use eval::{call_function, dump_fn_cases, evaluate_formula, GridHost};
pub use host::{CallCtx, EvalHost};
pub use spec::{param_kind, FuncKind, FunctionSpec, ParamKind};
pub use value::{Area, ErrorCode, LambdaValue, Matrix, RefValue, Scalar, Value};

/// Wall-clock serial the function harness and the calc engine share (2026-10-03 12:00).
pub(crate) fn pinned_now_serial(date1904: bool) -> f64 {
    text_num::ymd_to_serial(2026, 10, 3, date1904).unwrap_or(0.0) + 0.5
}

use std::collections::HashMap;
use std::sync::OnceLock;

fn registry() -> &'static HashMap<String, FunctionSpec> {
    static REGISTRY: OnceLock<HashMap<String, FunctionSpec>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut map = HashMap::new();
        for spec in math::specs()
            .into_iter()
            .chain(stat::specs())
            .chain(logical::specs())
            .chain(text::specs())
            .chain(info::specs())
            .chain(datetime::specs())
            .chain(lookup::specs())
            .chain(array::specs())
            .chain(financial::specs())
            .chain(engineering::specs())
        {
            map.insert(spec.name.to_ascii_uppercase(), spec);
        }
        map
    })
}

/// Upper-case function this crate implements.
pub fn get_function(name: &str) -> Option<&'static FunctionSpec> {
    registry().get(&name.to_ascii_uppercase())
}

/// Registered `(name, category)` pairs, sorted by name. Database functions are absent.
pub fn registered_functions() -> Vec<(&'static str, &'static str)> {
    let mut rows: Vec<(&'static str, &'static str)> = registry()
        .values()
        .map(|spec| (spec.name, spec.category))
        .collect();
    rows.sort_by_key(|row| row.0);
    rows
}
