// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/registry.ts
// and src/core/xlsx/formula/call.ts; changes listed in PROVENANCE.md.
//! Formula functions. Math & Trig and Statistical are #73. Logical, text, information
//! and date & time are #74. Lookup & Reference (including dynamic arrays), Financial
//! and Engineering are #75. Database functions (DAVERAGE … DVARP) are `TODO(#75-followup)`.
//! The workbook calc graph is `TODO(#72)`.

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
pub use host::CallCtx;
pub use spec::FunctionSpec;
pub use value::{ErrorCode, Scalar, Value};

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
