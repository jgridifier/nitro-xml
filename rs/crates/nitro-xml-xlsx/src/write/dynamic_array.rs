// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/dynamic-array.ts
// (DYNAMIC_ARRAY_METADATA_XML) and src/core/xlsx/formula/array-context.ts
// (needsArrayEvaluation); changes listed in PROVENANCE.md.
//! Dynamic-array formulas on save: which formulas are XLDAPR anchors, and the
//! `xl/metadata.xml` part written when the source workbook has none.

use crate::formula::{parse_formula, BinaryOp, FormulaAst, RefKind, StructuredSpecial, UnaryOp};

const NATIVE_ARRAY: &[&str] = &[
    "SUMPRODUCT",
    "MMULT",
    "TRANSPOSE",
    "LOOKUP",
    "INDEX",
    "MATCH",
];

struct Scan {
    multi: bool,
    needs: bool,
}

fn scan(node: &FormulaAst, native: bool) -> Scan {
    match node {
        FormulaAst::Ref {
            spill, reference, ..
        } => Scan {
            multi: *spill || reference.is_some_and(|r| r.kind != RefKind::Cell),
            needs: false,
        },
        FormulaAst::Array { rows } => Scan {
            multi: rows.len() > 1 || rows.first().is_some_and(|row| row.len() > 1),
            needs: false,
        },
        FormulaAst::Structured { reference, .. } => Scan {
            multi: !reference.specials.contains(&StructuredSpecial::ThisRow),
            needs: false,
        },
        FormulaAst::Unary { op, operand } => {
            let inner = scan(operand, native);
            if *op == UnaryOp::At {
                Scan {
                    multi: false,
                    needs: inner.needs,
                }
            } else {
                Scan {
                    multi: inner.multi,
                    needs: inner.needs || (inner.multi && !native),
                }
            }
        }
        FormulaAst::Percent { operand } => {
            let inner = scan(operand, native);
            Scan {
                multi: inner.multi,
                needs: inner.needs || (inner.multi && !native),
            }
        }
        FormulaAst::Binary { op, left, right } => {
            let l = scan(left, native);
            let r = scan(right, native);
            if matches!(op, BinaryOp::Range | BinaryOp::Intersect | BinaryOp::Union) {
                Scan {
                    multi: true,
                    needs: l.needs || r.needs,
                }
            } else {
                let multi = l.multi || r.multi;
                Scan {
                    multi,
                    needs: l.needs || r.needs || (multi && !native),
                }
            }
        }
        FormulaAst::Call { name, args, .. } => {
            // Parameter kinds need the function registry (#72). Unknown calls only
            // propagate `needs` from their arguments, which matches a missing spec.
            let inner = native || NATIVE_ARRAY.contains(&name.as_str());
            let mut needs = false;
            for arg in args {
                needs |= scan(arg, inner).needs;
            }
            Scan {
                multi: false,
                needs,
            }
        }
        FormulaAst::Invoke { .. } => Scan {
            multi: true,
            needs: true,
        },
        _ => Scan {
            multi: false,
            needs: false,
        },
    }
}

/// Whether `formula` must be stored as a dynamic array so its meaning survives.
/// Unparsable formulas report false, matching upstream.
pub fn needs_array_evaluation(formula: &str) -> bool {
    let Ok(ast) = parse_formula(formula) else {
        return false;
    };
    let result = scan(&ast, false);
    result.needs || result.multi
}

/// `xl/metadata.xml` declaring cell metadata block 1 as an Excel dynamic array (XLDAPR).
pub const DYNAMIC_ARRAY_METADATA_XML: &str =
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<metadata xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
xmlns:xda=\"http://schemas.microsoft.com/office/spreadsheetml/2017/dynamicarray\">\
<metadataTypes count=\"1\"><metadataType name=\"XLDAPR\" minSupportedVersion=\"120000\" copy=\"1\" \
pasteAll=\"1\" pasteValues=\"1\" merge=\"1\" splitFirst=\"1\" rowColShift=\"1\" clearFormats=\"1\" \
clearComments=\"1\" assign=\"1\" coerce=\"1\" cellMeta=\"1\"/></metadataTypes>\
<futureMetadata name=\"XLDAPR\" count=\"1\"><bk><extLst>\
<ext uri=\"{bdbb8cdc-fa1e-496e-a857-3c3f30c029c3}\">\
<xda:dynamicArrayProperties fDynamic=\"1\" fCollapsed=\"0\"/></ext></extLst></bk></futureMetadata>\
<cellMetadata count=\"1\"><bk><rc t=\"1\" v=\"0\"/></bk></cellMetadata></metadata>";
