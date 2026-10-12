// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/registry.ts
// (Math & Trig and Statistical entries) and src/core/xlsx/formula/call.ts;
// changes listed in PROVENANCE.md.
//! Math & Trig and Statistical functions (#73).
//! Other families are `TODO(#74)`. The workbook calc graph is `TODO(#72)`.

mod coerce;
mod criteria;
mod dist;
mod eval;
mod helpers;
mod host;
mod math;
mod ops;
mod spec;
mod stat;
mod stats_core;
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
        for spec in math::specs().into_iter().chain(stat::specs()) {
            map.insert(spec.name.to_ascii_uppercase(), spec);
        }
        map
    })
}

/// Upper-case Math & Trig or Statistical function, if this crate implements it.
pub fn get_function(name: &str) -> Option<&'static FunctionSpec> {
    registry().get(&name.to_ascii_uppercase())
}
