// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/types.ts; changes listed in PROVENANCE.md.
//! Function specifications for the math and statistical families.

use super::host::CallCtx;
use super::value::{Fx, Value};

/// `value` parameters lift over arrays. `any` parameters receive ranges unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    Value,
    Any,
}

/// How a registered function is implemented.
#[derive(Clone, Copy)]
pub enum FuncKind {
    /// Every argument coerces to a number. Omitted slots use `defaults` when present.
    Numeric {
        defaults: &'static [Option<f64>],
        f: fn(&[f64]) -> Fx<f64>,
    },
    General {
        f: fn(&CallCtx<'_>, &[Value]) -> Fx<Value>,
    },
}

/// One Excel function. Names are stored in upper case.
#[derive(Clone, Copy)]
pub struct FunctionSpec {
    pub name: &'static str,
    pub category: &'static str,
    pub syntax: &'static str,
    pub description: &'static str,
    pub min_args: usize,
    pub max_args: usize,
    /// Per position; the last entry repeats. Empty means every parameter is `value`.
    pub params: &'static [ParamKind],
    pub volatile: bool,
    pub kind: FuncKind,
}

pub fn param_kind(spec: &FunctionSpec, index: usize) -> ParamKind {
    if spec.params.is_empty() {
        ParamKind::Value
    } else {
        spec.params[index.min(spec.params.len() - 1)]
    }
}

#[allow(clippy::too_many_arguments)]
pub fn num_spec(
    name: &'static str,
    category: &'static str,
    syntax: &'static str,
    description: &'static str,
    min_args: usize,
    max_args: usize,
    defaults: &'static [Option<f64>],
    f: fn(&[f64]) -> Fx<f64>,
) -> FunctionSpec {
    FunctionSpec {
        name,
        category,
        syntax,
        description,
        min_args,
        max_args,
        params: &[],
        volatile: false,
        kind: FuncKind::Numeric { defaults, f },
    }
}

#[allow(clippy::too_many_arguments)]
pub fn gen_spec(
    name: &'static str,
    category: &'static str,
    syntax: &'static str,
    description: &'static str,
    min_args: usize,
    max_args: usize,
    params: &'static [ParamKind],
    volatile: bool,
    f: fn(&CallCtx<'_>, &[Value]) -> Fx<Value>,
) -> FunctionSpec {
    FunctionSpec {
        name,
        category,
        syntax,
        description,
        min_args,
        max_args,
        params,
        volatile,
        kind: FuncKind::General { f },
    }
}
