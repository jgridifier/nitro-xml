// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/ast.ts; changes listed in PROVENANCE.md.
//! Formula AST. JSON object key order matches `JSON.stringify` of the TypeScript nodes.

use serde_json::{json, Map, Value};

/// Malformed formula. `position` is a UTF-16 offset (`-1` when the failure has no spot).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaError {
    pub message: String,
    pub position: i64,
    pub code: Option<String>,
}

impl FormulaError {
    pub fn new(message: impl Into<String>, position: i64) -> Self {
        Self {
            message: message.into(),
            position,
            code: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("message".into(), Value::String(self.message.clone()));
        m.insert("position".into(), json!(self.position));
        if let Some(code) = &self.code {
            m.insert("code".into(), Value::String(code.clone()));
        }
        Value::Object(m)
    }
}

impl std::fmt::Display for FormulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FormulaError {}

/// One end of a reference. Whole-column refs have no meaningful row; whole-row refs no column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefCorner {
    pub row: i32,
    pub col: i32,
    pub row_abs: bool,
    pub col_abs: bool,
}

impl RefCorner {
    pub fn to_json(self) -> Value {
        json!({
            "row": self.row,
            "col": self.col,
            "rowAbs": self.row_abs,
            "colAbs": self.col_abs,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Cell,
    Area,
    Cols,
    Rows,
}

impl RefKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cell => "cell",
            Self::Area => "area",
            Self::Cols => "cols",
            Self::Rows => "rows",
        }
    }
}

/// A parsed A1-style reference without its sheet prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefSpec {
    pub kind: RefKind,
    pub start: RefCorner,
    pub end: RefCorner,
}

impl RefSpec {
    pub fn to_json(self) -> Value {
        json!({
            "kind": self.kind.as_str(),
            "start": self.start.to_json(),
            "end": self.end.to_json(),
        })
    }
}

/// The sheet part of a reference: `Sheet1!`, `'My Sheet'!`, `Sheet1:Sheet3!`, `[1]Sheet1!`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetPrefix {
    /// Raw prefix text including the trailing `!`.
    pub text: String,
    pub sheet: String,
    pub sheet2: Option<String>,
    pub book: Option<String>,
}

impl SheetPrefix {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("text".into(), Value::String(self.text.clone()));
        m.insert("sheet".into(), Value::String(self.sheet.clone()));
        if let Some(sheet2) = &self.sheet2 {
            m.insert("sheet2".into(), Value::String(sheet2.clone()));
        }
        if let Some(book) = &self.book {
            m.insert("book".into(), Value::String(book.clone()));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredSpecial {
    All,
    Data,
    Headers,
    Totals,
    ThisRow,
}

impl StructuredSpecial {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "#All",
            Self::Data => "#Data",
            Self::Headers => "#Headers",
            Self::Totals => "#Totals",
            Self::ThisRow => "#This Row",
        }
    }
}

/// A structured (table) reference such as `Table1[[#This Row],[Amount]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredRef {
    pub table: Option<String>,
    pub specials: Vec<StructuredSpecial>,
    pub column: Option<String>,
    pub column2: Option<String>,
}

impl StructuredRef {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert(
            "specials".into(),
            Value::Array(
                self.specials
                    .iter()
                    .map(|s| Value::String(s.as_str().to_owned()))
                    .collect(),
            ),
        );
        if let Some(table) = &self.table {
            m.insert("table".into(), Value::String(table.clone()));
        }
        if let Some(column) = &self.column {
            m.insert("column".into(), Value::String(column.clone()));
        }
        if let Some(column2) = &self.column2 {
            m.insert("column2".into(), Value::String(column2.clone()));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Range,
    Intersect,
    Union,
}

impl BinaryOp {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Pow => "^",
            Self::Concat => "&",
            Self::Eq => "=",
            Self::Ne => "<>",
            Self::Lt => "<",
            Self::Gt => ">",
            Self::Le => "<=",
            Self::Ge => ">=",
            Self::Range => ":",
            Self::Intersect => " ",
            Self::Union => ",",
        }
    }

    pub fn from_text(text: &str) -> Option<(u8, Self)> {
        Some(match text {
            "=" => (1, Self::Eq),
            "<>" => (1, Self::Ne),
            "<" => (1, Self::Lt),
            ">" => (1, Self::Gt),
            "<=" => (1, Self::Le),
            ">=" => (1, Self::Ge),
            "&" => (2, Self::Concat),
            "+" => (3, Self::Add),
            "-" => (3, Self::Sub),
            "*" => (4, Self::Mul),
            "/" => (4, Self::Div),
            "^" => (5, Self::Pow),
            ":" => (10, Self::Range),
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    At,
}

impl UnaryOp {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plus => "+",
            Self::Minus => "-",
            Self::At => "@",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayConstant {
    Number(f64),
    String(String),
    Bool(bool),
    Error(String),
}

impl ArrayConstant {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Number(n) => json_f64(*n),
            Self::String(s) => Value::String(s.clone()),
            Self::Bool(b) => Value::Bool(*b),
            Self::Error(code) => json!({ "error": code }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormulaAst {
    Number {
        value: f64,
    },
    String {
        value: String,
    },
    Boolean {
        value: bool,
    },
    Error {
        code: String,
    },
    Ref {
        prefix: Option<SheetPrefix>,
        reference: Option<RefSpec>,
        spill: bool,
    },
    Name {
        prefix: Option<SheetPrefix>,
        name: String,
    },
    Structured {
        reference: StructuredRef,
        text: String,
    },
    Array {
        rows: Vec<Vec<ArrayConstant>>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<FormulaAst>,
    },
    Percent {
        operand: Box<FormulaAst>,
    },
    Binary {
        op: BinaryOp,
        left: Box<FormulaAst>,
        right: Box<FormulaAst>,
    },
    Call {
        name: String,
        raw_name: String,
        args: Vec<FormulaAst>,
    },
    Invoke {
        callee: Box<FormulaAst>,
        args: Vec<FormulaAst>,
    },
    Missing,
}

impl FormulaAst {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Number { value } => json!({ "type": "number", "value": json_f64(*value) }),
            Self::String { value } => json!({ "type": "string", "value": value }),
            Self::Boolean { value } => json!({ "type": "boolean", "value": value }),
            Self::Error { code } => json!({ "type": "error", "code": code }),
            Self::Ref {
                prefix,
                reference,
                spill,
            } => {
                let mut m = Map::new();
                m.insert("type".into(), json!("ref"));
                if let Some(prefix) = prefix {
                    m.insert("prefix".into(), prefix.to_json());
                }
                if let Some(reference) = reference {
                    m.insert("ref".into(), reference.to_json());
                }
                if *spill {
                    m.insert("spill".into(), Value::Bool(true));
                }
                Value::Object(m)
            }
            Self::Name { prefix, name } => {
                let mut m = Map::new();
                m.insert("type".into(), json!("name"));
                m.insert("name".into(), Value::String(name.clone()));
                if let Some(prefix) = prefix {
                    m.insert("prefix".into(), prefix.to_json());
                }
                Value::Object(m)
            }
            Self::Structured { reference, text } => json!({
                "type": "structured",
                "ref": reference.to_json(),
                "text": text,
            }),
            Self::Array { rows } => {
                let rows = rows
                    .iter()
                    .map(|row| Value::Array(row.iter().map(ArrayConstant::to_json).collect()))
                    .collect();
                json!({ "type": "array", "rows": Value::Array(rows) })
            }
            Self::Unary { op, operand } => json!({
                "type": "unary",
                "op": op.as_str(),
                "operand": operand.to_json(),
            }),
            Self::Percent { operand } => json!({
                "type": "percent",
                "operand": operand.to_json(),
            }),
            Self::Binary { op, left, right } => json!({
                "type": "binary",
                "op": op.as_str(),
                "left": left.to_json(),
                "right": right.to_json(),
            }),
            Self::Call {
                name,
                raw_name,
                args,
            } => json!({
                "type": "call",
                "name": name,
                "rawName": raw_name,
                "args": Value::Array(args.iter().map(FormulaAst::to_json).collect()),
            }),
            Self::Invoke { callee, args } => json!({
                "type": "invoke",
                "callee": callee.to_json(),
                "args": Value::Array(args.iter().map(FormulaAst::to_json).collect()),
            }),
            Self::Missing => json!({ "type": "missing" }),
        }
    }
}

/// `JSON.stringify` of a finite number, or `null` for NaN / Infinity (ECMAScript).
pub(crate) fn json_f64(n: f64) -> Value {
    serde_json::Number::from_f64(n)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// Strips `_xlfn.` / `_xlws.` / `_xludf.` storage prefixes and uppercases the rest.
pub fn normalize_function_name(name: &str) -> String {
    let mut rest = name;
    loop {
        let next = strip_ascii_prefix(rest, &["_xlfn.", "_xlws.", "_xludf."]);
        if next.len() == rest.len() {
            break;
        }
        rest = next;
    }
    rest.to_uppercase()
}

/// Strips one `_xlpm.` prefix from a LET / LAMBDA parameter name.
pub fn normalize_local_name(name: &str) -> String {
    strip_ascii_prefix(name, &["_xlpm."]).to_uppercase()
}

fn strip_ascii_prefix<'a>(name: &'a str, prefixes: &[&str]) -> &'a str {
    for prefix in prefixes {
        if name.len() >= prefix.len() && name[..prefix.len()].eq_ignore_ascii_case(prefix) {
            return &name[prefix.len()..];
        }
    }
    name
}
