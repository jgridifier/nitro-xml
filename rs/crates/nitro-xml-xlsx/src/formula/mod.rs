// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/{ast,tokenizer,lexemes,parser,structured,ref-text}.ts
// and referencedRanges/referenceSpans in transform.ts; changes listed in PROVENANCE.md.
//! Excel formula parser: tokens, AST, structured refs, and direct reference spans.
//! Evaluation, the calc graph, and the function library are later issues (#72+).

mod ast;
mod lexer;
mod parser;
mod refs;

pub use ast::{
    normalize_function_name, normalize_local_name, ArrayConstant, BinaryOp, FormulaAst,
    FormulaError, RefCorner, RefKind, RefSpec, SheetPrefix, StructuredRef, StructuredSpecial,
    UnaryOp,
};
pub use lexer::{join_tokens, number_literal, tokenize, Token, TokenKind, TokenValue};
pub use parser::{parse_formula, parse_tokens, MAX_FUNCTION_NESTING};
pub use refs::{
    format_ref_spec, parse_structured, prefix_text, reference_spans, referenced_ranges,
    rewrite_ref, same_spec, ReferenceSpan, ReferencedRange,
};

use serde_json::{Map, Value};

/// `JSON.stringify` key order of the TypeScript formula nodes. `serde_json::Map` is a
/// `BTreeMap` unless `preserve_order` is on, and that feature would reorder every other
/// dump in the workspace, so the formula dump sorts keys explicitly.
const DUMP_KEY_ORDER: &[&str] = &[
    "formula",
    "join",
    "parseError",
    "ast",
    "refs",
    "spans",
    "message",
    "position",
    "type",
    "name",
    "rawName",
    "callee",
    "args",
    "op",
    "left",
    "right",
    "operand",
    "prefix",
    "ref",
    "spill",
    "kind",
    "start",
    "end",
    "text",
    "sheet",
    "sheet2",
    "book",
    "row",
    "col",
    "rowAbs",
    "colAbs",
    "specials",
    "table",
    "column",
    "column2",
    "value",
    "rows",
    "error",
    "code",
    "range",
];

fn dump_key_rank(key: &str) -> usize {
    DUMP_KEY_ORDER
        .iter()
        .position(|candidate| *candidate == key)
        .unwrap_or(DUMP_KEY_ORDER.len())
}

/// Serialize a formula dump the way `JSON.stringify` emits the TypeScript oracle.
pub fn stringify_formula_dump(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        Value::Number(n) => serde_json::to_string(n).unwrap_or_else(|_| "null".to_owned()),
        Value::String(s) => serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_owned()),
        Value::Array(items) => {
            let body = items
                .iter()
                .map(stringify_formula_dump)
                .collect::<Vec<_>>()
                .join(",");
            format!("[{body}]")
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| {
                dump_key_rank(a)
                    .cmp(&dump_key_rank(b))
                    .then_with(|| a.cmp(b))
            });
            let body = keys
                .into_iter()
                .map(|key| {
                    let rendered = map
                        .get(key)
                        .map(stringify_formula_dump)
                        .unwrap_or_else(|| "null".to_owned());
                    format!(
                        "{}:{rendered}",
                        serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_owned())
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
    }
}

/// Sheet name used when a dump resolves an unqualified reference. Both sides of the
/// differential pass the same constant.
pub const DUMP_SHEET: &str = "Sheet1";

/// Stable JSON for one formula: token-join round trip, parse error, AST, ranges, spans.
pub fn dump_formula(formula: &str, formula_sheet: &str) -> Value {
    let mut join = Value::Null;
    let mut parse_error = Value::Null;
    let mut ast = Value::Null;
    let mut refs = Value::Array(Vec::new());
    let mut spans = Value::Array(Vec::new());
    match tokenize(formula) {
        Err(err) => parse_error = err.to_json(),
        Ok(tokens) => {
            join = Value::String(join_tokens(&tokens));
            refs = Value::Array(
                referenced_ranges(formula, formula_sheet)
                    .iter()
                    .map(ReferencedRange::to_json)
                    .collect(),
            );
            spans = Value::Array(
                reference_spans(formula, formula_sheet)
                    .iter()
                    .map(ReferenceSpan::to_json)
                    .collect(),
            );
            match parse_tokens(&tokens) {
                Ok(tree) => ast = tree.to_json(),
                Err(err) => parse_error = err.to_json(),
            }
        }
    }
    let mut out = Map::new();
    out.insert("formula".into(), Value::String(formula.to_owned()));
    out.insert("join".into(), join);
    out.insert("parseError".into(), parse_error);
    out.insert("ast".into(), ast);
    out.insert("refs".into(), refs);
    out.insert("spans".into(), spans);
    Value::Object(out)
}

/// Dump every formula in order, unqualified refs resolved against [`DUMP_SHEET`] when `sheet` is empty.
pub fn dump_formulas(formulas: &[String], formula_sheet: &str) -> Value {
    let sheet = if formula_sheet.is_empty() {
        DUMP_SHEET
    } else {
        formula_sheet
    };
    Value::Array(
        formulas
            .iter()
            .map(|formula| dump_formula(formula, sheet))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_literal_keeps_fifteen_significant_digits() {
        // Catches: Excel keeps only 15 significant digits of a typed number literal.
        assert_eq!(
            number_literal("123456789012345678").to_bits(),
            0x437b69b4ba630f0a
        );
        assert_eq!(
            number_literal("0.1234567890123456789").to_bits(),
            0x3fbf9add3746f62e
        );
        assert_eq!(
            number_literal("1.234567890123456789012345e+20").to_bits(),
            0x441ac53a7e04bcb0
        );
    }

    #[test]
    fn space_intersects_and_parenthesized_comma_unions() {
        // Catches: a space between references is intersection, while a comma is union only inside parentheses.
        let inter = parse_formula("A1:B2 B1:C3").expect("intersect");
        assert!(matches!(
            inter,
            FormulaAst::Binary {
                op: BinaryOp::Intersect,
                ..
            }
        ));
        let union = parse_formula("SUM((A1,B2))").expect("union");
        match union {
            FormulaAst::Call { args, .. } => {
                assert!(matches!(
                    args[0],
                    FormulaAst::Binary {
                        op: BinaryOp::Union,
                        ..
                    }
                ));
            }
            _ => panic!("expected call"),
        }
        let separated = parse_formula("SUM(A1,B2)").expect("args");
        match separated {
            FormulaAst::Call { args, .. } => {
                assert!(matches!(args[0], FormulaAst::Ref { .. }));
                assert!(matches!(args[1], FormulaAst::Ref { .. }));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn nesting_limit_spill_and_three_d() {
        // Catches: the 65th nested call is #VALUE!, A1# is a spill ref, and Sheet1:Sheet3! keeps both sheets.
        let deep = format!("{}1{}", "SUM(".repeat(65), ")".repeat(65));
        let err = parse_formula(&deep).expect_err("65 deep");
        assert_eq!(err.message, "More than 64 nested functions");
        assert_eq!(err.code.as_deref(), Some("#VALUE!"));
        assert_eq!(err.position, 256);
        let shallow = format!("{}1{}", "SUM(".repeat(64), ")".repeat(64));
        assert!(parse_formula(&shallow).is_ok());
        match parse_formula("A1#").expect("spill") {
            FormulaAst::Ref {
                spill: true,
                reference: Some(spec),
                ..
            } => assert_eq!(spec.kind, RefKind::Cell),
            other => panic!("spill: {other:?}"),
        }
        match parse_formula("Sheet1:Sheet3!A1").expect("3d") {
            FormulaAst::Ref {
                prefix: Some(prefix),
                ..
            } => {
                assert_eq!(prefix.sheet, "Sheet1");
                assert_eq!(prefix.sheet2.as_deref(), Some("Sheet3"));
            }
            other => panic!("3d: {other:?}"),
        }
    }
}
