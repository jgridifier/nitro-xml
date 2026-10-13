// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/{evaluator,call,engine-util,references,spill}.ts
// (the slice math and statistical functions need); changes listed in PROVENANCE.md.
//! Minimal evaluator for the formula families this crate implements.
//! Dependency spilling and defined names stay `TODO(#72)`.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

use serde_json::{json, Value as Json};

use crate::address::{parse_address, MAX_COL, MAX_ROW};
use crate::formula::{
    parse_formula, ArrayConstant, BinaryOp, FormulaAst, FormulaError, SheetPrefix, UnaryOp,
};

use super::coerce::finite;
use super::helpers::num;
use super::host::{area_to_matrix, implicit_intersection, CallCtx, EvalHost};
use super::ops::{broadcast2, scalar_binary, scalar_unary, ScalarOrMatrix};
use super::spec::{param_kind, FuncKind, FunctionSpec, ParamKind};
use super::value::{clean, Area, ErrorCode, Matrix, RefValue, Scalar, Value};

const MAX_FORMULA_DEPTH: usize = 64;
const RAND_MOD: u64 = 2_147_483_647;

type Key = (usize, i32, i32);

#[derive(Clone)]
enum Slot {
    Const(Scalar),
    Formula(String),
}

#[derive(Clone)]
struct Settled {
    rows: Vec<Vec<Scalar>>,
}

impl Settled {
    fn scalar(value: Scalar) -> Self {
        Self {
            rows: vec![vec![value]],
        }
    }

    fn top(&self) -> Scalar {
        self.rows
            .first()
            .and_then(|line| line.first())
            .cloned()
            .unwrap_or(Scalar::Number(0.0))
    }

    fn rows_json(&self) -> Json {
        Json::Array(
            self.rows
                .iter()
                .map(|line| Json::Array(line.iter().map(Scalar::json).collect()))
                .collect(),
        )
    }
}

enum Final {
    Scalar(Scalar),
    Matrix(Matrix),
}

/// Sheet grid the harness evaluates against. Formula cells are computed on demand;
/// a circular group is 0 (`TODO(#72)` does not iterate). Array spills of dependency
/// formulas are not written (`TODO(#72)`); the anchor result is the full matrix.
pub struct GridHost {
    names: Vec<String>,
    sheets: Vec<BTreeMap<(i32, i32), Slot>>,
    hidden: HashSet<(usize, i32)>,
    date1904: bool,
    seed: Cell<u64>,
    memo: RefCell<HashMap<Key, Scalar>>,
    stack: RefCell<Vec<Key>>,
    cycle_hits: RefCell<HashSet<Key>>,
    anchor: Cell<Key>,
    anchor_out: RefCell<Option<Settled>>,
}

impl GridHost {
    pub fn new(sheet_names: &[&str]) -> Self {
        let names: Vec<String> = if sheet_names.is_empty() {
            vec!["Sheet1".to_owned()]
        } else {
            sheet_names.iter().map(|name| (*name).to_owned()).collect()
        };
        let sheets = (0..names.len()).map(|_| BTreeMap::new()).collect();
        Self {
            names,
            sheets,
            hidden: HashSet::new(),
            date1904: false,
            seed: Cell::new(42),
            memo: RefCell::new(HashMap::new()),
            stack: RefCell::new(Vec::new()),
            cycle_hits: RefCell::new(HashSet::new()),
            anchor: Cell::new((0, 0, 0)),
            anchor_out: RefCell::new(None),
        }
    }

    pub fn set_const(&mut self, sheet: usize, row: i32, col: i32, value: Scalar) {
        self.insert(sheet, row, col, Slot::Const(value));
    }

    pub fn set_formula(&mut self, sheet: usize, row: i32, col: i32, formula: &str) {
        self.insert(sheet, row, col, Slot::Formula(strip_eq(formula)));
    }

    pub fn set_hidden(&mut self, sheet: usize, row: i32) {
        self.hidden.insert((sheet, row));
    }

    fn insert(&mut self, sheet: usize, row: i32, col: i32, slot: Slot) {
        if let Some(cells) = self.sheets.get_mut(sheet) {
            cells.insert((row, col), slot);
        }
    }

    fn formula_keys(&self) -> Vec<Key> {
        let mut keys = Vec::new();
        for (sheet, cells) in self.sheets.iter().enumerate() {
            for (&(row, col), slot) in cells {
                if matches!(slot, Slot::Formula(_)) {
                    keys.push((sheet, row, col));
                }
            }
        }
        keys
    }

    fn formula_text(&self, key: Key) -> Option<String> {
        match self.sheets.get(key.0)?.get(&(key.1, key.2))? {
            Slot::Formula(text) => Some(text.clone()),
            Slot::Const(_) => None,
        }
    }

    fn run(&self) -> Settled {
        self.memo.borrow_mut().clear();
        self.stack.borrow_mut().clear();
        self.cycle_hits.borrow_mut().clear();
        *self.anchor_out.borrow_mut() = None;
        self.seed.set(42);
        let keys = self.formula_keys();
        let edges = self.dependency_edges(&keys);
        for component in tarjan(&keys, &edges) {
            let cyclic = component.len() > 1
                || edges
                    .get(&component[0])
                    .is_some_and(|next| next.contains(&component[0]));
            if cyclic {
                for key in component {
                    self.memo.borrow_mut().insert(key, Scalar::Number(0.0));
                    if key == self.anchor.get() {
                        *self.anchor_out.borrow_mut() = Some(Settled::scalar(Scalar::Number(0.0)));
                    }
                }
            } else {
                let _ = self.compute_formula(component[0]);
            }
        }
        self.anchor_out
            .borrow_mut()
            .take()
            .unwrap_or_else(|| Settled::scalar(Scalar::Error(ErrorCode::Name)))
    }

    fn dependency_edges(&self, keys: &[Key]) -> HashMap<Key, Vec<Key>> {
        let mut edges = HashMap::new();
        for &key in keys {
            let areas = self.dep_areas(key);
            let mut next = Vec::new();
            for &other in keys {
                if areas.iter().any(|area| covers(*area, other)) {
                    next.push(other);
                }
            }
            edges.insert(key, next);
        }
        edges
    }

    fn dep_areas(&self, key: Key) -> Vec<Area> {
        let Some(text) = self.formula_text(key) else {
            return Vec::new();
        };
        let Ok(ast) = parse_formula(&text) else {
            return Vec::new();
        };
        let mut areas = Vec::new();
        self.collect_areas(&ast, key.0, &mut areas);
        areas
    }

    fn collect_areas(&self, node: &FormulaAst, sheet: usize, out: &mut Vec<Area>) {
        match node {
            FormulaAst::Ref {
                prefix,
                reference,
                spill,
            } => {
                let Some(spec) = reference else {
                    return;
                };
                let Some(sheets) = self.prefix_sheets(prefix, sheet) else {
                    return;
                };
                for index in sheets {
                    if *spill {
                        out.push(Area {
                            sheet: index,
                            start_row: spec.start.row,
                            start_col: spec.start.col,
                            end_row: spec.start.row,
                            end_col: spec.start.col,
                        });
                    } else {
                        out.push(Area::normalize(
                            index,
                            spec.start.row,
                            spec.start.col,
                            spec.end.row,
                            spec.end.col,
                        ));
                    }
                }
            }
            FormulaAst::Unary { operand, .. } | FormulaAst::Percent { operand } => {
                self.collect_areas(operand, sheet, out);
            }
            FormulaAst::Binary { op, left, right } => {
                if *op == BinaryOp::Range
                    && matches!(left.as_ref(), FormulaAst::Ref { .. })
                    && matches!(right.as_ref(), FormulaAst::Ref { .. })
                {
                    let start = out.len();
                    self.collect_areas(left, sheet, out);
                    let mid = out.len();
                    self.collect_areas(right, sheet, out);
                    let left_areas = out[start..mid].to_vec();
                    let right_areas = out[mid..].to_vec();
                    for left_area in &left_areas {
                        for right_area in &right_areas {
                            if left_area.sheet == right_area.sheet {
                                out.push(Area {
                                    sheet: left_area.sheet,
                                    start_row: left_area.start_row.min(right_area.start_row),
                                    start_col: left_area.start_col.min(right_area.start_col),
                                    end_row: left_area.end_row.max(right_area.end_row),
                                    end_col: left_area.end_col.max(right_area.end_col),
                                });
                            }
                        }
                    }
                } else {
                    self.collect_areas(left, sheet, out);
                    self.collect_areas(right, sheet, out);
                }
            }
            FormulaAst::Call { args, .. } => {
                for arg in args {
                    self.collect_areas(arg, sheet, out);
                }
            }
            FormulaAst::Invoke { callee, args } => {
                self.collect_areas(callee, sheet, out);
                for arg in args {
                    self.collect_areas(arg, sheet, out);
                }
            }
            _ => {}
        }
    }

    fn prefix_sheets(&self, prefix: &Option<SheetPrefix>, current: usize) -> Option<Vec<usize>> {
        let Some(prefix) = prefix else {
            return Some(vec![current]);
        };
        if prefix.book.is_some() {
            return None;
        }
        let first = self.find_sheet(&prefix.sheet)?;
        let Some(second) = &prefix.sheet2 else {
            return Some(vec![first]);
        };
        let last = self.find_sheet(second)?;
        let (lo, hi) = if first <= last {
            (first, last)
        } else {
            (last, first)
        };
        Some((lo..=hi).collect())
    }

    fn compute_formula(&self, key: Key) -> Scalar {
        if let Some(value) = self.memo.borrow().get(&key).cloned() {
            return value;
        }
        if self.stack.borrow().contains(&key) {
            self.cycle_hits.borrow_mut().insert(key);
            return Scalar::Number(0.0);
        }
        if self.stack.borrow().len() >= MAX_FORMULA_DEPTH {
            return Scalar::Error(ErrorCode::Value);
        }
        let Some(text) = self.formula_text(key) else {
            return Scalar::Blank;
        };
        self.stack.borrow_mut().push(key);
        let mut settled = self.settle_formula(&text, key);
        let hit = self.cycle_hits.borrow_mut().remove(&key);
        self.stack.borrow_mut().pop();
        if hit {
            settled = Settled::scalar(Scalar::Number(0.0));
        }
        let top = settled.top();
        self.memo.borrow_mut().insert(key, top.clone());
        if key == self.anchor.get() {
            *self.anchor_out.borrow_mut() = Some(settled);
        }
        top
    }

    fn settle_formula(&self, formula: &str, key: Key) -> Settled {
        let ast = match parse_formula(formula) {
            Ok(ast) => ast,
            Err(err) => return Settled::scalar(Scalar::Error(parse_error_code(&err))),
        };
        let value = eval_node(self, &ast, &ast, key, empty_scope());
        spill_check(self, key, finalize(self, value))
    }

    fn cell_scalar(&self, sheet: usize, row: i32, col: i32) -> Scalar {
        let Some(cells) = self.sheets.get(sheet) else {
            return Scalar::Blank;
        };
        match cells.get(&(row, col)) {
            None => Scalar::Blank,
            Some(Slot::Const(value)) => value.clone(),
            Some(Slot::Formula(_)) => self.compute_formula((sheet, row, col)),
        }
    }
}

impl EvalHost for GridHost {
    fn read_cell(&self, sheet: usize, row: i32, col: i32) -> Scalar {
        self.cell_scalar(sheet, row, col)
    }

    fn for_each_stored(&self, sheet: usize, area: Area, visit: &mut dyn FnMut(Scalar, i32, i32)) {
        let Some(cells) = self.sheets.get(sheet) else {
            return;
        };
        let keys: Vec<(i32, i32)> = cells
            .keys()
            .copied()
            .filter(|(row, col)| {
                *row >= area.start_row
                    && *row <= area.end_row
                    && *col >= area.start_col
                    && *col <= area.end_col
            })
            .collect();
        for (row, col) in keys {
            let value = self.cell_scalar(sheet, row, col);
            if !matches!(value, Scalar::Blank) {
                visit(value, row, col);
            }
        }
    }

    fn bounds(&self, sheet: usize) -> (i32, i32) {
        let mut rows = 0i32;
        let mut cols = 0i32;
        if let Some(cells) = self.sheets.get(sheet) {
            for &(row, col) in cells.keys() {
                rows = rows.max(row.saturating_add(1));
                cols = cols.max(col.saturating_add(1));
            }
        }
        (rows, cols)
    }

    fn cell_formula(&self, sheet: usize, row: i32, col: i32) -> Option<String> {
        self.formula_text((sheet, row, col))
    }

    fn sheet_count(&self) -> usize {
        self.names.len()
    }

    fn sheet_name(&self, index: usize) -> Option<&str> {
        self.names.get(index).map(String::as_str)
    }

    fn find_sheet(&self, name: &str) -> Option<usize> {
        if let Some(index) = self.names.iter().position(|sheet| sheet == name) {
            return Some(index);
        }
        self.names
            .iter()
            .position(|sheet| sheet.eq_ignore_ascii_case(name))
    }

    fn random(&self) -> f64 {
        let next = self.seed.get().saturating_mul(16807) % RAND_MOD;
        self.seed.set(next);
        (next.saturating_sub(1)) as f64 / (RAND_MOD - 1) as f64
    }

    fn now_serial(&self) -> f64 {
        // test-helpers.ts pins the clock at 2026-10-03 12:00 local.
        super::pinned_now_serial(self.date1904)
    }

    fn row_hidden(&self, sheet: usize, row: i32) -> bool {
        self.hidden.contains(&(sheet, row))
    }

    fn occupied(&self, sheet: usize, row: i32, col: i32) -> bool {
        match self
            .sheets
            .get(sheet)
            .and_then(|cells| cells.get(&(row, col)))
        {
            Some(Slot::Formula(_)) => true,
            Some(Slot::Const(Scalar::Blank)) | None => false,
            Some(Slot::Const(_)) => true,
        }
    }
}

fn covers(area: Area, key: Key) -> bool {
    area.sheet == key.0
        && key.1 >= area.start_row
        && key.1 <= area.end_row
        && key.2 >= area.start_col
        && key.2 <= area.end_col
}

fn parse_error_code(err: &FormulaError) -> ErrorCode {
    err.code
        .as_deref()
        .and_then(ErrorCode::parse)
        .unwrap_or(ErrorCode::Name)
}

fn strip_eq(formula: &str) -> String {
    formula.strip_prefix('=').unwrap_or(formula).to_owned()
}

fn empty_scope() -> &'static HashMap<String, Value> {
    static EMPTY: OnceLock<HashMap<String, Value>> = OnceLock::new();
    EMPTY.get_or_init(HashMap::new)
}

fn noop_eval(_node: &FormulaAst) -> Value {
    Value::error(ErrorCode::Value)
}

fn noop_eval_in(_node: &FormulaAst, _scope: &HashMap<String, Value>) -> Value {
    Value::error(ErrorCode::Value)
}

fn noop_lambda(_lambda: &super::value::LambdaValue, _args: &[Value]) -> Value {
    Value::error(ErrorCode::Value)
}

fn ctx<'a>(host: &'a GridHost, at: Key) -> CallCtx<'a> {
    let depth = host.stack.borrow().len() as u32;
    CallCtx {
        host,
        sheet: at.0,
        row: at.1,
        col: at.2,
        legacy: false,
        date1904: host.date1904,
        depth,
        scope: empty_scope(),
        eval: Some(&noop_eval),
        eval_in: Some(&noop_eval_in),
        call_lambda: Some(&noop_lambda),
    }
}

fn error_code(value: &Value) -> Option<ErrorCode> {
    match value {
        Value::Scalar(Scalar::Error(code)) => Some(*code),
        _ => None,
    }
}

fn eval_node(
    host: &GridHost,
    node: &FormulaAst,
    root: &FormulaAst,
    at: Key,
    scope: &HashMap<String, Value>,
) -> Value {
    let date1904 = host.date1904;
    match node {
        FormulaAst::Number { value } => Value::number(*value),
        FormulaAst::String { value } => Value::Scalar(Scalar::Text(value.clone())),
        FormulaAst::Boolean { value } => Value::Scalar(Scalar::Bool(*value)),
        FormulaAst::Error { code } => {
            Value::error(ErrorCode::parse(code).unwrap_or(ErrorCode::Value))
        }
        FormulaAst::Missing => Value::blank(),
        FormulaAst::Array { rows } => Value::Matrix(Matrix::from_rows(
            rows.iter()
                .map(|line| line.iter().map(constant_scalar).collect())
                .collect(),
        )),
        FormulaAst::Ref { .. } => eval_ref(host, node, at),
        FormulaAst::Name { prefix, name } => {
            if prefix.is_none() {
                let key = crate::formula::ast::normalize_local_name(name);
                if let Some(value) = scope.get(&key) {
                    return value.clone();
                }
            }
            Value::error(ErrorCode::Name)
        }
        FormulaAst::Structured { reference, .. } => {
            if reference.table.is_some() {
                Value::error(ErrorCode::Name)
            } else {
                Value::error(ErrorCode::Ref)
            }
        }
        FormulaAst::Unary { op, operand } => match op {
            UnaryOp::Plus => eval_node(host, operand, root, at, scope),
            UnaryOp::Minus => unary_map(
                operand_value(host, &eval_node(host, operand, root, at, scope), at),
                |value| scalar_unary(false, value, date1904),
            ),
            UnaryOp::At => {
                let value = eval_node(host, operand, root, at, scope);
                Value::Scalar(implicit_intersection(&ctx(host, at), &value))
            }
        },
        FormulaAst::Percent { operand } => unary_map(
            operand_value(host, &eval_node(host, operand, root, at, scope), at),
            |value| scalar_unary(true, value, date1904),
        ),
        FormulaAst::Binary { .. } => eval_binary(host, node, root, at, scope),
        FormulaAst::Call {
            name,
            raw_name,
            args,
        } => eval_call(host, name, raw_name, args, root, at, scope),
        FormulaAst::Invoke { callee, args } => {
            let callee_value = eval_node(host, callee, root, at, scope);
            let values: Vec<Value> = args
                .iter()
                .map(|arg| eval_node(host, arg, root, at, scope))
                .collect();
            if let Some(code) = error_code(&callee_value) {
                return Value::error(code);
            }
            match callee_value {
                Value::Lambda(lambda) => invoke_lambda(host, &lambda, &values, root, at),
                _ => Value::error(ErrorCode::Value),
            }
        }
    }
}

fn constant_scalar(item: &ArrayConstant) -> Scalar {
    match item {
        ArrayConstant::Number(value) => Scalar::Number(*value),
        ArrayConstant::String(value) => Scalar::Text(value.clone()),
        ArrayConstant::Bool(value) => Scalar::Bool(*value),
        ArrayConstant::Error(code) => {
            Scalar::Error(ErrorCode::parse(code).unwrap_or(ErrorCode::Value))
        }
    }
}

fn eval_ref(host: &GridHost, node: &FormulaAst, at: Key) -> Value {
    let FormulaAst::Ref {
        prefix,
        reference,
        spill,
    } = node
    else {
        return Value::error(ErrorCode::Ref);
    };
    let Some(spec) = reference else {
        return Value::error(ErrorCode::Ref);
    };
    // TODO(#72): spill references need the anchor's footprint.
    if *spill {
        return Value::error(ErrorCode::Ref);
    }
    let Some(sheets) = host.prefix_sheets(prefix, at.0) else {
        return Value::error(ErrorCode::Ref);
    };
    Value::Ref(RefValue {
        areas: sheets
            .into_iter()
            .map(|sheet| {
                Area::normalize(
                    sheet,
                    spec.start.row,
                    spec.start.col,
                    spec.end.row,
                    spec.end.col,
                )
            })
            .collect(),
    })
}

fn eval_binary(
    host: &GridHost,
    node: &FormulaAst,
    root: &FormulaAst,
    at: Key,
    scope: &HashMap<String, Value>,
) -> Value {
    let mut spine = Vec::new();
    let mut leftmost = node;
    while let FormulaAst::Binary { left, .. } = leftmost {
        spine.push(leftmost);
        leftmost = left;
    }
    let mut value = eval_node(host, leftmost, root, at, scope);
    for item in spine.iter().rev() {
        value = apply_binary(host, item, value, root, at, scope);
    }
    value
}

fn apply_binary(
    host: &GridHost,
    node: &FormulaAst,
    left: Value,
    root: &FormulaAst,
    at: Key,
    scope: &HashMap<String, Value>,
) -> Value {
    let FormulaAst::Binary { op, right, .. } = node else {
        return Value::error(ErrorCode::Value);
    };
    if matches!(op, BinaryOp::Range | BinaryOp::Intersect | BinaryOp::Union) {
        if let Some(code) = error_code(&left) {
            return Value::error(code);
        }
        let right = eval_node(host, right, root, at, scope);
        if let Some(code) = error_code(&right) {
            return Value::error(code);
        }
        return match op {
            BinaryOp::Range => range_op(&left, &right),
            BinaryOp::Intersect => intersect_op(&left, &right),
            _ => union_op(&left, &right),
        };
    }
    let right = eval_node(host, right, root, at, scope);
    let snap = std::ptr::eq(node, root) && matches!(op, BinaryOp::Add | BinaryOp::Sub);
    match broadcast2(
        &operand_value(host, &left, at),
        &operand_value(host, &right, at),
        |x, y| scalar_binary(*op, x, y, snap, host.date1904),
    ) {
        Ok(ScalarOrMatrix::Scalar(value)) => Value::Scalar(value),
        Ok(ScalarOrMatrix::Matrix(matrix)) => Value::Matrix(matrix),
        Err(code) => Value::error(code),
    }
}

fn range_op(left: &Value, right: &Value) -> Value {
    let (Value::Ref(left), Value::Ref(right)) = (left, right) else {
        return Value::error(ErrorCode::Value);
    };
    if left.areas.len() != 1 || right.areas.len() != 1 {
        return Value::error(ErrorCode::Value);
    }
    let x = left.areas[0];
    let y = right.areas[0];
    if x.sheet != y.sheet {
        return Value::error(ErrorCode::Value);
    }
    Value::Ref(RefValue {
        areas: vec![Area {
            sheet: x.sheet,
            start_row: x.start_row.min(y.start_row),
            start_col: x.start_col.min(y.start_col),
            end_row: x.end_row.max(y.end_row),
            end_col: x.end_col.max(y.end_col),
        }],
    })
}

fn intersect_op(left: &Value, right: &Value) -> Value {
    let (Value::Ref(left), Value::Ref(right)) = (left, right) else {
        return Value::error(ErrorCode::Value);
    };
    let mut areas = Vec::new();
    for x in &left.areas {
        for y in &right.areas {
            if x.sheet != y.sheet {
                continue;
            }
            let start_row = x.start_row.max(y.start_row);
            let start_col = x.start_col.max(y.start_col);
            let end_row = x.end_row.min(y.end_row);
            let end_col = x.end_col.min(y.end_col);
            if start_row <= end_row && start_col <= end_col {
                areas.push(Area {
                    sheet: x.sheet,
                    start_row,
                    start_col,
                    end_row,
                    end_col,
                });
            }
        }
    }
    if areas.is_empty() {
        Value::error(ErrorCode::Null)
    } else {
        Value::Ref(RefValue { areas })
    }
}

fn union_op(left: &Value, right: &Value) -> Value {
    let (Value::Ref(left), Value::Ref(right)) = (left, right) else {
        return Value::error(ErrorCode::Value);
    };
    let mut areas = left.areas.clone();
    areas.extend(right.areas.iter().copied());
    Value::Ref(RefValue { areas })
}

fn operand_value(host: &GridHost, value: &Value, _at: Key) -> ScalarOrMatrix {
    match value {
        Value::Matrix(matrix) => ScalarOrMatrix::Matrix(matrix.clone()),
        Value::Scalar(scalar) => ScalarOrMatrix::Scalar(scalar.clone()),
        Value::Lambda(_) => ScalarOrMatrix::Scalar(Scalar::Error(ErrorCode::Calc)),
        Value::Ref(reference) => {
            if reference.is_cell() {
                let Some(area) = reference.areas.first() else {
                    return ScalarOrMatrix::Scalar(Scalar::Blank);
                };
                return ScalarOrMatrix::Scalar(host.cell_scalar(
                    area.sheet,
                    area.start_row,
                    area.start_col,
                ));
            }
            match host_matrix(host, reference) {
                Ok(matrix) => ScalarOrMatrix::Matrix(matrix),
                Err(code) => ScalarOrMatrix::Scalar(Scalar::Error(code)),
            }
        }
    }
}

fn host_matrix(host: &GridHost, reference: &RefValue) -> Result<Matrix, ErrorCode> {
    if reference.areas.len() != 1 {
        return Err(ErrorCode::Value);
    }
    let Some(area) = reference.areas.first().copied() else {
        return Err(ErrorCode::Value);
    };
    area_to_matrix(host, area)
}

fn unary_map(value: ScalarOrMatrix, mut func: impl FnMut(&Scalar) -> Scalar) -> Value {
    match value {
        ScalarOrMatrix::Scalar(scalar) => Value::Scalar(func(&scalar)),
        ScalarOrMatrix::Matrix(matrix) => Value::Matrix(matrix.map_values(|scalar| func(scalar))),
    }
}

fn invoke_lambda(
    host: &GridHost,
    lambda: &super::value::LambdaValue,
    args: &[Value],
    root: &FormulaAst,
    at: Key,
) -> Value {
    if args.len() > lambda.params.len() {
        return Value::error(ErrorCode::Value);
    }
    let mut scope = lambda.scope.clone();
    for (index, name) in lambda.params.iter().enumerate() {
        let value = args.get(index).cloned().unwrap_or_else(Value::blank);
        scope.insert(name.clone(), value);
    }
    eval_node(host, &lambda.body, root, at, &scope)
}

fn eval_call(
    host: &GridHost,
    name: &str,
    raw_name: &str,
    args: &[FormulaAst],
    root: &FormulaAst,
    at: Key,
    scope: &HashMap<String, Value>,
) -> Value {
    let local_key = crate::formula::ast::normalize_local_name(raw_name);
    if let Some(Value::Lambda(lambda)) = scope.get(&local_key) {
        let values: Vec<Value> = args
            .iter()
            .map(|arg| eval_node(host, arg, root, at, scope))
            .collect();
        return invoke_lambda(host, lambda, &values, root, at);
    }
    let Some(spec) = super::get_function(name) else {
        return Value::error(ErrorCode::Name);
    };
    if args.len() < spec.min_args || args.len() > spec.max_args {
        return Value::error(ErrorCode::Value);
    }
    let eval = |node: &FormulaAst| eval_node(host, node, root, at, scope);
    let eval_in =
        |node: &FormulaAst, inner: &HashMap<String, Value>| eval_node(host, node, root, at, inner);
    let call_lambda = |lambda: &super::value::LambdaValue, values: &[Value]| {
        invoke_lambda(host, lambda, values, root, at)
    };
    let depth = host.stack.borrow().len() as u32;
    let context = CallCtx {
        host,
        sheet: at.0,
        row: at.1,
        col: at.2,
        legacy: false,
        date1904: host.date1904,
        depth,
        scope,
        eval: Some(&eval),
        eval_in: Some(&eval_in),
        call_lambda: Some(&call_lambda),
    };
    if let FuncKind::Lazy { f } = spec.kind {
        return match f(&context, args) {
            Ok(value) => value,
            Err(code) => Value::error(code),
        };
    }
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        // `any` parameters keep references; both kinds evaluate the argument.
        values.push(eval_node(host, arg, root, at, scope));
    }
    apply_function(spec, values, &context)
}

/// Calls a registered function. `value` arguments lift over arrays; `any` arguments do not.
pub fn call_function(name: &str, args: &[Value], ctx: &CallCtx<'_>) -> Value {
    let Some(spec) = super::get_function(name) else {
        return Value::error(ErrorCode::Name);
    };
    if args.len() < spec.min_args || args.len() > spec.max_args {
        return Value::error(ErrorCode::Value);
    }
    apply_function(spec, args.to_vec(), ctx)
}

fn apply_function(spec: &FunctionSpec, mut args: Vec<Value>, ctx: &CallCtx<'_>) -> Value {
    let mut lifted = Vec::new();
    for (index, slot) in args.iter_mut().enumerate() {
        if param_kind(spec, index) != ParamKind::Value {
            continue;
        }
        let mut arg = slot.clone();
        if let Value::Ref(reference) = &arg {
            // A cell is read. A legacy formula intersects any larger reference.
            if reference.is_cell() || ctx.legacy {
                arg = Value::Scalar(ctx.to_scalar(&arg));
            } else {
                match ctx.to_matrix(&arg) {
                    Ok(matrix) => arg = Value::Matrix(matrix),
                    Err(code) => return Value::error(code),
                }
            }
        }
        if let Value::Matrix(matrix) = &arg {
            if matrix.rows() == 1 && matrix.cols() == 1 {
                arg = Value::Scalar(matrix.get(0, 0));
            } else {
                lifted.push(index);
            }
        }
        *slot = arg;
    }
    if lifted.is_empty() {
        return invoke(spec, &args, ctx);
    }
    let matrices: Vec<Matrix> = lifted
        .iter()
        .map(|index| match &args[*index] {
            Value::Matrix(matrix) => matrix.clone(),
            _ => Matrix::from_rows(vec![vec![Scalar::Blank]]),
        })
        .collect();
    match super::ops::broadcast_shape(&matrices, |row, col| {
        let element: Vec<Value> = args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                if param_kind(spec, index) == ParamKind::Value {
                    if let Value::Matrix(matrix) = arg {
                        if matrix.rows() != 1 || matrix.cols() != 1 {
                            return Value::Scalar(super::ops::pick(matrix, row, col));
                        }
                    }
                }
                arg.clone()
            })
            .collect();
        to_element(invoke(spec, &element, ctx), ctx)
    }) {
        Ok(matrix) => Value::Matrix(matrix),
        Err(code) => Value::error(code),
    }
}

fn invoke(spec: &FunctionSpec, args: &[Value], ctx: &CallCtx<'_>) -> Value {
    match spec.kind {
        FuncKind::Numeric { defaults, f } => numeric_invoke(spec.max_args, defaults, f, args, ctx),
        FuncKind::General { f } => match f(ctx, args) {
            Ok(value) => value,
            Err(code) => Value::error(code),
        },
        FuncKind::Lazy { .. } => Value::error(ErrorCode::Value),
    }
}

fn numeric_invoke(
    max_args: usize,
    defaults: &[Option<f64>],
    func: fn(&[f64]) -> Result<f64, ErrorCode>,
    args: &[Value],
    ctx: &CallCtx<'_>,
) -> Value {
    let mut values = Vec::new();
    for index in 0..max_args {
        if index < args.len() {
            match num(ctx, &args[index]) {
                Ok(value) => values.push(value),
                Err(code) => return Value::error(code),
            }
        } else if let Some(Some(default)) = defaults.get(index) {
            values.push(*default);
        }
    }
    match func(&values).and_then(finite) {
        Ok(value) => Value::number(value),
        Err(code) => Value::error(code),
    }
}

fn to_element(value: Value, ctx: &CallCtx<'_>) -> Scalar {
    match value {
        Value::Matrix(matrix) => matrix.get(0, 0),
        Value::Ref(_) => ctx.to_scalar(&value),
        Value::Scalar(scalar) => scalar,
        Value::Lambda(_) => Scalar::Error(ErrorCode::Calc),
    }
}

fn finalize(host: &GridHost, value: Value) -> Final {
    let current = match value {
        Value::Ref(reference) => {
            if reference.areas.len() != 1 {
                return Final::Scalar(Scalar::Error(ErrorCode::Value));
            }
            if reference.is_cell() {
                let area = reference.areas[0];
                Final::Scalar(host.cell_scalar(area.sheet, area.start_row, area.start_col))
            } else {
                match area_to_matrix(host, reference.areas[0]) {
                    Ok(matrix) => Final::Matrix(matrix),
                    Err(code) => return Final::Scalar(Scalar::Error(code)),
                }
            }
        }
        Value::Scalar(scalar) => Final::Scalar(scalar),
        Value::Matrix(matrix) => Final::Matrix(matrix),
        Value::Lambda(_) => return Final::Scalar(Scalar::Error(ErrorCode::Calc)),
    };
    match current {
        Final::Matrix(matrix) if matrix.rows() == 1 && matrix.cols() == 1 => {
            Final::Scalar(clean(matrix.get(0, 0)))
        }
        Final::Matrix(matrix) => {
            Final::Matrix(matrix.block().map_values(|value| clean(value.clone())))
        }
        Final::Scalar(scalar) => Final::Scalar(clean(scalar)),
    }
}

fn spill_check(host: &GridHost, key: Key, value: Final) -> Settled {
    let matrix = match value {
        Final::Scalar(scalar) => return Settled::scalar(scalar),
        Final::Matrix(matrix) => matrix,
    };
    if matrix.rows() == 0 || matrix.cols() == 0 {
        return Settled::scalar(Scalar::Error(ErrorCode::Value));
    }
    let end_row = key.1 as i64 + matrix.rows() as i64 - 1;
    let end_col = key.2 as i64 + matrix.cols() as i64 - 1;
    if end_row > i64::from(MAX_ROW) || end_col > i64::from(MAX_COL) {
        return Settled::scalar(Scalar::Error(ErrorCode::Spill));
    }
    for row in key.1..=end_row as i32 {
        for col in key.2..=end_col as i32 {
            if row == key.1 && col == key.2 {
                continue;
            }
            if host.occupied(key.0, row, col) {
                return Settled::scalar(Scalar::Error(ErrorCode::Spill));
            }
        }
    }
    let mut rows = Vec::with_capacity(matrix.rows());
    for row in 0..matrix.rows() {
        let mut line = Vec::with_capacity(matrix.cols());
        for col in 0..matrix.cols() {
            line.push(matrix.get(row, col));
        }
        rows.push(line);
    }
    Settled { rows }
}

fn tarjan(nodes: &[Key], edges: &HashMap<Key, Vec<Key>>) -> Vec<Vec<Key>> {
    let mut index_of = HashMap::new();
    for (index, key) in nodes.iter().enumerate() {
        index_of.insert(*key, index);
    }
    let mut indices = vec![None; nodes.len()];
    let mut low = vec![0usize; nodes.len()];
    let mut on_stack = vec![false; nodes.len()];
    let mut stack = Vec::new();
    let mut next_index = 0usize;
    let mut components = Vec::new();
    for start in 0..nodes.len() {
        if indices[start].is_none() {
            strong(
                start,
                nodes,
                edges,
                &index_of,
                &mut indices,
                &mut low,
                &mut on_stack,
                &mut stack,
                &mut next_index,
                &mut components,
            );
        }
    }
    components
}

#[allow(clippy::too_many_arguments)]
fn strong(
    at: usize,
    nodes: &[Key],
    edges: &HashMap<Key, Vec<Key>>,
    index_of: &HashMap<Key, usize>,
    indices: &mut [Option<usize>],
    low: &mut [usize],
    on_stack: &mut [bool],
    stack: &mut Vec<usize>,
    next_index: &mut usize,
    components: &mut Vec<Vec<Key>>,
) {
    indices[at] = Some(*next_index);
    low[at] = *next_index;
    *next_index += 1;
    stack.push(at);
    on_stack[at] = true;
    if let Some(next) = edges.get(&nodes[at]) {
        for key in next {
            let Some(&other) = index_of.get(key) else {
                continue;
            };
            if indices[other].is_none() {
                strong(
                    other, nodes, edges, index_of, indices, low, on_stack, stack, next_index,
                    components,
                );
                low[at] = low[at].min(low[other]);
            } else if on_stack[other] {
                low[at] = low[at].min(indices[other].unwrap_or(0));
            }
        }
    }
    if low[at] == indices[at].unwrap_or(0) {
        let mut component = Vec::new();
        while let Some(item) = stack.pop() {
            on_stack[item] = false;
            component.push(nodes[item]);
            if item == at {
                break;
            }
        }
        components.push(component);
    }
}

/// Evaluates `formula` at `sheet`/`row`/`col` and returns the cleaned spill as JSON rows.
pub fn evaluate_formula(
    formula: &str,
    host: &mut GridHost,
    sheet: usize,
    row: i32,
    col: i32,
) -> Json {
    host.set_formula(sheet, row, col, formula);
    host.anchor.set((sheet, row, col));
    host.run().rows_json()
}

/// Differential dump: one `{formula, rows}` object per case.
pub fn dump_fn_cases(cases: &Json) -> Json {
    let Some(list) = cases.as_array() else {
        return Json::Array(Vec::new());
    };
    Json::Array(list.iter().map(dump_one).collect())
}

fn dump_one(case: &Json) -> Json {
    let formula = case
        .get("formula")
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_owned();
    let mut names: Vec<String> = case
        .get("sheets")
        .and_then(Json::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Json::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() {
        names = vec!["Sheet1".to_owned(), "Sheet2".to_owned()];
    }
    let borrowed: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut host = GridHost::new(&borrowed);
    host.date1904 = case
        .get("date1904")
        .and_then(Json::as_bool)
        .unwrap_or(false);
    if let Some(cells) = case.get("cells").and_then(Json::as_object) {
        for (key, value) in cells {
            if let Some((sheet, row, col)) = parse_cell_key(&host, key) {
                match slot_from_json(value) {
                    Slot::Const(scalar) => host.set_const(sheet, row, col, scalar),
                    Slot::Formula(text) => host.set_formula(sheet, row, col, &text),
                }
            }
        }
    }
    if let Some(hidden) = case.get("hidden").and_then(Json::as_object) {
        for (name, rows) in hidden {
            let Some(sheet) = host.find_sheet(name) else {
                continue;
            };
            let Some(rows) = rows.as_array() else {
                continue;
            };
            for row in rows {
                if let Some(row) = row.as_i64() {
                    host.set_hidden(sheet, row as i32);
                }
            }
        }
    }
    let sheet = case
        .get("sheet")
        .and_then(Json::as_str)
        .and_then(|name| host.find_sheet(name))
        .unwrap_or(0);
    let row = case.get("row").and_then(Json::as_i64).unwrap_or(0) as i32;
    let col = case.get("col").and_then(Json::as_i64).unwrap_or(26) as i32;
    let rows = evaluate_formula(&formula, &mut host, sheet, row, col);
    json!({
        "formula": formula,
        "rows": rows,
    })
}

fn parse_cell_key(host: &GridHost, key: &str) -> Option<(usize, i32, i32)> {
    let (sheet_name, addr) = match key.rfind('!') {
        Some(index) => (&key[..index], &key[index + 1..]),
        None => (host.names.first().map(String::as_str)?, key),
    };
    let sheet = host.find_sheet(sheet_name)?;
    let address = parse_address(addr)?;
    Some((sheet, address.row as i32, address.col as i32))
}

fn slot_from_json(value: &Json) -> Slot {
    match value {
        Json::Null => Slot::Const(Scalar::Blank),
        Json::Bool(bit) => Slot::Const(Scalar::Bool(*bit)),
        Json::Number(number) => Slot::Const(Scalar::Number(number.as_f64().unwrap_or(0.0))),
        Json::String(text) if text.starts_with('=') => Slot::Formula(text[1..].to_owned()),
        Json::String(text) => Slot::Const(Scalar::Text(text.clone())),
        Json::Object(map) => {
            let code = map
                .get("error")
                .and_then(Json::as_str)
                .and_then(ErrorCode::parse)
                .unwrap_or(ErrorCode::Value);
            Slot::Const(Scalar::Error(code))
        }
        Json::Array(_) => Slot::Const(Scalar::Blank),
    }
}
