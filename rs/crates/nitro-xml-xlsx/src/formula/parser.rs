// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/parser.ts; changes listed in PROVENANCE.md.
//! Pratt parser for Excel formulas. Whitespace between references is intersection;
//! a comma is union only inside parentheses.

use crate::formula::ast::{
    normalize_function_name, ArrayConstant, BinaryOp, FormulaAst, FormulaError, UnaryOp,
};
use crate::formula::lexer::{tokenize, Token, TokenKind, TokenValue};
use crate::formula::refs::parse_structured;

/// Excel allows 64 nested function calls; deeper formulas show `#VALUE!`.
pub const MAX_FUNCTION_NESTING: u32 = 64;
/// Nesting of parentheses and prefix operators, bounded so parsing cannot exhaust the stack.
const MAX_EXPRESSION_NESTING: u32 = 512;
const PREC_UNARY: u8 = 6;
const PREC_PERCENT: u8 = 7;
const PREC_UNION: u8 = 8;
const PREC_INTERSECT: u8 = 9;
const PREC_RANGE: u8 = 10;

struct Cursor<'a> {
    tokens: &'a [Token],
    ws_before: Vec<bool>,
    pos: usize,
    depth: u32,
    calls: u32,
}

/// Parses a formula (with or without a leading `=`) into a syntax tree.
pub fn parse_formula(formula: &str) -> Result<FormulaAst, FormulaError> {
    parse_tokens(&tokenize(formula)?)
}

/// Parses tokens as [`tokenize`] returns them.
pub fn parse_tokens(all: &[Token]) -> Result<FormulaAst, FormulaError> {
    let mut tokens = Vec::new();
    let mut ws_before = Vec::new();
    let mut saw_ws = false;
    for token in all {
        if token.kind == TokenKind::Ws {
            saw_ws = true;
            continue;
        }
        tokens.push(token.clone());
        ws_before.push(saw_ws);
        saw_ws = false;
    }
    if tokens.is_empty() {
        return Err(FormulaError::new("Empty formula", 0));
    }
    let mut cursor = Cursor {
        tokens: &tokens,
        ws_before,
        pos: 0,
        depth: 0,
        calls: 0,
    };
    let ast = parse_expression(&mut cursor, 0, false)?;
    if let Some(extra) = cursor.tokens.get(cursor.pos) {
        return Err(FormulaError::new(
            format!("Unexpected '{}'", extra.text),
            extra.start as i64,
        ));
    }
    Ok(ast)
}

fn parse_expression(
    c: &mut Cursor<'_>,
    min_prec: u8,
    in_parens: bool,
) -> Result<FormulaAst, FormulaError> {
    c.depth += 1;
    if c.depth > MAX_EXPRESSION_NESTING {
        let pos = c.tokens.get(c.pos).map(|t| t.start as i64).unwrap_or(-1);
        c.depth -= 1;
        return Err(FormulaError::new("Formula is nested too deeply", pos).with_code("#VALUE!"));
    }
    let result = parse_operators(c, min_prec, in_parens);
    c.depth -= 1;
    result
}

fn parse_operators(
    c: &mut Cursor<'_>,
    min_prec: u8,
    in_parens: bool,
) -> Result<FormulaAst, FormulaError> {
    let mut left = parse_prefix(c, in_parens)?;
    loop {
        if c.pos >= c.tokens.len() {
            return Ok(left);
        }
        let kind = c.tokens[c.pos].kind;
        let text = c.tokens[c.pos].text.clone();
        let ws = c.ws_before[c.pos];
        if kind == TokenKind::Op && text == "%" {
            if PREC_PERCENT < min_prec {
                return Ok(left);
            }
            c.pos += 1;
            left = FormulaAst::Percent {
                operand: Box::new(left),
            };
            continue;
        }
        if ws && operand_start(kind) && PREC_INTERSECT >= min_prec && is_reference_like(&left) {
            let right = parse_expression(c, PREC_INTERSECT + 1, in_parens)?;
            left = binary(BinaryOp::Intersect, left, right);
            continue;
        }
        if kind == TokenKind::Comma && in_parens && PREC_UNION >= min_prec {
            c.pos += 1;
            let right = parse_expression(c, PREC_UNION + 1, in_parens)?;
            left = binary(BinaryOp::Union, left, right);
            continue;
        }
        if kind == TokenKind::Open && is_invocable(&left) {
            c.pos += 1;
            let args = parse_arguments(c)?;
            left = FormulaAst::Invoke {
                callee: Box::new(left),
                args,
            };
            continue;
        }
        let Some((prec, op)) = (kind == TokenKind::Op)
            .then(|| BinaryOp::from_text(&text))
            .flatten()
        else {
            return Ok(left);
        };
        if prec < min_prec {
            return Ok(left);
        }
        c.pos += 1;
        let right = parse_expression(c, prec + 1, in_parens)?;
        left = binary(op, left, right);
    }
}

fn binary(op: BinaryOp, left: FormulaAst, right: FormulaAst) -> FormulaAst {
    FormulaAst::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn is_invocable(node: &FormulaAst) -> bool {
    matches!(node, FormulaAst::Call { .. } | FormulaAst::Invoke { .. })
}

fn is_reference_like(node: &FormulaAst) -> bool {
    match node {
        FormulaAst::Ref { .. }
        | FormulaAst::Name { .. }
        | FormulaAst::Structured { .. }
        | FormulaAst::Call { .. } => true,
        FormulaAst::Binary { op, .. } => {
            matches!(op, BinaryOp::Range | BinaryOp::Intersect | BinaryOp::Union)
        }
        _ => false,
    }
}

fn operand_start(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Ref
            | TokenKind::Name
            | TokenKind::Func
            | TokenKind::Open
            | TokenKind::Structured
    )
}

fn parse_prefix(c: &mut Cursor<'_>, in_parens: bool) -> Result<FormulaAst, FormulaError> {
    let Some(peek) = c.tokens.get(c.pos) else {
        return Err(FormulaError::new("Unexpected end of formula", -1));
    };
    let kind = peek.kind;
    let text = peek.text.clone();
    if kind == TokenKind::Op && (text == "-" || text == "+") {
        let op = if text == "-" {
            UnaryOp::Minus
        } else {
            UnaryOp::Plus
        };
        c.pos += 1;
        let operand = parse_expression(c, PREC_UNARY, in_parens)?;
        return Ok(FormulaAst::Unary {
            op,
            operand: Box::new(operand),
        });
    }
    if kind == TokenKind::Op && text == "@" {
        c.pos += 1;
        let operand = parse_expression(c, PREC_RANGE, in_parens)?;
        return Ok(FormulaAst::Unary {
            op: UnaryOp::At,
            operand: Box::new(operand),
        });
    }
    let token = c.tokens[c.pos].clone();
    c.pos += 1;
    match token.kind {
        TokenKind::Number => Ok(FormulaAst::Number {
            value: number_value(&token)?,
        }),
        TokenKind::String => Ok(FormulaAst::String {
            value: text_value(&token)?,
        }),
        TokenKind::Bool => Ok(FormulaAst::Boolean {
            value: bool_value(&token)?,
        }),
        TokenKind::Error => Ok(FormulaAst::Error {
            code: text_value(&token)?,
        }),
        TokenKind::Ref => Ok(FormulaAst::Ref {
            prefix: token.prefix,
            reference: token.reference,
            spill: token.spill,
        }),
        TokenKind::Name => Ok(FormulaAst::Name {
            name: text_value(&token)?,
            prefix: token.prefix,
        }),
        TokenKind::Structured => {
            let table = text_value(&token)?;
            let reference = parse_structured(&token.text, &table)?;
            Ok(FormulaAst::Structured {
                reference,
                text: token.text,
            })
        }
        TokenKind::Func => parse_call(c, &token),
        TokenKind::Open => {
            let inner = parse_expression(c, 0, true)?;
            expect(c, TokenKind::Close, "')'")?;
            Ok(inner)
        }
        TokenKind::Lbrace => parse_array(c),
        _ => Err(FormulaError::new(
            format!("Unexpected '{}'", token.text),
            token.start as i64,
        )),
    }
}

fn parse_call(c: &mut Cursor<'_>, token: &Token) -> Result<FormulaAst, FormulaError> {
    expect(c, TokenKind::Open, "'('")?;
    let raw_name = text_value(token)?;
    c.calls += 1;
    if c.calls > MAX_FUNCTION_NESTING {
        return Err(
            FormulaError::new("More than 64 nested functions", token.start as i64)
                .with_code("#VALUE!"),
        );
    }
    let args = parse_arguments(c)?;
    c.calls -= 1;
    Ok(FormulaAst::Call {
        name: normalize_function_name(&raw_name),
        raw_name,
        args,
    })
}

fn parse_arguments(c: &mut Cursor<'_>) -> Result<Vec<FormulaAst>, FormulaError> {
    if c.tokens.get(c.pos).map(|t| t.kind) == Some(TokenKind::Close) {
        c.pos += 1;
        return Ok(Vec::new());
    }
    let mut args = Vec::new();
    loop {
        let kind = c.tokens.get(c.pos).map(|t| t.kind);
        if kind == Some(TokenKind::Comma) || kind == Some(TokenKind::Close) {
            args.push(FormulaAst::Missing);
        } else {
            args.push(parse_expression(c, 0, false)?);
        }
        if c.tokens.get(c.pos).map(|t| t.kind) == Some(TokenKind::Comma) {
            c.pos += 1;
            continue;
        }
        expect(c, TokenKind::Close, "',' or ')'")?;
        return Ok(args);
    }
}

fn parse_array_item(c: &mut Cursor<'_>) -> Result<ArrayConstant, FormulaError> {
    let mut sign = 1i32;
    while let Some(token) = c.tokens.get(c.pos) {
        if token.kind != TokenKind::Op || (token.text != "-" && token.text != "+") {
            break;
        }
        if token.text == "-" {
            sign = -sign;
        }
        c.pos += 1;
    }
    let Some(token) = c.tokens.get(c.pos).cloned() else {
        return Err(FormulaError::new("Unterminated array constant", -1));
    };
    c.pos += 1;
    if token.kind == TokenKind::Number {
        let n = number_value(&token)?;
        return Ok(ArrayConstant::Number(if sign < 0 { -n } else { n }));
    }
    if sign != 1 {
        return Err(FormulaError::new(
            "Invalid array constant",
            token.start as i64,
        ));
    }
    match token.kind {
        TokenKind::String => Ok(ArrayConstant::String(text_value(&token)?)),
        TokenKind::Bool => Ok(ArrayConstant::Bool(bool_value(&token)?)),
        TokenKind::Error => Ok(ArrayConstant::Error(text_value(&token)?)),
        TokenKind::Ref if token.reference.is_none() && token.prefix.is_none() => {
            Ok(ArrayConstant::Error("#REF!".to_owned()))
        }
        _ => Err(FormulaError::new(
            "Array constants may only hold numbers, text, logicals and errors",
            token.start as i64,
        )),
    }
}

fn parse_array(c: &mut Cursor<'_>) -> Result<FormulaAst, FormulaError> {
    let mut rows: Vec<Vec<ArrayConstant>> = vec![Vec::new()];
    loop {
        let item = parse_array_item(c)?;
        rows.last_mut().expect("row").push(item);
        let kind = c.tokens.get(c.pos).map(|t| t.kind);
        let start = c.tokens.get(c.pos).map(|t| t.start as i64).unwrap_or(-1);
        c.pos += 1;
        if kind == Some(TokenKind::Comma) {
            continue;
        }
        if kind == Some(TokenKind::Semicolon) {
            rows.push(Vec::new());
            continue;
        }
        if kind == Some(TokenKind::Rbrace) {
            break;
        }
        return Err(FormulaError::new("Expected ',', ';' or '}'", start));
    }
    let width = rows.first().map(Vec::len).unwrap_or(0);
    if rows.iter().any(|row| row.len() != width) {
        return Err(FormulaError::new(
            "Array constant rows must have the same length",
            -1,
        ));
    }
    Ok(FormulaAst::Array { rows })
}

fn expect(c: &mut Cursor<'_>, kind: TokenKind, what: &str) -> Result<(), FormulaError> {
    let token = c.tokens.get(c.pos);
    if token.map(|t| t.kind) != Some(kind) {
        return Err(FormulaError::new(
            format!("Expected {what}"),
            token.map(|t| t.start as i64).unwrap_or(-1),
        ));
    }
    c.pos += 1;
    Ok(())
}

fn number_value(token: &Token) -> Result<f64, FormulaError> {
    match token.value {
        Some(TokenValue::Number(n)) => Ok(n),
        _ => Err(FormulaError::new(
            format!("Unexpected '{}'", token.text),
            token.start as i64,
        )),
    }
}

fn text_value(token: &Token) -> Result<String, FormulaError> {
    match &token.value {
        Some(TokenValue::Text(text)) => Ok(text.clone()),
        _ => Err(FormulaError::new(
            format!("Unexpected '{}'", token.text),
            token.start as i64,
        )),
    }
}

fn bool_value(token: &Token) -> Result<bool, FormulaError> {
    match token.value {
        Some(TokenValue::Bool(value)) => Ok(value),
        _ => Err(FormulaError::new(
            format!("Unexpected '{}'", token.text),
            token.start as i64,
        )),
    }
}
