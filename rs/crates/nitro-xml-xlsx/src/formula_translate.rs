// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/transform.ts, tokenizer.ts,
// lexemes.ts, ref-text.ts; changes listed in PROVENANCE.md.
//! Shared-formula reference translation (`translateFormula`).

#[path = "formula_internal.rs"]
mod internal;

pub use internal::translate_formula;
