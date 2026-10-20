// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/{engine,engine-core,engine-host,
// engine-prepare,engine-order,engine-types,engine-util,evaluator,operators,coerce,text-number,
// references,values,store,spill,date-serial}.ts; changes listed in PROVENANCE.md.
//! Dependency-graph recalculation. Catalogued function calls evaluate through
//! [`crate::formula::fns`]; an unknown name is `#NAME?`.

use crate::address::{parse_range, MAX_COL, MAX_ROW};
use crate::formula::ast::{
    normalize_local_name, ArrayConstant, BinaryOp, FormulaAst, FormulaError, SheetPrefix,
    StructuredRef, StructuredSpecial, UnaryOp,
};
use crate::formula::fns::{
    call_function, get_function, param_kind, Area as FnArea, CallCtx, ErrorCode, EvalHost,
    FuncKind, LambdaValue, Matrix as FnMatrix, ParamKind, RefValue, Scalar as FnScalar,
    Value as FnValue,
};
use crate::formula::graph::{
    analyze, bounding_box, cell_range, formula_order, prefix_sheets, range_has, ranges_intersect,
    same_range, sheet_index, spec_range, Area, AreaIndex, ColumnIndex, DepHost, FormulaNode,
    InsSet, ReverseIndex,
};
use crate::formula::parser::parse_formula;
use crate::model::{CellAddress, CellRange, DumpCellValue};
use crate::number_format::js_number_string;
use crate::sheet_data_source::split_sheet_data;
use crate::workbook::{load_workbook, parse_all_worksheets};
use crate::xml_util::{att, num_attr, rel_attr, x_children, x_first};
use nitro_xml_opc::opc::{relationship_types, Package, Relationship};
use nitro_xml_opc::xml::parse_xml_str;
use regex::Regex;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

/// Shown once when a recalculation finds a circular reference.
pub const CIRCULAR_REFERENCE_WARNING: &str =
    "The workbook contains circular references; the cells involved show 0.";

const MAX_ROUNDS: usize = 8;
const MAX_NESTED_COMPUTE: u32 = 40;
const MAX_NAME_DEPTH: u32 = 32;
const CHECK_EVERY: u32 = 32;
const CLIP_THRESHOLD: u64 = 100_000;
const MAX_MATRIX_CELLS: u64 = 4_000_000;
const MAX_ARRAY_CELLS: u64 = 16_777_216;
const MAX_ARRAY_SIDE: u32 = 1_048_576;

/// A formula cell the engine was asked to recalculate from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPos {
    pub sheet: u32,
    pub row: u32,
    pub col: u32,
}

/// One cell copied from the edit host into the engine.
#[derive(Debug, Clone)]
pub struct CalcCellSeed {
    pub row: u32,
    pub col: u32,
    pub value: DumpCellValue,
    pub formula: Option<String>,
    pub spill_anchor: Option<CellAddress>,
    pub legacy: bool,
    pub array_range: Option<CellRange>,
}

/// One sheet of [`CalcBookSeed`].
#[derive(Debug, Clone)]
pub struct CalcSheetSeed {
    pub merges: Vec<CellRange>,
    pub cells: Vec<CalcCellSeed>,
    /// Rows with `rowInfo.hidden`.
    pub hidden_rows: Vec<u32>,
}

/// Workbook grid the editor installs after a structural edit the graph cannot follow.
#[derive(Debug, Clone)]
pub struct CalcBookSeed {
    pub sheet_names: Vec<String>,
    pub date1904: bool,
    pub iterate: Option<(f64, f64)>,
    pub defined: Vec<(String, String, Option<u32>)>,
    pub sheets: Vec<CalcSheetSeed>,
}

/// A formula or spill cell after recalculation, ready to copy back onto the host.
#[derive(Debug, Clone)]
pub struct CalculatedCell {
    pub sheet: u32,
    pub row: u32,
    pub col: u32,
    pub value: DumpCellValue,
    pub has_formula: bool,
    pub spill_anchor: Option<CellAddress>,
}

#[derive(Debug, Clone, PartialEq)]
enum Scalar {
    Empty,
    Number(f64),
    Text(String),
    Bool(bool),
    Error(String),
}

impl Scalar {
    fn error(code: &str) -> Self {
        Self::Error(code.to_owned())
    }

    fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

#[derive(Debug, Clone)]
enum Value {
    Scalar(Scalar),
    Ref(Vec<Area>),
    Matrix(Matrix),
    Lambda(Box<LambdaValue>),
}

impl Value {
    fn err(code: &str) -> Self {
        Self::Scalar(Scalar::error(code))
    }

    fn number(n: f64) -> Self {
        Self::Scalar(nz(n))
    }
}

#[derive(Debug, Clone)]
struct Matrix {
    dense: Vec<Vec<Scalar>>,
    pad_rows: u32,
    pad_cols: u32,
    fill: Scalar,
}

impl Matrix {
    fn plain(dense: Vec<Vec<Scalar>>) -> Self {
        Self {
            dense,
            pad_rows: 0,
            pad_cols: 0,
            fill: Scalar::Empty,
        }
    }

    fn stored_rows(&self) -> u32 {
        self.dense.len() as u32
    }

    fn stored_cols(&self) -> u32 {
        self.dense.first().map(|row| row.len()).unwrap_or(0) as u32
    }

    fn is_padded(&self) -> bool {
        self.pad_rows > self.stored_rows() || self.pad_cols > self.stored_cols()
    }

    fn rows(&self) -> u32 {
        if self.pad_rows > self.stored_rows() {
            self.pad_rows
        } else {
            self.stored_rows()
        }
    }

    fn cols(&self) -> u32 {
        if self.pad_cols > self.stored_cols() {
            self.pad_cols
        } else {
            self.stored_cols()
        }
    }

    fn padded(mut self, rows: u32, cols: u32, fill: Scalar) -> Self {
        if rows > self.stored_rows() || cols > self.stored_cols() {
            self.pad_rows = rows;
            self.pad_cols = cols;
            self.fill = fill;
        }
        self
    }

    fn block(mut self) -> Self {
        self.pad_rows = 0;
        self.pad_cols = 0;
        self.fill = Scalar::Empty;
        self
    }

    fn get(&self, row: u32, col: u32) -> Scalar {
        if let Some(line) = self.dense.get(row as usize) {
            if let Some(value) = line.get(col as usize) {
                return value.clone();
            }
        }
        if self.is_padded() && row < self.pad_rows && col < self.pad_cols {
            return self.fill.clone();
        }
        Scalar::Empty
    }

    fn map_values(&self, mut f: impl FnMut(&Scalar) -> Scalar) -> Self {
        let dense = self
            .dense
            .iter()
            .map(|line| line.iter().map(&mut f).collect())
            .collect();
        let fill = if self.is_padded() {
            f(&self.fill)
        } else {
            Scalar::Empty
        };
        Self {
            dense,
            pad_rows: self.pad_rows,
            pad_cols: self.pad_cols,
            fill,
        }
    }
}

enum Stored {
    Scalar(Scalar),
    Matrix(Matrix),
}

enum Halt {
    Deferred(u32),
}

#[derive(Clone)]
struct TableMeta {
    name: String,
    display: String,
    range: CellRange,
    header: bool,
    totals: bool,
    columns: Vec<String>,
}

struct NameDef {
    name: String,
    formula: String,
    local_sheet: Option<u32>,
}

struct HostCell {
    value: Scalar,
    formula: Option<String>,
    spill_anchor: Option<CellAddress>,
    legacy: bool,
    array_range: Option<CellRange>,
}

struct SheetGrid {
    cells: HashMap<u64, HostCell>,
    rows: BTreeMap<u32, BTreeSet<u32>>,
    merges: Vec<CellRange>,
    tables: Vec<TableMeta>,
    hidden: BTreeSet<u32>,
}

struct FpChange {
    sheet: u32,
    range: CellRange,
    at: i64,
}

struct Frame<'a> {
    sheet: u32,
    row: u32,
    col: u32,
    depth: u32,
    root: Option<&'a FormulaAst>,
    legacy: bool,
    scope: &'a HashMap<String, FnValue>,
}

/// Workbook-bound calc engine (prepare, recalculate, circular cells).
pub struct CalcEngine {
    sheet_names: Vec<String>,
    date1904: bool,
    iterate: Option<(f64, f64)>,
    warnings: Vec<String>,
    defined: Vec<NameDef>,
    sheets: Vec<SheetGrid>,
    nodes: Vec<FormulaNode>,
    by_cell: HashMap<(u32, u32, u32), u32>,
    formula_locs: Vec<(u32, u32, u32)>,
    reverse: ReverseIndex,
    prep_at: usize,
    dynamic_ids: Vec<u32>,
    unvalued_ids: Vec<u32>,
    built: bool,
    needs_full: bool,
    fresh: bool,
    ranked: bool,
    eval_active: bool,
    iterating: bool,
    pending: HashSet<u32>,
    in_progress: HashSet<u32>,
    cycles: InsSet,
    dynamic_hits: HashSet<u32>,
    dynamic_hit: bool,
    compute_depth: u32,
    evaluations: i64,
    evaluated_at: HashMap<u32, i64>,
    footprint_changes: Vec<FpChange>,
    bounds_cache: Vec<Option<(u32, u32)>>,
    parsed: HashMap<String, Result<FormulaAst, FormulaError>>,
    /// Seeded `RAND` / `RANDBETWEEN`, matching the function harness.
    rand_seed: Cell<u64>,
}

impl CalcEngine {
    /// Load every worksheet of `bytes` into a calc engine. Nodes are built on [`prepare`].
    pub fn load(bytes: &[u8]) -> Result<Self, String> {
        let workbook = load_workbook(bytes)?;
        let parsed = parse_all_worksheets(bytes, &workbook)?;
        let sst = workbook.shared_strings.clone();
        let parsed: Vec<Option<crate::model::ActiveWorksheet>> = parsed
            .into_iter()
            .map(|opt| {
                opt.map(|mut ws| {
                    ws.ensure_dump_cells(&sst);
                    ws
                })
            })
            .collect();
        let parts: Vec<Option<String>> = workbook
            .sheets
            .iter()
            .map(|sheet| sheet.part_name.clone())
            .collect();
        let tables = load_tables(bytes, &parts)?;
        let mut sheets = Vec::with_capacity(workbook.sheets.len());
        let mut formula_locs = Vec::new();
        for (index, stub) in workbook.sheets.iter().enumerate() {
            let worksheet = parsed.get(index).and_then(|sheet| sheet.as_ref());
            let mut hidden = BTreeSet::new();
            if let Some(worksheet) = worksheet {
                for (row, info) in &worksheet.row_info {
                    if info.hidden == Some(true) {
                        hidden.insert(*row);
                    }
                }
            }
            let mut grid = SheetGrid {
                cells: HashMap::new(),
                rows: BTreeMap::new(),
                merges: worksheet
                    .map(|sheet| sheet.merges.clone())
                    .unwrap_or_default(),
                tables: tables.get(index).cloned().unwrap_or_default(),
                hidden,
            };
            if let Some(worksheet) = worksheet {
                for cell in &worksheet.cells {
                    let host = HostCell {
                        value: scalar_from_dump(&cell.value),
                        formula: cell.formula.clone(),
                        spill_anchor: cell.spill_anchor,
                        legacy: cell.legacy_formula == Some(true) && cell.array_range.is_none(),
                        array_range: cell.array_range.clone(),
                    };
                    if host.formula.is_some() {
                        formula_locs.push((index as u32, cell.row, cell.col));
                    }
                    grid.insert(cell.row, cell.col, host);
                }
            }
            let _ = stub;
            sheets.push(grid);
        }
        let defined = workbook
            .defined_names
            .iter()
            .map(|name| NameDef {
                name: name.name.clone(),
                formula: name.formula.clone(),
                local_sheet: name.local_sheet,
            })
            .collect();
        Ok(Self {
            sheet_names: workbook
                .sheets
                .iter()
                .map(|sheet| sheet.name.clone())
                .collect(),
            date1904: workbook.date1904,
            iterate: workbook.iterate,
            warnings: Vec::new(),
            defined,
            bounds_cache: vec![None; sheets.len()],
            sheets,
            nodes: Vec::new(),
            by_cell: HashMap::new(),
            formula_locs,
            reverse: ReverseIndex::new(),
            prep_at: 0,
            dynamic_ids: Vec::new(),
            unvalued_ids: Vec::new(),
            built: false,
            needs_full: false,
            fresh: true,
            ranked: false,
            eval_active: false,
            iterating: false,
            pending: HashSet::new(),
            in_progress: HashSet::new(),
            cycles: InsSet::default(),
            dynamic_hits: HashSet::new(),
            dynamic_hit: false,
            compute_depth: 0,
            evaluations: 0,
            evaluated_at: HashMap::new(),
            footprint_changes: Vec::new(),
            parsed: HashMap::new(),
            rand_seed: Cell::new(42),
        })
    }

    /// Build the dependency graph. `time_remaining` pauses the walk the way an idle deadline does.
    pub fn prepare(&mut self, time_remaining: Option<&dyn Fn() -> f64>) -> bool {
        if self.built || self.needs_full {
            return true;
        }
        let mut visited = 0u32;
        while self.prep_at < self.formula_locs.len() {
            if let Some(clock) = time_remaining {
                if visited > 0 && visited.is_multiple_of(CHECK_EVERY) && clock() <= 0.0 {
                    return false;
                }
            }
            let (sheet, row, col) = self.formula_locs[self.prep_at];
            self.create_node(sheet, row, col, true);
            self.prep_at += 1;
            visited += 1;
        }
        self.built = true;
        true
    }

    /// Drop a graph prepared before the first recalculation.
    pub fn discard_preparation(&mut self) {
        if !self.fresh || self.needs_full {
            return;
        }
        self.nodes.clear();
        self.by_cell.clear();
        self.reverse = ReverseIndex::new();
        self.prep_at = 0;
        self.dynamic_ids.clear();
        self.unvalued_ids.clear();
        self.built = false;
        self.ranked = false;
    }

    /// Evaluate every formula cell.
    pub fn recalculate_all(&mut self) {
        self.fresh = false;
        self.dynamic_ids.clear();
        self.unvalued_ids.clear();
        if self.needs_full {
            self.release_spills();
            self.rebuild();
        } else if !self.built {
            let _ = self.prepare(None);
        }
        self.needs_full = false;
        self.cycles = InsSet::default();
        self.ranked = false;
        let all: Vec<u32> = (0..self.nodes.len() as u32)
            .filter(|id| self.nodes[*id as usize].alive)
            .collect();
        self.run(all, true);
    }

    /// Recalculate formulas that read `changes`, plus every volatile formula.
    pub fn recalculate_from(&mut self, changes: &[CellPos]) {
        let mut missing = Vec::new();
        if self.fresh && !self.needs_full {
            self.fresh = false;
            if !self.built {
                let _ = self.prepare(None);
            }
            if !self.trusts_stored(changes, &mut missing) {
                self.recalculate_all();
                return;
            }
        } else if !self.built || self.needs_full {
            self.recalculate_all();
            return;
        }
        let mut seeds = HashSet::new();
        for id in missing {
            seeds.insert(id);
        }
        let mut changed = Vec::new();
        let mut structural = false;
        for change in changes {
            let Some(grid) = self.sheets.get(change.sheet as usize) else {
                continue;
            };
            let key = pos_key(change.row, change.col);
            let formula = grid.cells.get(&key).and_then(|cell| cell.formula.clone());
            let existing = self
                .by_cell
                .get(&(change.sheet, change.row, change.col))
                .copied();
            if let Some(id) = existing {
                if self.nodes[id as usize].formula != formula.as_deref().unwrap_or("") {
                    self.remove_node(id);
                    structural = true;
                }
            }
            let existing = self
                .by_cell
                .get(&(change.sheet, change.row, change.col))
                .copied();
            if formula.is_some() && existing.is_none() {
                if let Some(id) = self.create_node(change.sheet, change.row, change.col, false) {
                    seeds.insert(id);
                    structural = true;
                }
            } else if let Some(id) = existing {
                if formula.is_some() {
                    seeds.insert(id);
                }
            }
            changed.push(Area {
                sheet: change.sheet,
                range: cell_range(change.row, change.col),
            });
            let mut anchors = InsSet::default();
            let footprints = build_footprints(&self.nodes);
            footprints.dependents_into(
                change.sheet,
                &cell_range(change.row, change.col),
                &mut anchors,
            );
            for anchor in anchors.order {
                let node = &self.nodes[anchor as usize];
                let fp = node.spill.clone().or(node.blocked_spill.clone());
                if let Some(fp) = fp {
                    if range_has(&fp, change.row, change.col)
                        && !(node.row == change.row && node.col == change.col)
                    {
                        seeds.insert(anchor);
                    }
                }
            }
        }
        if structural {
            self.rebuild_reverse();
            self.ranked = false;
        }
        for area in &changed {
            let mut found = InsSet::default();
            self.reverse.dependents(area.sheet, &area.range, &mut found);
            for id in found.order {
                seeds.insert(id);
            }
        }
        for (id, node) in self.nodes.iter().enumerate() {
            if node.volatile {
                seeds.insert(id as u32);
            }
        }
        self.run(seeds.into_iter().collect(), false);
    }

    /// Forget the graph. The next recalculation rebuilds it.
    pub fn invalidate(&mut self) {
        self.release_spills();
        self.nodes.clear();
        self.by_cell.clear();
        self.reverse = ReverseIndex::new();
        self.prep_at = 0;
        self.dynamic_ids.clear();
        self.unvalued_ids.clear();
        self.built = false;
        self.needs_full = true;
        self.ranked = false;
        self.cycles = InsSet::default();
    }

    /// Warnings recorded by the last recalculation (circular references).
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Replace the grid from the edit host and mark the dependency graph stale.
    ///
    /// `CalcEngine` has no `shift_cells` / `rename_sheet`. Structural edits reload
    /// the cells the host already shifted, then [`Self::invalidate`] plus
    /// [`Self::recalculate_all`] rebuild every formula.
    pub fn replace_book(&mut self, seed: CalcBookSeed) {
        self.invalidate();
        self.sheet_names = seed.sheet_names;
        self.date1904 = seed.date1904;
        self.iterate = seed.iterate;
        self.defined = seed
            .defined
            .into_iter()
            .map(|(name, formula, local_sheet)| NameDef {
                name,
                formula,
                local_sheet,
            })
            .collect();
        let mut sheets = Vec::with_capacity(seed.sheets.len());
        let mut formula_locs = Vec::new();
        for (index, sheet) in seed.sheets.into_iter().enumerate() {
            let mut grid = SheetGrid {
                cells: HashMap::new(),
                rows: BTreeMap::new(),
                merges: sheet.merges,
                tables: Vec::new(),
                hidden: sheet.hidden_rows.into_iter().collect(),
            };
            for cell in sheet.cells {
                let empty = matches!(cell.value, DumpCellValue::Null)
                    && cell.formula.is_none()
                    && cell.spill_anchor.is_none();
                if empty {
                    continue;
                }
                if cell.formula.is_some() {
                    formula_locs.push((index as u32, cell.row, cell.col));
                }
                grid.insert(
                    cell.row,
                    cell.col,
                    HostCell {
                        value: scalar_from_dump(&cell.value),
                        formula: cell.formula,
                        spill_anchor: cell.spill_anchor,
                        legacy: cell.legacy,
                        array_range: cell.array_range,
                    },
                );
            }
            sheets.push(grid);
        }
        self.sheets = sheets;
        self.bounds_cache = vec![None; self.sheets.len()];
        self.formula_locs = formula_locs;
        self.needs_full = true;
        self.fresh = false;
    }

    /// Copy one edited cell into the grid before [`Self::recalculate_from`].
    pub fn sync_input(&mut self, sheet: u32, input: CalcCellSeed) {
        let Some(grid) = self.sheets.get_mut(sheet as usize) else {
            return;
        };
        let row = input.row;
        let col = input.col;
        if matches!(input.value, DumpCellValue::Null) && input.formula.is_none() {
            grid.remove_at(row, col);
            return;
        }
        let scalar = scalar_from_dump(&input.value);
        let key = pos_key(row, col);
        if let Some(cell) = grid.cells.get_mut(&key) {
            let formula_changed = cell.formula != input.formula;
            cell.value = scalar;
            cell.formula = input.formula;
            cell.legacy = input.legacy;
            cell.array_range = input.array_range;
            if formula_changed {
                cell.spill_anchor = None;
            }
        } else {
            grid.insert(
                row,
                col,
                HostCell {
                    value: scalar,
                    formula: input.formula,
                    spill_anchor: None,
                    legacy: input.legacy,
                    array_range: input.array_range,
                },
            );
        }
    }

    /// Formula anchors and spill cells produced by the last recalculation.
    pub fn export_calculated(&self) -> Vec<CalculatedCell> {
        let mut out = Vec::new();
        for (sheet, grid) in self.sheets.iter().enumerate() {
            for (&row, cols) in &grid.rows {
                for &col in cols {
                    let Some(cell) = grid.cells.get(&pos_key(row, col)) else {
                        continue;
                    };
                    if cell.formula.is_none() && cell.spill_anchor.is_none() {
                        continue;
                    }
                    out.push(CalculatedCell {
                        sheet: sheet as u32,
                        row,
                        col,
                        value: scalar_to_dump(&cell.value),
                        has_formula: cell.formula.is_some(),
                        spill_anchor: cell.spill_anchor,
                    });
                }
            }
        }
        out
    }

    /// Formula cells marked circular by the last recalculation, in discovery order.
    pub fn circular_cells(&self) -> Vec<CellPos> {
        self.cycles
            .order
            .iter()
            .map(|id| {
                let node = &self.nodes[*id as usize];
                CellPos {
                    sheet: node.sheet,
                    row: node.row,
                    col: node.col,
                }
            })
            .collect()
    }

    /// JSON array of formula cells after recalculation, in `(sheet, row, col)` order.
    pub fn cells_json(&self) -> String {
        let mut out = String::from("[");
        let mut started = false;
        for (index, node) in self.nodes.iter().enumerate() {
            if !node.alive {
                continue;
            }
            if started {
                out.push(',');
            }
            started = true;
            let value = self.cell_scalar(node.sheet, node.row, node.col);
            let circular = self.cycles.contains(index as u32);
            out.push_str(&format!(
                "{{\"sheet\":{},\"row\":{},\"col\":{},\"formula\":{},\"rank\":{},\"cyclic\":{},\"circular\":{},\"volatile\":{},\"unsupported\":{},\"keepCached\":{},\"hasCall\":{},\"deps\":{},\"dynamicDeps\":{},\"value\":{}}}",
                node.sheet,
                node.row,
                node.col,
                json_string(&node.formula),
                node.rank,
                bool_json(node.cyclic),
                bool_json(circular),
                bool_json(node.volatile),
                bool_json(node.unsupported),
                bool_json(node.keep_cached),
                bool_json(node.has_call),
                areas_json(&node.deps),
                areas_json(&node.dynamic_deps),
                scalar_json(&value),
            ));
        }
        out.push(']');
        out
    }

    fn trusts_stored(&self, changes: &[CellPos], missing: &mut Vec<u32>) -> bool {
        let changed: HashSet<(u32, u32, u32)> = changes
            .iter()
            .map(|change| (change.sheet, change.row, change.col))
            .collect();
        for id in &self.dynamic_ids {
            let node = &self.nodes[*id as usize];
            let key = (node.sheet, node.row, node.col);
            if self.by_cell.get(&key) == Some(id) && !changed.contains(&key) {
                return false;
            }
        }
        for id in &self.unvalued_ids {
            let node = &self.nodes[*id as usize];
            if self.by_cell.get(&(node.sheet, node.row, node.col)) != Some(id) {
                continue;
            }
            if self.cell_scalar(node.sheet, node.row, node.col).is_empty() {
                missing.push(*id);
            }
        }
        true
    }

    fn rebuild(&mut self) {
        self.nodes.clear();
        self.by_cell.clear();
        self.reverse = ReverseIndex::new();
        self.prep_at = 0;
        self.dynamic_ids.clear();
        self.unvalued_ids.clear();
        self.built = false;
        self.needs_full = false;
        self.formula_locs.clear();
        for (sheet_index, sheet) in self.sheets.iter().enumerate() {
            for (row, cols) in &sheet.rows {
                for col in cols {
                    let key = pos_key(*row, *col);
                    if sheet
                        .cells
                        .get(&key)
                        .and_then(|cell| cell.formula.as_ref())
                        .is_some()
                    {
                        self.formula_locs.push((sheet_index as u32, *row, *col));
                    }
                }
            }
        }
        let _ = self.prepare(None);
    }

    fn create_node(&mut self, sheet: u32, row: u32, col: u32, initial: bool) -> Option<u32> {
        let formula = self.cell_formula(sheet, row, col)?;
        let legacy = self.cell_legacy(sheet, row, col);
        let array_range = self.cell_array_range(sheet, row, col);
        let value_empty = self.cell_scalar(sheet, row, col).is_empty();
        let parsed = self.parse_cached(&formula);
        let mut node = FormulaNode::blank(sheet, row, col, formula);
        match parsed {
            Ok(ast) => {
                let info = analyze(&ast, self, sheet, row, col);
                node.ast = Some(ast);
                node.deps = info.deps;
                node.dynamic_deps = info.dynamic_deps;
                node.volatile = info.volatile;
                node.unsupported = info.unsupported;
                node.has_call = info.has_call;
            }
            Err(err) => {
                if let Some(code) = err.code {
                    node.parse_error = Some(code);
                }
            }
        }
        if let Some(range) = array_range.clone() {
            node.array_range = Some(range);
        } else if legacy {
            node.legacy = true;
        }
        node.keep_cached = initial && (node.ast.is_none() || node.unsupported) && !value_empty;
        let dynamic = !node.legacy && node.array_range.is_none();
        let unvalued = value_empty && !node.keep_cached;
        let deps = node.deps.clone();
        let dynamic_deps = node.dynamic_deps.clone();
        let id = self.nodes.len() as u32;
        self.nodes.push(node);
        self.by_cell.insert((sheet, row, col), id);
        self.reverse.add_node(id, &deps, &dynamic_deps);
        if dynamic {
            self.dynamic_ids.push(id);
        }
        if unvalued {
            self.unvalued_ids.push(id);
        }
        Some(id)
    }

    fn remove_node(&mut self, id: u32) {
        let node = &self.nodes[id as usize];
        let sheet = node.sheet;
        let row = node.row;
        let col = node.col;
        if let Some(spill) = node.spill.clone() {
            self.clear_footprint(sheet, &spill, row, col);
        }
        self.by_cell.remove(&(sheet, row, col));
        self.nodes[id as usize].alive = false;
        self.nodes[id as usize].formula.clear();
        self.nodes[id as usize].deps.clear();
        self.nodes[id as usize].dynamic_deps.clear();
        self.ranked = false;
    }

    fn run(&mut self, seeds: Vec<u32>, everything: bool) {
        let mut current: HashSet<u32> = seeds.into_iter().collect();
        for round in 0..MAX_ROUNDS {
            if current.is_empty() {
                break;
            }
            let dirty = if everything && round == 0 {
                current.clone()
            } else {
                self.closure(&current)
            };
            self.footprint_changes.clear();
            self.evaluated_at.clear();
            self.evaluate_set(&dirty, everything && round == 0);
            if self.footprint_changes.is_empty() {
                break;
            }
            let changes = std::mem::take(&mut self.footprint_changes);
            let mut next = HashSet::new();
            for change in &changes {
                let mut found = InsSet::default();
                self.reverse
                    .dependents(change.sheet, &change.range, &mut found);
                for id in found.order {
                    let at = self.evaluated_at.get(&id).copied().unwrap_or(-1);
                    if at <= change.at {
                        next.insert(id);
                    }
                }
            }
            self.footprint_changes = changes;
            current = next;
            self.footprint_changes.clear();
        }
        if !self.cycles.order.is_empty()
            && !self
                .warnings
                .iter()
                .any(|warning| warning == CIRCULAR_REFERENCE_WARNING)
        {
            self.warnings.push(CIRCULAR_REFERENCE_WARNING.to_owned());
        }
    }

    fn closure(&self, seeds: &HashSet<u32>) -> HashSet<u32> {
        let mut out = seeds.clone();
        let mut queue: Vec<u32> = seeds.iter().copied().collect();
        while let Some(id) = queue.pop() {
            let node = &self.nodes[id as usize];
            let mut found = InsSet::default();
            self.reverse
                .dependents(node.sheet, &cell_range(node.row, node.col), &mut found);
            if let Some(fp) = node.spill.clone().or(node.array_range.clone()) {
                self.reverse.dependents(node.sheet, &fp, &mut found);
            }
            for dep in found.order {
                if out.insert(dep) {
                    queue.push(dep);
                }
            }
        }
        out
    }

    fn evaluate_set(&mut self, dirty: &HashSet<u32>, full: bool) {
        self.bounds_cache.fill(None);
        let mut order_ids: Vec<u32> = dirty.iter().copied().collect();
        order_ids.sort_unstable();
        let components = {
            let columns = ColumnIndex::build(&self.nodes);
            let footprints = build_footprints(&self.nodes);
            let mut cache: HashMap<u32, Vec<u32>> = HashMap::new();
            let nodes = &self.nodes;
            formula_order(&order_ids, &mut |id| {
                if let Some(cached) = cache.get(&id) {
                    return cached.clone();
                }
                let mut found = Vec::new();
                let mut anchors = InsSet::default();
                for dep in &nodes[id as usize].deps {
                    columns.nodes_in(dep, &mut found);
                    footprints.dependents_into(dep.sheet, &dep.range, &mut anchors);
                }
                for anchor in &anchors.order {
                    let node = &nodes[*anchor as usize];
                    if node.alive && (node.spill.is_some() || node.array_range.is_some()) {
                        found.push(*anchor);
                    }
                }
                let mut seen = InsSet::default();
                let mut out = Vec::new();
                for candidate in found {
                    if dirty.contains(&candidate) && seen.insert(candidate) {
                        out.push(candidate);
                    }
                }
                cache.insert(id, out.clone());
                out
            })
            .0
        };
        if full && !self.ranked {
            self.ranked = true;
            for (rank, component) in components.iter().enumerate() {
                for id in &component.nodes {
                    let node = &mut self.nodes[*id as usize];
                    node.rank = rank as u32;
                    node.cyclic = component.cyclic;
                    node.ranked = true;
                }
            }
        }
        self.pending.clone_from(dirty);
        self.dynamic_hits.clear();
        self.eval_active = true;
        for component in &components {
            let first = component.nodes[0];
            if component.cyclic {
                if let Some(limits) = self.iterate {
                    self.iterate_group(&component.nodes, limits);
                } else {
                    for id in &component.nodes {
                        self.mark_cycle(*id);
                    }
                }
            } else if self.pending.contains(&first) {
                self.compute_from(first);
            }
        }
        self.eval_active = false;
        self.pending.clear();
    }

    fn compute_from(&mut self, root: u32) {
        let mut stack = vec![root];
        let mut waiting = HashSet::from([root]);
        while let Some(&node) = stack.last() {
            if !self.pending.contains(&node) {
                if let Some(done) = stack.pop() {
                    waiting.remove(&done);
                }
                continue;
            }
            match self.compute(node) {
                Ok(()) => {}
                Err(Halt::Deferred(need)) => {
                    if !waiting.contains(&need) {
                        stack.push(need);
                        waiting.insert(need);
                        continue;
                    }
                    while let Some(member) = stack.pop() {
                        waiting.remove(&member);
                        self.mark_cycle(member);
                        if member == need {
                            break;
                        }
                    }
                }
            }
        }
    }

    fn iterate_group(&mut self, nodes: &[u32], limits: (f64, f64)) {
        for id in nodes {
            self.pending.remove(id);
        }
        self.iterating = true;
        let mut pass = 0.0;
        while pass < limits.0 {
            let mut change = 0.0f64;
            for id in nodes {
                let before = self.cell_scalar_of(*id);
                let _ = self.compute(*id);
                let after = self.cell_scalar_of(*id);
                change = change.max(value_change(&before, &after));
            }
            if change <= limits.1 {
                break;
            }
            pass += 1.0;
        }
        self.iterating = false;
    }

    fn mark_cycle(&mut self, id: u32) {
        self.pending.remove(&id);
        self.cycles.insert(id);
        if !self.nodes[id as usize].keep_cached {
            self.store_result(id, Stored::Scalar(Scalar::Number(0.0)));
        }
    }

    fn compute(&mut self, id: u32) -> Result<(), Halt> {
        self.pending.remove(&id);
        let formula = self.nodes[id as usize].formula.clone();
        let keep = self.nodes[id as usize].keep_cached;
        let sheet = self.nodes[id as usize].sheet;
        let row = self.nodes[id as usize].row;
        let col = self.nodes[id as usize].col;
        let legacy = self.nodes[id as usize].legacy;
        let ast = self.nodes[id as usize].ast.clone();
        let parse_error = self.nodes[id as usize].parse_error.clone();
        let cell_formula = self.cell_formula(sheet, row, col);
        if cell_formula.as_deref() != Some(formula.as_str()) || keep {
            return Ok(());
        }
        self.nodes[id as usize].computes += 1;
        self.in_progress.insert(id);
        let evaluated = if let Some(ast) = ast {
            let frame = Frame {
                sheet,
                row,
                col,
                depth: 0,
                root: Some(&ast),
                legacy,
                scope: empty_scope(),
            };
            match catch_deferred(|| self.eval(&ast, &frame)) {
                Ok(Ok(value)) => self.finalize(value, legacy, sheet, row, col),
                Ok(Err(halt)) | Err(halt) => Err(halt),
            }
        } else {
            Ok(Stored::Scalar(Scalar::error(
                parse_error.as_deref().unwrap_or("#NAME?"),
            )))
        };
        self.in_progress.remove(&id);
        let mut stored = match evaluated {
            Ok(stored) => stored,
            Err(Halt::Deferred(need)) => {
                self.pending.insert(id);
                return Err(Halt::Deferred(need));
            }
        };
        if self.dynamic_hits.contains(&id) {
            self.cycles.insert(id);
            stored = Stored::Scalar(Scalar::Number(0.0));
        }
        self.evaluations += 1;
        self.evaluated_at.insert(id, self.evaluations);
        self.store_result(id, stored);
        Ok(())
    }

    fn compute_nested(&mut self, id: u32) -> Result<(), Halt> {
        if self.compute_depth >= MAX_NESTED_COMPUTE {
            return Err(Halt::Deferred(id));
        }
        self.compute_depth += 1;
        let result = self.compute(id);
        self.compute_depth -= 1;
        result
    }

    fn finalize(
        &mut self,
        value: Value,
        legacy: bool,
        sheet: u32,
        row: u32,
        col: u32,
    ) -> Result<Stored, Halt> {
        if legacy {
            let scalar = self.implicit_intersection(value, sheet, row, col)?;
            return Ok(Stored::Scalar(clean(scalar)));
        }
        let stored = match value {
            Value::Lambda(_) => return Ok(Stored::Scalar(Scalar::error("#CALC!"))),
            Value::Ref(areas) => {
                if areas.len() != 1 {
                    Stored::Scalar(Scalar::error("#VALUE!"))
                } else {
                    let area = &areas[0];
                    if is_cell(area) {
                        Stored::Scalar(self.read_cell(
                            area.sheet,
                            area.range.start.row,
                            area.range.start.col,
                        )?)
                    } else {
                        match self.area_matrix(area)? {
                            Ok(matrix) => Stored::Matrix(matrix),
                            Err(code) => Stored::Scalar(Scalar::error(&code)),
                        }
                    }
                }
            }
            Value::Matrix(matrix) => Stored::Matrix(matrix),
            Value::Scalar(scalar) => Stored::Scalar(scalar),
        };
        Ok(match stored {
            Stored::Matrix(matrix) if matrix.rows() == 1 && matrix.cols() == 1 => {
                Stored::Scalar(clean(matrix.get(0, 0)))
            }
            Stored::Matrix(matrix) => {
                Stored::Matrix(matrix.block().map_values(|value| clean(value.clone())))
            }
            Stored::Scalar(scalar) => Stored::Scalar(clean(scalar)),
        })
    }

    fn store_result(&mut self, id: u32, result: Stored) {
        let sheet = self.nodes[id as usize].sheet;
        let row = self.nodes[id as usize].row;
        let col = self.nodes[id as usize].col;
        let array_range = self.nodes[id as usize].array_range.clone();
        let before = self.nodes[id as usize].spill.clone();
        let blocked_before = self.nodes[id as usize].blocked_spill.clone();
        match result {
            Stored::Scalar(scalar) => {
                if let Some(range) = array_range {
                    self.fill_array(sheet, &range, &Matrix::plain(vec![vec![scalar]]));
                    return;
                }
                if let Some(spill) = before.clone() {
                    self.clear_footprint(sheet, &spill, row, col);
                }
                self.nodes[id as usize].spill = None;
                self.nodes[id as usize].blocked_spill = None;
                self.set_cell(sheet, row, col, scalar);
            }
            Stored::Matrix(matrix) => {
                if let Some(range) = array_range {
                    self.fill_array(sheet, &range, &matrix);
                    return;
                }
                let footprint = footprint_for(row, col, &matrix);
                let free = footprint
                    .as_ref()
                    .is_some_and(|fp| self.footprint_free(id, sheet, fp, row, col));
                match footprint {
                    Some(footprint) if free => {
                        if let Some(spill) = before.clone() {
                            if !same_range(&Some(spill.clone()), &Some(footprint.clone())) {
                                self.clear_footprint(sheet, &spill, row, col);
                            }
                        }
                        self.nodes[id as usize].blocked_spill = None;
                        self.write_footprint(sheet, &footprint, &matrix);
                        self.nodes[id as usize].spill = Some(footprint.clone());
                        self.note_spill(id, sheet, &before, &footprint);
                    }
                    other => {
                        if let Some(spill) = before.clone() {
                            self.clear_footprint(sheet, &spill, row, col);
                        }
                        self.nodes[id as usize].spill = None;
                        self.nodes[id as usize].blocked_spill = other.or_else(|| {
                            Some(CellRange {
                                start: CellAddress { row, col },
                                end: CellAddress {
                                    row: row
                                        .saturating_add(matrix.rows().saturating_sub(1))
                                        .min(MAX_ROW),
                                    col: col
                                        .saturating_add(matrix.cols().saturating_sub(1))
                                        .min(MAX_COL),
                                },
                            })
                        });
                        self.set_cell(sheet, row, col, Scalar::error("#SPILL!"));
                    }
                }
            }
        }
        let spill_now = self.nodes[id as usize].spill.clone();
        let blocked_now = self.nodes[id as usize].blocked_spill.clone();
        if !same_range(&before, &spill_now) || !same_range(&blocked_before, &blocked_now) {
            self.ranked = false;
        }
    }

    fn note_spill(&mut self, id: u32, sheet: u32, before: &Option<CellRange>, spill: &CellRange) {
        if same_range(before, &Some(spill.clone())) {
            return;
        }
        let ranges: Vec<CellRange> = [before.clone(), Some(spill.clone())]
            .into_iter()
            .flatten()
            .collect();
        if ranges.is_empty() {
            return;
        }
        self.footprint_changes.push(FpChange {
            sheet,
            range: bounding_box(&ranges),
            at: self.evaluations,
        });
        if let Some(Some(bounds)) = self.bounds_cache.get_mut(sheet as usize) {
            bounds.0 = bounds.0.max(spill.end.row.saturating_add(1));
            bounds.1 = bounds.1.max(spill.end.col.saturating_add(1));
        }
        let _ = id;
    }

    fn fill_array(&mut self, sheet: u32, range: &CellRange, matrix: &Matrix) {
        for row in range.start.row..=range.end.row {
            for col in range.start.col..=range.end.col {
                let value = clean(pick(matrix, row - range.start.row, col - range.start.col));
                let stored = match value {
                    Scalar::Empty => Scalar::Number(0.0),
                    other => other,
                };
                self.set_cell(sheet, row, col, stored);
            }
        }
    }

    fn rebuild_reverse(&mut self) {
        let mut reverse = ReverseIndex::new();
        for (id, node) in self.nodes.iter().enumerate() {
            if node.alive {
                reverse.add_node(id as u32, &node.deps, &node.dynamic_deps);
            }
        }
        self.reverse = reverse;
    }

    fn release_spills(&mut self) {
        for sheet in &mut self.sheets {
            let keys: Vec<u64> = sheet
                .cells
                .iter()
                .filter(|(_, cell)| cell.spill_anchor.is_some() && cell.formula.is_none())
                .map(|(key, _)| *key)
                .collect();
            for key in keys {
                if let Some(cell) = sheet.cells.get_mut(&key) {
                    cell.value = Scalar::Empty;
                    cell.spill_anchor = None;
                }
            }
        }
    }

    fn cell_formula(&self, sheet: u32, row: u32, col: u32) -> Option<String> {
        self.sheets
            .get(sheet as usize)?
            .cells
            .get(&pos_key(row, col))?
            .formula
            .clone()
    }

    fn cell_legacy(&self, sheet: u32, row: u32, col: u32) -> bool {
        self.sheets
            .get(sheet as usize)
            .and_then(|grid| grid.cells.get(&pos_key(row, col)))
            .is_some_and(|cell| cell.legacy)
    }

    fn cell_array_range(&self, sheet: u32, row: u32, col: u32) -> Option<CellRange> {
        self.sheets
            .get(sheet as usize)
            .and_then(|grid| grid.cells.get(&pos_key(row, col)))
            .and_then(|cell| cell.array_range.clone())
    }

    fn cell_scalar(&self, sheet: u32, row: u32, col: u32) -> Scalar {
        self.sheets
            .get(sheet as usize)
            .and_then(|grid| grid.cells.get(&pos_key(row, col)))
            .map(|cell| cell.value.clone())
            .unwrap_or(Scalar::Empty)
    }

    fn cell_scalar_of(&self, id: u32) -> Scalar {
        let node = &self.nodes[id as usize];
        self.cell_scalar(node.sheet, node.row, node.col)
    }

    fn set_cell(&mut self, sheet: u32, row: u32, col: u32, value: Scalar) {
        let Some(grid) = self.sheets.get_mut(sheet as usize) else {
            return;
        };
        let key = pos_key(row, col);
        if let Some(cell) = grid.cells.get_mut(&key) {
            cell.value = value;
        } else {
            grid.insert(
                row,
                col,
                HostCell {
                    value,
                    formula: None,
                    spill_anchor: None,
                    legacy: false,
                    array_range: None,
                },
            );
        }
    }

    fn parse_cached(&mut self, formula: &str) -> Result<FormulaAst, FormulaError> {
        if let Some(cached) = self.parsed.get(formula) {
            return cached.clone();
        }
        let parsed = parse_formula(formula);
        self.parsed.insert(formula.to_owned(), parsed.clone());
        parsed
    }

    fn lookup_defined(&self, name: &str, sheet: u32, explicit: bool) -> Option<String> {
        let mut global = None;
        for defined in &self.defined {
            if !eq_name(&defined.name, name) {
                continue;
            }
            if defined.local_sheet == Some(sheet) {
                return Some(defined.formula.clone());
            }
            if defined.local_sheet.is_none() {
                global = Some(defined.formula.clone());
            }
        }
        if explicit && global.is_none() {
            None
        } else {
            global
        }
    }

    fn read_cell(&mut self, sheet: u32, row: u32, col: u32) -> Result<Scalar, Halt> {
        let anchor = self
            .sheets
            .get(sheet as usize)
            .and_then(|grid| grid.cells.get(&pos_key(row, col)))
            .and_then(|cell| cell.spill_anchor);
        if self.eval_active {
            if let Some(anchor) = anchor {
                if let Some(id) = self.by_cell.get(&(sheet, anchor.row, anchor.col)).copied() {
                    if self.pending.contains(&id) {
                        self.compute_nested(id)?;
                    }
                }
            }
            if let Some(id) = self.by_cell.get(&(sheet, row, col)).copied() {
                if self.nodes[id as usize].alive {
                    if self.in_progress.contains(&id) {
                        if !self.iterating {
                            self.dynamic_hits.insert(id);
                            self.dynamic_hit = true;
                            return Ok(Scalar::Number(0.0));
                        }
                    } else if self.pending.contains(&id) {
                        self.compute_nested(id)?;
                    }
                }
            }
        }
        if self.dynamic_hit {
            self.dynamic_hit = false;
            return Ok(Scalar::Number(0.0));
        }
        Ok(self.cell_scalar(sheet, row, col))
    }

    fn bounds_of(&mut self, sheet: u32) -> (u32, u32) {
        if let Some(Some(bounds)) = self.bounds_cache.get(sheet as usize) {
            return *bounds;
        }
        let mut rows = 0u32;
        let mut cols = 0u32;
        if let Some(grid) = self.sheets.get(sheet as usize) {
            for (row, row_cols) in &grid.rows {
                if row_cols.is_empty() {
                    continue;
                }
                rows = rows.max(row.saturating_add(1));
                if let Some(col) = row_cols.iter().next_back() {
                    cols = cols.max(col.saturating_add(1));
                }
            }
        }
        if let Some(slot) = self.bounds_cache.get_mut(sheet as usize) {
            *slot = Some((rows, cols));
        }
        (rows, cols)
    }

    fn area_matrix(&mut self, area: &Area) -> Result<Result<Matrix, String>, Halt> {
        let start = area.range.start;
        let mut end_row = area.range.end.row;
        let mut end_col = area.range.end.col;
        let full_rows = end_row as u64 - start.row as u64 + 1;
        let full_cols = end_col as u64 - start.col as u64 + 1;
        if full_rows * full_cols > CLIP_THRESHOLD {
            let (brows, bcols) = self.bounds_of(area.sheet);
            let row_cap = if brows == 0 {
                start.row
            } else {
                end_row.min(brows - 1).max(start.row)
            };
            let col_cap = if bcols == 0 {
                start.col
            } else {
                end_col.min(bcols - 1).max(start.col)
            };
            end_row = row_cap;
            end_col = col_cap;
        }
        let rows = end_row - start.row + 1;
        let cols = end_col - start.col + 1;
        if rows > MAX_ARRAY_SIDE || cols > MAX_ARRAY_SIDE {
            return Ok(Err("#VALUE!".to_owned()));
        }
        if u64::from(rows) * u64::from(cols) > MAX_MATRIX_CELLS {
            return Ok(Err("#NUM!".to_owned()));
        }
        let mut dense = Vec::with_capacity(rows as usize);
        for row in 0..rows {
            let mut line = Vec::with_capacity(cols as usize);
            for col in 0..cols {
                line.push(self.read_cell(area.sheet, start.row + row, start.col + col)?);
            }
            dense.push(line);
        }
        let block = Matrix::plain(dense);
        if full_rows * full_cols > MAX_ARRAY_CELLS {
            return Ok(Ok(block));
        }
        Ok(Ok(block.padded(
            full_rows as u32,
            full_cols as u32,
            Scalar::Empty,
        )))
    }

    fn implicit_intersection(
        &mut self,
        value: Value,
        _sheet: u32,
        row: u32,
        col: u32,
    ) -> Result<Scalar, Halt> {
        match value {
            Value::Matrix(matrix) => Ok(matrix.get(0, 0)),
            Value::Scalar(scalar) => Ok(scalar),
            Value::Lambda(_) => Ok(Scalar::error("#CALC!")),
            Value::Ref(areas) => {
                if areas.len() != 1 {
                    return Ok(Scalar::error("#VALUE!"));
                }
                let area = &areas[0];
                let start = area.range.start;
                let end = area.range.end;
                if start.row == end.row && start.col == end.col {
                    return self.read_cell(area.sheet, start.row, start.col);
                }
                let row_inside = row >= start.row && row <= end.row;
                let col_inside = col >= start.col && col <= end.col;
                if start.col == end.col && row_inside {
                    return self.read_cell(area.sheet, row, start.col);
                }
                if start.row == end.row && col_inside {
                    return self.read_cell(area.sheet, start.row, col);
                }
                if row_inside && col_inside {
                    return self.read_cell(area.sheet, row, col);
                }
                Ok(Scalar::error("#VALUE!"))
            }
        }
    }

    fn clear_footprint(&mut self, sheet: u32, footprint: &CellRange, row: u32, col: u32) {
        for r in footprint.start.row..=footprint.end.row {
            for c in footprint.start.col..=footprint.end.col {
                if r == row && c == col {
                    continue;
                }
                let Some(grid) = self.sheets.get_mut(sheet as usize) else {
                    return;
                };
                let key = pos_key(r, c);
                let Some(cell) = grid.cells.get_mut(&key) else {
                    continue;
                };
                if cell.spill_anchor == Some(CellAddress { row, col }) {
                    cell.spill_anchor = None;
                    cell.value = Scalar::Empty;
                }
            }
        }
    }

    fn write_footprint(&mut self, sheet: u32, footprint: &CellRange, matrix: &Matrix) {
        let origin_row = footprint.start.row;
        let origin_col = footprint.start.col;
        for row in footprint.start.row..=footprint.end.row {
            for col in footprint.start.col..=footprint.end.col {
                let mut value = clean(matrix.get(row - origin_row, col - origin_col));
                if matches!(value, Scalar::Empty) {
                    value = Scalar::Number(0.0);
                }
                if row == origin_row && col == origin_col {
                    self.set_cell(sheet, row, col, value);
                    continue;
                }
                self.set_cell(sheet, row, col, value);
                if let Some(grid) = self.sheets.get_mut(sheet as usize) {
                    if let Some(cell) = grid.cells.get_mut(&pos_key(row, col)) {
                        cell.spill_anchor = Some(CellAddress {
                            row: origin_row,
                            col: origin_col,
                        });
                    }
                }
            }
        }
    }

    fn footprint_free(
        &self,
        id: u32,
        sheet: u32,
        footprint: &CellRange,
        row: u32,
        col: u32,
    ) -> bool {
        let Some(grid) = self.sheets.get(sheet as usize) else {
            return false;
        };
        if grid
            .merges
            .iter()
            .any(|merge| ranges_intersect(merge, footprint))
        {
            return false;
        }
        for r in footprint.start.row..=footprint.end.row {
            for c in footprint.start.col..=footprint.end.col {
                if r == row && c == col {
                    continue;
                }
                let Some(cell) = grid.cells.get(&pos_key(r, c)) else {
                    continue;
                };
                if cell.formula.is_some() {
                    return false;
                }
                if let Some(anchor) = cell.spill_anchor {
                    if anchor.row == row && anchor.col == col {
                        continue;
                    }
                    if self.other_claims(id, sheet, anchor, r, c) {
                        return false;
                    }
                    continue;
                }
                if !cell.value.is_empty() {
                    return false;
                }
            }
        }
        true
    }

    fn other_claims(&self, id: u32, sheet: u32, anchor: CellAddress, row: u32, col: u32) -> bool {
        let Some(other) = self.by_cell.get(&(sheet, anchor.row, anchor.col)).copied() else {
            return false;
        };
        if other == id {
            return false;
        }
        self.nodes[other as usize]
            .spill
            .as_ref()
            .is_some_and(|spill| range_has(spill, row, col))
    }

    fn eval<'a>(&mut self, node: &'a FormulaAst, frame: &Frame<'a>) -> Result<Value, Halt> {
        match node {
            FormulaAst::Number { value } => Ok(Value::number(*value)),
            FormulaAst::String { value } => Ok(Value::Scalar(Scalar::Text(value.clone()))),
            FormulaAst::Boolean { value } => Ok(Value::Scalar(Scalar::Bool(*value))),
            FormulaAst::Error { code } => Ok(Value::err(code)),
            FormulaAst::Missing => Ok(Value::Scalar(Scalar::Empty)),
            FormulaAst::Array { rows } => {
                let dense = rows
                    .iter()
                    .map(|row| row.iter().map(scalar_from_constant).collect())
                    .collect();
                Ok(Value::Matrix(Matrix::plain(dense)))
            }
            FormulaAst::Ref {
                prefix,
                reference,
                spill,
            } => self.eval_ref(prefix, reference, *spill, frame),
            FormulaAst::Name { prefix, name } => self.eval_name(name, prefix, frame),
            FormulaAst::Structured { reference, .. } => {
                Ok(self.resolve_structured(reference, frame.sheet, frame.row, frame.col))
            }
            FormulaAst::Unary { op, operand } => {
                if *op == UnaryOp::Plus {
                    return self.eval(operand, frame);
                }
                let value = self.eval(operand, frame)?;
                if *op == UnaryOp::At {
                    return Ok(Value::Scalar(self.implicit_intersection(
                        value,
                        frame.sheet,
                        frame.row,
                        frame.col,
                    )?));
                }
                let operand = self.operand(value, frame)?;
                let date1904 = self.date1904;
                Ok(broadcast1(operand, move |value| {
                    scalar_unary('-', value, date1904)
                }))
            }
            FormulaAst::Percent { operand } => {
                let evaluated = self.eval(operand, frame)?;
                let value = self.operand(evaluated, frame)?;
                let date1904 = self.date1904;
                Ok(broadcast1(value, move |value| {
                    scalar_unary('%', value, date1904)
                }))
            }
            FormulaAst::Binary { .. } => self.eval_binary(node, frame),
            FormulaAst::Call {
                name,
                raw_name,
                args,
            } => self.eval_call(name, raw_name, args, frame),
            FormulaAst::Invoke { callee, args } => self.eval_invoke(callee, args, frame),
        }
    }

    fn eval_ref(
        &mut self,
        prefix: &Option<SheetPrefix>,
        reference: &Option<crate::formula::ast::RefSpec>,
        spill: bool,
        frame: &Frame<'_>,
    ) -> Result<Value, Halt> {
        let Some(reference) = reference else {
            return Ok(Value::err("#REF!"));
        };
        let Some(sheets) = prefix_sheets(&self.sheet_names, prefix, frame.sheet) else {
            return Ok(Value::err("#REF!"));
        };
        let range = spec_range(reference);
        if spill {
            let sheet = sheets.first().copied().unwrap_or(frame.sheet);
            let spilled = self.live_spill(sheet, range.start.row, range.start.col)?;
            return Ok(match spilled {
                Some(range) => Value::Ref(vec![Area { sheet, range }]),
                None => Value::err("#REF!"),
            });
        }
        Ok(Value::Ref(
            sheets
                .into_iter()
                .map(|sheet| Area {
                    sheet,
                    range: range.clone(),
                })
                .collect(),
        ))
    }

    fn live_spill(&mut self, sheet: u32, row: u32, col: u32) -> Result<Option<CellRange>, Halt> {
        let Some(id) = self.by_cell.get(&(sheet, row, col)).copied() else {
            return Ok(None);
        };
        if self.eval_active && self.pending.contains(&id) {
            self.compute_nested(id)?;
        }
        Ok(self.nodes[id as usize].spill.clone())
    }

    fn eval_name(
        &mut self,
        name: &str,
        prefix: &Option<SheetPrefix>,
        frame: &Frame<'_>,
    ) -> Result<Value, Halt> {
        if prefix.is_none() {
            let key = normalize_local_name(name);
            if let Some(bound) = frame.scope.get(&key) {
                return Ok(from_fns(bound.clone()));
            }
        }
        let mut sheet = frame.sheet;
        if let Some(prefix) = prefix {
            if prefix.book.is_some() {
                return Ok(Value::err("#REF!"));
            }
            let Some(index) = sheet_index(&self.sheet_names, &prefix.sheet) else {
                return Ok(Value::err("#REF!"));
            };
            sheet = index;
        }
        if let Some(formula) = self.lookup_defined(name, sheet, prefix.is_some()) {
            if frame.depth >= MAX_NAME_DEPTH {
                return Ok(Value::err("#NAME?"));
            }
            let ast = match self.parse_cached(&formula) {
                Ok(ast) => ast,
                Err(_) => return Ok(Value::err("#NAME?")),
            };
            return self.eval(
                &ast,
                &Frame {
                    sheet: frame.sheet,
                    row: frame.row,
                    col: frame.col,
                    depth: frame.depth + 1,
                    root: None,
                    legacy: frame.legacy,
                    scope: empty_scope(),
                },
            );
        }
        if prefix.is_none() {
            let structured = self.resolve_structured(
                &StructuredRef {
                    table: Some(name.to_owned()),
                    specials: Vec::new(),
                    column: None,
                    column2: None,
                },
                frame.sheet,
                frame.row,
                frame.col,
            );
            if !is_name_error(&structured) {
                return Ok(structured);
            }
        }
        Ok(Value::err("#NAME?"))
    }

    fn eval_binary<'a>(&mut self, node: &'a FormulaAst, frame: &Frame<'a>) -> Result<Value, Halt> {
        let mut spine = Vec::new();
        let mut leftmost = node;
        while let FormulaAst::Binary { left, .. } = leftmost {
            spine.push(leftmost);
            leftmost = left.as_ref();
        }
        let mut value = self.eval(leftmost, frame)?;
        for op_node in spine.into_iter().rev() {
            value = self.apply_binary(op_node, value, frame)?;
        }
        Ok(value)
    }

    fn apply_binary<'a>(
        &mut self,
        node: &'a FormulaAst,
        left: Value,
        frame: &Frame<'a>,
    ) -> Result<Value, Halt> {
        let FormulaAst::Binary { op, right, .. } = node else {
            return Ok(left);
        };
        if matches!(op, BinaryOp::Range | BinaryOp::Intersect | BinaryOp::Union) {
            if is_error_value(&left) {
                return Ok(left);
            }
            let right = self.eval(right, frame)?;
            if is_error_value(&right) {
                return Ok(right);
            }
            return Ok(match op {
                BinaryOp::Range => range_operator(left, right),
                BinaryOp::Intersect => intersect_operator(left, right),
                _ => union_operator(left, right),
            });
        }
        let right = self.eval(right, frame)?;
        let snap = frame.root.is_some_and(|root| std::ptr::eq(root, node))
            && matches!(op, BinaryOp::Add | BinaryOp::Sub);
        let left = self.operand(left, frame)?;
        let right = self.operand(right, frame)?;
        let date1904 = self.date1904;
        Ok(broadcast2(left, right, move |a, b| {
            scalar_binary(*op, a, b, snap, date1904)
        }))
    }

    fn operand(&mut self, value: Value, frame: &Frame<'_>) -> Result<Value, Halt> {
        if matches!(value, Value::Lambda(_)) {
            return Ok(Value::err("#CALC!"));
        }
        let Value::Ref(areas) = value else {
            return Ok(value);
        };
        let single = areas.len() == 1 && areas.first().is_some_and(is_cell);
        if frame.legacy && !single {
            return Ok(Value::Scalar(self.implicit_intersection(
                Value::Ref(areas),
                frame.sheet,
                frame.row,
                frame.col,
            )?));
        }
        if single {
            let area = &areas[0];
            return Ok(Value::Scalar(self.read_cell(
                area.sheet,
                area.range.start.row,
                area.range.start.col,
            )?));
        }
        if areas.len() != 1 {
            return Ok(Value::err("#VALUE!"));
        }
        match self.area_matrix(&areas[0])? {
            Ok(matrix) => Ok(Value::Matrix(matrix)),
            Err(code) => Ok(Value::err(&code)),
        }
    }

    fn resolve_structured(
        &self,
        reference: &StructuredRef,
        sheet: u32,
        row: u32,
        col: u32,
    ) -> Value {
        let Some((table_sheet, table)) =
            self.find_table(reference.table.as_deref(), sheet, row, col)
        else {
            return if reference.table.is_some() {
                Value::err("#NAME?")
            } else {
                Value::err("#REF!")
            };
        };
        let range = norm_range(table.range.clone());
        let data_start = range.start.row as i64 + i64::from(table.header);
        let data_end = range.end.row as i64 - i64::from(table.totals);
        let header_row = if table.header {
            range.start.row as i64
        } else {
            -1
        };
        let totals_row = if table.totals {
            range.end.row as i64
        } else {
            -1
        };
        let mut c1 = range.start.col;
        let mut c2 = range.end.col;
        if let Some(column) = &reference.column {
            let Some(found) = column_index(table, column) else {
                return Value::err("#REF!");
            };
            c1 = found;
            c2 = if let Some(column2) = &reference.column2 {
                let Some(found) = column_index(table, column2) else {
                    return Value::err("#REF!");
                };
                found
            } else {
                c1
            };
        }
        let mut r1 = i64::MAX;
        let mut r2 = i64::MIN;
        let specials: Vec<StructuredSpecial> = if reference.specials.is_empty() {
            vec![StructuredSpecial::Data]
        } else {
            reference.specials.clone()
        };
        for special in specials {
            match special {
                StructuredSpecial::All => {
                    r1 = r1.min(range.start.row as i64);
                    r2 = r2.max(range.end.row as i64);
                }
                StructuredSpecial::Data => {
                    r1 = r1.min(data_start);
                    r2 = r2.max(data_end);
                }
                StructuredSpecial::Headers => {
                    if header_row < 0 {
                        return Value::err("#REF!");
                    }
                    r1 = r1.min(header_row);
                    r2 = r2.max(header_row);
                }
                StructuredSpecial::Totals => {
                    if totals_row < 0 {
                        return Value::err("#REF!");
                    }
                    r1 = r1.min(totals_row);
                    r2 = r2.max(totals_row);
                }
                StructuredSpecial::ThisRow => {
                    if table_sheet != sheet || (row as i64) < data_start || (row as i64) > data_end
                    {
                        return Value::err("#VALUE!");
                    }
                    r1 = r1.min(row as i64);
                    r2 = r2.max(row as i64);
                }
            }
        }
        if r1 > r2 || r1 < 0 {
            return Value::err("#REF!");
        }
        Value::Ref(vec![Area {
            sheet: table_sheet,
            range: norm_range(CellRange {
                start: CellAddress {
                    row: r1 as u32,
                    col: c1,
                },
                end: CellAddress {
                    row: r2 as u32,
                    col: c2,
                },
            }),
        }])
    }

    fn find_table(
        &self,
        name: Option<&str>,
        sheet: u32,
        row: u32,
        col: u32,
    ) -> Option<(u32, &TableMeta)> {
        if let Some(name) = name {
            for (index, grid) in self.sheets.iter().enumerate() {
                for table in &grid.tables {
                    if eq_name(&table.display, name) || eq_name(&table.name, name) {
                        return Some((index as u32, table));
                    }
                }
            }
            return None;
        }
        let grid = self.sheets.get(sheet as usize)?;
        grid.tables
            .iter()
            .find(|table| range_has(&table.range, row, col))
            .map(|table| (sheet, table))
    }

    fn eval_to_fns(&mut self, node: &FormulaAst, frame: &Frame<'_>) -> Result<FnValue, Halt> {
        Ok(to_fns(self.eval(node, frame)?))
    }

    fn eval_invoke<'a>(
        &mut self,
        callee: &'a FormulaAst,
        args: &'a [FormulaAst],
        frame: &Frame<'a>,
    ) -> Result<Value, Halt> {
        let callee_value = self.eval(callee, frame)?;
        if is_error_value(&callee_value) {
            return Ok(callee_value);
        }
        let Value::Lambda(lambda) = callee_value else {
            return Ok(Value::err("#VALUE!"));
        };
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.eval_to_fns(arg, frame)?);
        }
        Ok(from_fns(self.call_lambda(&lambda, &values, frame)?))
    }

    fn eval_call<'a>(
        &mut self,
        name: &str,
        raw_name: &str,
        args: &'a [FormulaAst],
        frame: &Frame<'a>,
    ) -> Result<Value, Halt> {
        let key = normalize_local_name(raw_name);
        if let Some(FnValue::Lambda(lambda)) = frame.scope.get(&key).cloned() {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(self.eval_to_fns(arg, frame)?);
            }
            return Ok(from_fns(self.call_lambda(&lambda, &values, frame)?));
        }
        let Some(spec) = get_function(name) else {
            return self.call_defined_name(raw_name, args, frame);
        };
        if args.len() < spec.min_args || args.len() > spec.max_args {
            return Ok(Value::err("#VALUE!"));
        }
        let ptr = self as *mut CalcEngine;
        let host = EngineHost { engine: ptr };
        let sheet = frame.sheet;
        let row = frame.row;
        let col = frame.col;
        let legacy = frame.legacy;
        let depth = frame.depth;
        let root = frame.root;
        let scope = frame.scope;
        let date1904 = self.date1904;
        let eval = move |node: &FormulaAst| -> FnValue {
            // SAFETY: `ptr` is this calc engine, pinned for the call. The hook runs
            // only from a function body and does not overlap another `&mut` of it.
            let engine = unsafe { &mut *ptr };
            let local = Frame {
                sheet,
                row,
                col,
                depth,
                root: None,
                legacy,
                scope,
            };
            match engine.eval(node, &local) {
                Ok(value) => to_fns(value),
                Err(Halt::Deferred(id)) => raise_deferred(id),
            }
        };
        let eval_in = move |node: &FormulaAst, inner: &HashMap<String, FnValue>| -> FnValue {
            let engine = unsafe { &mut *ptr };
            let local = Frame {
                sheet,
                row,
                col,
                depth,
                root: None,
                legacy,
                scope: inner,
            };
            match engine.eval(node, &local) {
                Ok(value) => to_fns(value),
                Err(Halt::Deferred(id)) => raise_deferred(id),
            }
        };
        let call_lambda_fn = move |lambda: &LambdaValue, values: &[FnValue]| -> FnValue {
            let engine = unsafe { &mut *ptr };
            let local = Frame {
                sheet,
                row,
                col,
                depth,
                root: None,
                legacy,
                scope,
            };
            match engine.call_lambda(lambda, values, &local) {
                Ok(value) => value,
                Err(Halt::Deferred(id)) => raise_deferred(id),
            }
        };
        let ctx = CallCtx {
            host: &host,
            sheet: sheet as usize,
            row: row as i32,
            col: col as i32,
            legacy,
            date1904,
            depth,
            scope,
            eval: Some(&eval),
            eval_in: Some(&eval_in),
            call_lambda: Some(&call_lambda_fn),
        };
        let result = match spec.kind {
            FuncKind::Lazy { f } => f(&ctx, args),
            _ => {
                let mut values = Vec::with_capacity(args.len());
                for (index, arg) in args.iter().enumerate() {
                    let use_legacy = legacy && param_kind(spec, index) != ParamKind::Any;
                    let local = Frame {
                        sheet,
                        row,
                        col,
                        depth,
                        root,
                        legacy: use_legacy,
                        scope,
                    };
                    let engine = unsafe { &mut *ptr };
                    values.push(match engine.eval(arg, &local) {
                        Ok(value) => to_fns(value),
                        Err(Halt::Deferred(id)) => raise_deferred(id),
                    });
                }
                Ok(call_function(name, &values, &ctx))
            }
        };
        match result {
            Ok(value) => Ok(from_fns(value)),
            Err(code) => Ok(Value::err(code.as_excel())),
        }
    }

    fn call_defined_name(
        &mut self,
        raw_name: &str,
        args: &[FormulaAst],
        frame: &Frame<'_>,
    ) -> Result<Value, Halt> {
        let Some(formula) = self.lookup_defined(raw_name, frame.sheet, false) else {
            return Ok(Value::err("#NAME?"));
        };
        if frame.depth >= MAX_NAME_DEPTH {
            return Ok(Value::err("#NAME?"));
        }
        let ast = match self.parse_cached(&formula) {
            Ok(ast) => ast,
            Err(_) => return Ok(Value::err("#NAME?")),
        };
        let named = self.eval(
            &ast,
            &Frame {
                sheet: frame.sheet,
                row: frame.row,
                col: frame.col,
                depth: frame.depth + 1,
                root: None,
                legacy: frame.legacy,
                scope: empty_scope(),
            },
        )?;
        let Value::Lambda(lambda) = named else {
            return Ok(Value::err("#NAME?"));
        };
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.eval_to_fns(arg, frame)?);
        }
        Ok(from_fns(self.call_lambda(&lambda, &values, frame)?))
    }

    fn call_lambda(
        &mut self,
        lambda: &LambdaValue,
        args: &[FnValue],
        frame: &Frame<'_>,
    ) -> Result<FnValue, Halt> {
        if args.len() > lambda.params.len() {
            return Ok(FnValue::error(ErrorCode::Value));
        }
        let mut scope = lambda.scope.clone();
        for (index, name) in lambda.params.iter().enumerate() {
            let value = args.get(index).cloned().unwrap_or_else(FnValue::blank);
            scope.insert(name.clone(), value);
        }
        self.eval_to_fns(
            &lambda.body,
            &Frame {
                sheet: frame.sheet,
                row: frame.row,
                col: frame.col,
                depth: frame.depth,
                root: None,
                legacy: frame.legacy,
                scope: &scope,
            },
        )
    }
}

struct DeferredHalt(u32);

fn raise_deferred(id: u32) -> ! {
    std::panic::panic_any(DeferredHalt(id));
}

fn catch_deferred<T>(body: impl FnOnce() -> T) -> Result<T, Halt> {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(value) => Ok(value),
        Err(payload) => match payload.downcast::<DeferredHalt>() {
            Ok(halt) => Err(Halt::Deferred(halt.0)),
            Err(payload) => resume_unwind(payload),
        },
    }
}

fn empty_scope() -> &'static HashMap<String, FnValue> {
    static EMPTY: OnceLock<HashMap<String, FnValue>> = OnceLock::new();
    EMPTY.get_or_init(HashMap::new)
}

/// `EvalHost` for a calc engine. Reads compute pending formulas. A nest that is too
/// deep panics with [`DeferredHalt`] so [`catch_deferred`] can retry it, the same
/// unwind `Deferred` uses upstream.
struct EngineHost {
    engine: *mut CalcEngine,
}

impl EvalHost for EngineHost {
    fn read_cell(&self, sheet: usize, row: i32, col: i32) -> FnScalar {
        if row < 0 || col < 0 {
            return FnScalar::Blank;
        }
        // SAFETY: the pointer is the engine evaluating this call. Methods do not hold
        // a Rust reference across the re-entrant read.
        let engine = unsafe { &mut *self.engine };
        match engine.read_cell(sheet as u32, row as u32, col as u32) {
            Ok(value) => scalar_to_fns(value),
            Err(Halt::Deferred(id)) => raise_deferred(id),
        }
    }

    fn for_each_stored(
        &self,
        sheet: usize,
        area: FnArea,
        visit: &mut dyn FnMut(FnScalar, i32, i32),
    ) {
        let start_row = u32::try_from(area.start_row).unwrap_or(0);
        let end_row = u32::try_from(area.end_row).unwrap_or(0);
        let start_col = u32::try_from(area.start_col).unwrap_or(0);
        let end_col = u32::try_from(area.end_col).unwrap_or(0);
        if area.end_row < 0 || area.end_col < 0 || start_row > end_row || start_col > end_col {
            return;
        }
        let engine = unsafe { &mut *self.engine };
        let coords = {
            let Some(grid) = engine.sheets.get(sheet) else {
                return;
            };
            let mut coords = Vec::new();
            for (row, cols) in grid.rows.range(start_row..=end_row) {
                for col in cols.range(start_col..=end_col) {
                    coords.push((*row, *col));
                }
            }
            coords
        };
        for (row, col) in coords {
            let value = match engine.read_cell(sheet as u32, row, col) {
                Ok(value) => value,
                Err(Halt::Deferred(id)) => raise_deferred(id),
            };
            if value.is_empty() {
                continue;
            }
            visit(scalar_to_fns(value), row as i32, col as i32);
        }
    }

    fn bounds(&self, sheet: usize) -> (i32, i32) {
        let engine = unsafe { &mut *self.engine };
        let (rows, cols) = engine.bounds_of(sheet as u32);
        (rows as i32, cols as i32)
    }

    fn cell_formula(&self, sheet: usize, row: i32, col: i32) -> Option<String> {
        if row < 0 || col < 0 {
            return None;
        }
        let engine = unsafe { &*self.engine };
        engine.cell_formula(sheet as u32, row as u32, col as u32)
    }

    fn sheet_count(&self) -> usize {
        let engine = unsafe { &*self.engine };
        engine.sheet_names.len()
    }

    fn sheet_name(&self, index: usize) -> Option<&str> {
        let text = unsafe { &*self.engine }
            .sheet_names
            .get(index)
            .map(|name| name.as_str() as *const str)?;
        // SAFETY: `text` points into this engine's sheet names, which outlive the host.
        Some(unsafe { &*text })
    }

    fn find_sheet(&self, name: &str) -> Option<usize> {
        let engine = unsafe { &*self.engine };
        if let Some(index) = engine.sheet_names.iter().position(|sheet| sheet == name) {
            return Some(index);
        }
        engine
            .sheet_names
            .iter()
            .position(|sheet| sheet.eq_ignore_ascii_case(name))
    }

    fn random(&self) -> f64 {
        const MOD: u64 = 2_147_483_647;
        let engine = unsafe { &*self.engine };
        let next = engine.rand_seed.get().saturating_mul(16807) % MOD;
        engine.rand_seed.set(next);
        (next.saturating_sub(1)) as f64 / (MOD - 1) as f64
    }

    fn now_serial(&self) -> f64 {
        let engine = unsafe { &*self.engine };
        crate::formula::fns::pinned_now_serial(engine.date1904)
    }

    fn row_hidden(&self, sheet: usize, row: i32) -> bool {
        if row < 0 {
            return false;
        }
        let engine = unsafe { &*self.engine };
        engine
            .sheets
            .get(sheet)
            .is_some_and(|grid| grid.hidden.contains(&(row as u32)))
    }

    fn occupied(&self, sheet: usize, row: i32, col: i32) -> bool {
        if row < 0 || col < 0 {
            return false;
        }
        let engine = unsafe { &*self.engine };
        let Some(cell) = engine
            .sheets
            .get(sheet)
            .and_then(|grid| grid.cells.get(&pos_key(row as u32, col as u32)))
        else {
            return false;
        };
        cell.formula.is_some() || cell.spill_anchor.is_some() || !cell.value.is_empty()
    }
}

fn scalar_to_fns(value: Scalar) -> FnScalar {
    match value {
        Scalar::Empty => FnScalar::Blank,
        Scalar::Number(n) => FnScalar::Number(n),
        Scalar::Text(text) => FnScalar::Text(text),
        Scalar::Bool(value) => FnScalar::Bool(value),
        Scalar::Error(code) => FnScalar::Error(ErrorCode::parse(&code).unwrap_or(ErrorCode::Value)),
    }
}

fn scalar_from_fns(value: FnScalar) -> Scalar {
    match value {
        FnScalar::Blank => Scalar::Empty,
        FnScalar::Number(n) => Scalar::Number(n),
        FnScalar::Text(text) => Scalar::Text(text),
        FnScalar::Bool(value) => Scalar::Bool(value),
        FnScalar::Error(code) => Scalar::error(code.as_excel()),
    }
}

fn area_to_fns(area: Area) -> FnArea {
    FnArea {
        sheet: area.sheet as usize,
        start_row: area.range.start.row as i32,
        start_col: area.range.start.col as i32,
        end_row: area.range.end.row as i32,
        end_col: area.range.end.col as i32,
    }
}

fn area_from_fns(area: FnArea) -> Area {
    Area {
        sheet: area.sheet as u32,
        range: norm_range(CellRange {
            start: CellAddress {
                row: area.start_row.max(0) as u32,
                col: area.start_col.max(0) as u32,
            },
            end: CellAddress {
                row: area.end_row.max(0) as u32,
                col: area.end_col.max(0) as u32,
            },
        }),
    }
}

fn matrix_to_fns(matrix: Matrix) -> FnMatrix {
    let Matrix {
        dense,
        pad_rows,
        pad_cols,
        fill,
    } = matrix;
    let stored_rows = dense.len() as u32;
    let stored_cols = dense.first().map(|line| line.len()).unwrap_or(0) as u32;
    let padded = pad_rows > stored_rows || pad_cols > stored_cols;
    let dense = dense
        .into_iter()
        .map(|line| line.into_iter().map(scalar_to_fns).collect())
        .collect();
    let block = FnMatrix::from_rows(dense);
    if padded {
        FnMatrix::padded(
            block,
            pad_rows as usize,
            pad_cols as usize,
            scalar_to_fns(fill),
        )
    } else {
        block
    }
}

fn matrix_from_fns(matrix: FnMatrix) -> Matrix {
    let stored_rows = matrix.block_rows();
    let stored_cols = matrix.block_cols();
    let mut dense = Vec::with_capacity(stored_rows);
    for row in 0..stored_rows {
        let mut line = Vec::with_capacity(stored_cols);
        for col in 0..stored_cols {
            line.push(scalar_from_fns(matrix.get(row, col)));
        }
        dense.push(line);
    }
    let mut out = Matrix::plain(dense);
    if matrix.is_padded() {
        let fill = if matrix.rows() > stored_rows {
            scalar_from_fns(matrix.get(stored_rows, 0))
        } else if matrix.cols() > stored_cols {
            scalar_from_fns(matrix.get(0, stored_cols))
        } else {
            Scalar::Empty
        };
        out = out.padded(matrix.rows() as u32, matrix.cols() as u32, fill);
    }
    out
}

fn to_fns(value: Value) -> FnValue {
    match value {
        Value::Scalar(scalar) => FnValue::Scalar(scalar_to_fns(scalar)),
        Value::Matrix(matrix) => FnValue::Matrix(matrix_to_fns(matrix)),
        Value::Ref(areas) => FnValue::Ref(RefValue {
            areas: areas.into_iter().map(area_to_fns).collect(),
        }),
        Value::Lambda(lambda) => FnValue::Lambda(lambda),
    }
}

fn from_fns(value: FnValue) -> Value {
    match value {
        FnValue::Scalar(scalar) => Value::Scalar(scalar_from_fns(scalar)),
        FnValue::Matrix(matrix) => Value::Matrix(matrix_from_fns(matrix)),
        FnValue::Ref(reference) => {
            Value::Ref(reference.areas.into_iter().map(area_from_fns).collect())
        }
        FnValue::Lambda(lambda) => Value::Lambda(lambda),
    }
}

impl DepHost for CalcEngine {
    fn sheet_names(&self) -> &[String] {
        &self.sheet_names
    }

    fn parse_dep(&mut self, formula: &str) -> Result<FormulaAst, FormulaError> {
        self.parse_cached(formula)
    }

    fn defined_formula(&self, name: &str, sheet: u32, explicit: bool) -> Option<String> {
        self.lookup_defined(name, sheet, explicit)
    }

    fn structured_areas(
        &self,
        reference: &StructuredRef,
        sheet: u32,
        row: u32,
        col: u32,
    ) -> Vec<Area> {
        match self.resolve_structured(reference, sheet, row, col) {
            Value::Ref(areas) => areas,
            _ => Vec::new(),
        }
    }
}

impl SheetGrid {
    fn insert(&mut self, row: u32, col: u32, cell: HostCell) {
        self.rows.entry(row).or_default().insert(col);
        self.cells.insert(pos_key(row, col), cell);
    }

    fn remove_at(&mut self, row: u32, col: u32) {
        self.cells.remove(&pos_key(row, col));
        if let Some(cols) = self.rows.get_mut(&row) {
            cols.remove(&col);
            if cols.is_empty() {
                self.rows.remove(&row);
            }
        }
    }
}

fn build_footprints(nodes: &[FormulaNode]) -> AreaIndex {
    let mut index = AreaIndex::new();
    for (id, node) in nodes.iter().enumerate() {
        let fp = node
            .spill
            .clone()
            .or(node.blocked_spill.clone())
            .or(node.array_range.clone());
        if !node.alive {
            continue;
        }
        if let Some(range) = fp {
            index.add(
                &Area {
                    sheet: node.sheet,
                    range,
                },
                id as u32,
            );
        }
    }
    index
}

fn pos_key(row: u32, col: u32) -> u64 {
    (row as u64) * 16_384 + col as u64
}

fn is_cell(area: &Area) -> bool {
    area.range.start.row == area.range.end.row && area.range.start.col == area.range.end.col
}

fn nz(n: f64) -> Scalar {
    if !n.is_finite() {
        Scalar::error("#NUM!")
    } else if n == 0.0 {
        Scalar::Number(0.0)
    } else {
        Scalar::Number(n)
    }
}

fn clean(value: Scalar) -> Scalar {
    match value {
        Scalar::Empty => Scalar::Number(0.0),
        Scalar::Number(n) => nz(n),
        other => other,
    }
}

fn value_change(before: &Scalar, after: &Scalar) -> f64 {
    match (before, after) {
        (Scalar::Number(a), Scalar::Number(b)) => (b - a).abs(),
        _ => {
            if scalar_eq(before, after) {
                0.0
            } else {
                f64::INFINITY
            }
        }
    }
}

fn scalar_eq(a: &Scalar, b: &Scalar) -> bool {
    match (a, b) {
        (Scalar::Empty, Scalar::Empty) => true,
        (Scalar::Number(x), Scalar::Number(y)) => x == y,
        (Scalar::Text(x), Scalar::Text(y)) => x == y,
        (Scalar::Bool(x), Scalar::Bool(y)) => x == y,
        (Scalar::Error(x), Scalar::Error(y)) => x == y,
        _ => false,
    }
}

fn scalar_from_dump(value: &DumpCellValue) -> Scalar {
    match value {
        DumpCellValue::Null => Scalar::Empty,
        DumpCellValue::Number(n) => Scalar::Number(*n),
        DumpCellValue::NumberRaw(raw) => match raw.parse::<f64>() {
            Ok(n) if n.is_finite() => Scalar::Number(n),
            _ => Scalar::Number(0.0),
        },
        DumpCellValue::String(text) => Scalar::Text(text.clone()),
        DumpCellValue::Bool(value) => Scalar::Bool(*value),
        DumpCellValue::Error(code) => Scalar::Error(code.clone()),
    }
}

fn scalar_to_dump(value: &Scalar) -> DumpCellValue {
    match value {
        Scalar::Empty => DumpCellValue::Null,
        Scalar::Number(n) => DumpCellValue::Number(*n),
        Scalar::Text(text) => DumpCellValue::String(text.clone()),
        Scalar::Bool(value) => DumpCellValue::Bool(*value),
        Scalar::Error(code) => DumpCellValue::Error(code.clone()),
    }
}

fn footprint_for(row: u32, col: u32, matrix: &Matrix) -> Option<CellRange> {
    let end_row = row.saturating_add(matrix.rows().saturating_sub(1));
    let end_col = col.saturating_add(matrix.cols().saturating_sub(1));
    if end_row > MAX_ROW || end_col > MAX_COL || matrix.rows() == 0 || matrix.cols() == 0 {
        return None;
    }
    Some(CellRange {
        start: CellAddress { row, col },
        end: CellAddress {
            row: end_row,
            col: end_col,
        },
    })
}

fn pick(matrix: &Matrix, row: u32, col: u32) -> Scalar {
    let r = if matrix.rows() == 1 { 0 } else { row };
    let c = if matrix.cols() == 1 { 0 } else { col };
    if r >= matrix.rows() || c >= matrix.cols() {
        return Scalar::error("#N/A");
    }
    matrix.get(r, c)
}

fn scalar_from_constant(value: &ArrayConstant) -> Scalar {
    match value {
        ArrayConstant::Number(n) => nz(*n),
        ArrayConstant::String(text) => Scalar::Text(text.clone()),
        ArrayConstant::Bool(value) => Scalar::Bool(*value),
        ArrayConstant::Error(code) => Scalar::Error(code.clone()),
    }
}

fn eq_name(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right) || left.to_uppercase() == right.to_uppercase()
}

fn is_error_value(value: &Value) -> bool {
    matches!(value, Value::Scalar(Scalar::Error(_)))
}

fn is_name_error(value: &Value) -> bool {
    matches!(value, Value::Scalar(Scalar::Error(code)) if code == "#NAME?")
}

fn norm_range(range: CellRange) -> CellRange {
    CellRange {
        start: CellAddress {
            row: range.start.row.min(range.end.row),
            col: range.start.col.min(range.end.col),
        },
        end: CellAddress {
            row: range.start.row.max(range.end.row),
            col: range.start.col.max(range.end.col),
        },
    }
}

fn column_index(table: &TableMeta, name: &str) -> Option<u32> {
    table
        .columns
        .iter()
        .position(|column| eq_name(column, name))
        .map(|index| table.range.start.col + index as u32)
}

fn range_operator(left: Value, right: Value) -> Value {
    let (Value::Ref(left), Value::Ref(right)) = (left, right) else {
        return Value::err("#VALUE!");
    };
    if left.len() != 1 || right.len() != 1 {
        return Value::err("#VALUE!");
    }
    let left = &left[0];
    let right = &right[0];
    if left.sheet != right.sheet {
        return Value::err("#VALUE!");
    }
    Value::Ref(vec![Area {
        sheet: left.sheet,
        range: CellRange {
            start: CellAddress {
                row: left.range.start.row.min(right.range.start.row),
                col: left.range.start.col.min(right.range.start.col),
            },
            end: CellAddress {
                row: left.range.end.row.max(right.range.end.row),
                col: left.range.end.col.max(right.range.end.col),
            },
        },
    }])
}

fn intersect_operator(left: Value, right: Value) -> Value {
    let (Value::Ref(left), Value::Ref(right)) = (left, right) else {
        return Value::err("#VALUE!");
    };
    let mut out = Vec::new();
    for x in &left {
        for y in &right {
            if x.sheet != y.sheet {
                continue;
            }
            let start = CellAddress {
                row: x.range.start.row.max(y.range.start.row),
                col: x.range.start.col.max(y.range.start.col),
            };
            let end = CellAddress {
                row: x.range.end.row.min(y.range.end.row),
                col: x.range.end.col.min(y.range.end.col),
            };
            if start.row <= end.row && start.col <= end.col {
                out.push(Area {
                    sheet: x.sheet,
                    range: CellRange { start, end },
                });
            }
        }
    }
    if out.is_empty() {
        Value::err("#NULL!")
    } else {
        Value::Ref(out)
    }
}

fn union_operator(left: Value, right: Value) -> Value {
    let (Value::Ref(mut left), Value::Ref(right)) = (left, right) else {
        return Value::err("#VALUE!");
    };
    left.extend(right);
    Value::Ref(left)
}

fn broadcast1(value: Value, f: impl Fn(&Scalar) -> Scalar) -> Value {
    match value {
        Value::Matrix(matrix) => Value::Matrix(matrix.map_values(&f)),
        Value::Scalar(scalar) => Value::Scalar(f(&scalar)),
        other => other,
    }
}

fn broadcast2(left: Value, right: Value, f: impl Fn(&Scalar, &Scalar) -> Scalar) -> Value {
    match (left, right) {
        (Value::Scalar(left), Value::Scalar(right)) => Value::Scalar(f(&left, &right)),
        (left, right) => {
            let left = as_matrix(left);
            let right = as_matrix(right);
            match broadcast_shape(&left, &right, &f) {
                Ok(matrix) => Value::Matrix(matrix),
                Err(code) => Value::err(&code),
            }
        }
    }
}

fn as_matrix(value: Value) -> Matrix {
    match value {
        Value::Matrix(matrix) => matrix,
        Value::Scalar(scalar) => Matrix::plain(vec![vec![scalar]]),
        Value::Ref(_) => Matrix::plain(vec![vec![Scalar::error("#VALUE!")]]),
        Value::Lambda(_) => Matrix::plain(vec![vec![Scalar::error("#CALC!")]]),
    }
}

fn broadcast_shape(
    left: &Matrix,
    right: &Matrix,
    f: &impl Fn(&Scalar, &Scalar) -> Scalar,
) -> Result<Matrix, String> {
    let rows = left.rows().max(right.rows());
    let cols = left.cols().max(right.cols());
    let at = |row: u32, col: u32| f(&pick(left, row, col), &pick(right, row, col));
    if !left.is_padded() && !right.is_padded() {
        return matrix_build(rows, cols, at);
    }
    let block_rows = left
        .stored_rows()
        .max(if right.is_padded() {
            right.stored_rows()
        } else {
            right.rows()
        })
        .min(rows);
    let block_cols = left
        .stored_cols()
        .max(if right.is_padded() {
            right.stored_cols()
        } else {
            right.cols()
        })
        .min(cols);
    let varies = [left, right].into_iter().any(|matrix| {
        (block_rows < rows && matrix.rows() == 1 && matrix.cols() > 1)
            || (block_cols < cols && matrix.cols() == 1 && matrix.rows() > 1)
    });
    if varies {
        return matrix_build(rows, cols, at);
    }
    let block = matrix_build(block_rows, block_cols, at)?;
    Ok(block.padded(
        rows,
        cols,
        at(rows.saturating_sub(1), cols.saturating_sub(1)),
    ))
}

fn matrix_build(
    rows: u32,
    cols: u32,
    mut at: impl FnMut(u32, u32) -> Scalar,
) -> Result<Matrix, String> {
    if rows > MAX_ARRAY_SIDE || cols > MAX_ARRAY_SIDE {
        return Err("#VALUE!".to_owned());
    }
    if u64::from(rows) * u64::from(cols) > MAX_ARRAY_CELLS {
        return Err("#NUM!".to_owned());
    }
    let mut dense = Vec::with_capacity(rows as usize);
    for row in 0..rows {
        let mut line = Vec::with_capacity(cols as usize);
        for col in 0..cols {
            line.push(at(row, col));
        }
        dense.push(line);
    }
    Ok(Matrix::plain(dense))
}

fn scalar_unary(op: char, value: &Scalar, date1904: bool) -> Scalar {
    if let Scalar::Error(code) = value {
        return Scalar::Error(code.clone());
    }
    match to_number(value, date1904) {
        Ok(n) => nz(if op == '-' { -n } else { n / 100.0 }),
        Err(code) => Scalar::error(&code),
    }
}

fn scalar_binary(
    op: BinaryOp,
    left: &Scalar,
    right: &Scalar,
    snap: bool,
    date1904: bool,
) -> Scalar {
    if let Scalar::Error(code) = left {
        return Scalar::Error(code.clone());
    }
    if let Scalar::Error(code) = right {
        return Scalar::Error(code.clone());
    }
    match op {
        BinaryOp::Concat => match (to_text(left), to_text(right)) {
            (Ok(left), Ok(right)) => Scalar::Text(left + &right),
            (Err(code), _) | (_, Err(code)) => Scalar::error(&code),
        },
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow => {
            match (to_number(left, date1904), to_number(right, date1904)) {
                (Ok(a), Ok(b)) => {
                    let result = match op {
                        BinaryOp::Add => {
                            let mut r = a + b;
                            if snap {
                                r = snap_to_zero(r, a, b);
                            }
                            r
                        }
                        BinaryOp::Sub => {
                            let mut r = a - b;
                            if snap {
                                r = snap_to_zero(r, a, b);
                            }
                            r
                        }
                        BinaryOp::Mul => a * b,
                        BinaryOp::Div => {
                            if b == 0.0 {
                                return Scalar::error("#DIV/0!");
                            }
                            a / b
                        }
                        _ => match power(a, b) {
                            Ok(n) => n,
                            Err(code) => return Scalar::error(&code),
                        },
                    };
                    nz(result)
                }
                (Err(code), _) | (_, Err(code)) => Scalar::error(&code),
            }
        }
        _ => {
            let order = compare_scalars(left, right);
            Scalar::Bool(match op {
                BinaryOp::Eq => order == 0,
                BinaryOp::Ne => order != 0,
                BinaryOp::Lt => order < 0,
                BinaryOp::Gt => order > 0,
                BinaryOp::Le => order <= 0,
                _ => order >= 0,
            })
        }
    }
}

fn power(base: f64, exponent: f64) -> Result<f64, String> {
    if base == 0.0 && exponent == 0.0 {
        return Err("#NUM!".to_owned());
    }
    if base == 0.0 && exponent < 0.0 {
        return Err("#DIV/0!".to_owned());
    }
    if base < 0.0 && !(exponent.is_finite() && exponent.fract() == 0.0) {
        let inverse = 1.0 / exponent;
        let odd = inverse.round();
        if (inverse - odd).abs() < 1e-10 && odd.abs() % 2.0 == 1.0 {
            return Ok(-(-base).powf(exponent));
        }
        return Err("#NUM!".to_owned());
    }
    Ok(base.powf(exponent))
}

fn snap_to_zero(result: f64, left: f64, right: f64) -> f64 {
    if result != 0.0 && result.abs() <= left.abs().max(right.abs()) * 1e-15 {
        0.0
    } else {
        result
    }
}

fn to_number(value: &Scalar, date1904: bool) -> Result<f64, String> {
    match value {
        Scalar::Number(n) => Ok(*n),
        Scalar::Empty => Ok(0.0),
        Scalar::Bool(value) => Ok(if *value { 1.0 } else { 0.0 }),
        Scalar::Text(text) => parse_number_text(text, date1904).ok_or_else(|| "#VALUE!".to_owned()),
        Scalar::Error(code) => Err(code.clone()),
    }
}

fn to_text(value: &Scalar) -> Result<String, String> {
    match value {
        Scalar::Text(text) => Ok(text.clone()),
        Scalar::Number(n) => Ok(number_to_text(*n)),
        Scalar::Bool(value) => Ok(if *value { "TRUE" } else { "FALSE" }.to_owned()),
        Scalar::Empty => Ok(String::new()),
        Scalar::Error(code) => Err(code.clone()),
    }
}

fn compare_scalars(left: &Scalar, right: &Scalar) -> i32 {
    match (left, right) {
        (Scalar::Empty, Scalar::Empty) => 0,
        (Scalar::Empty, other) => compare_scalars(&blank_like(other), other),
        (other, Scalar::Empty) => compare_scalars(other, &blank_like(other)),
        _ => {
            let left_rank = type_rank(left);
            let right_rank = type_rank(right);
            if left_rank != right_rank {
                return if left_rank < right_rank { -1 } else { 1 };
            }
            match (left, right) {
                (Scalar::Number(a), Scalar::Number(b)) => compare_numbers(*a, *b),
                (Scalar::Text(a), Scalar::Text(b)) => compare_text(a, b),
                (Scalar::Bool(a), Scalar::Bool(b)) => {
                    if a == b {
                        0
                    } else if *a {
                        1
                    } else {
                        -1
                    }
                }
                _ => 0,
            }
        }
    }
}

fn blank_like(value: &Scalar) -> Scalar {
    match value {
        Scalar::Number(_) => Scalar::Number(0.0),
        Scalar::Text(_) => Scalar::Text(String::new()),
        Scalar::Bool(_) => Scalar::Bool(false),
        other => other.clone(),
    }
}

fn type_rank(value: &Scalar) -> u8 {
    match value {
        Scalar::Number(_) => 0,
        Scalar::Text(_) => 1,
        Scalar::Bool(_) => 2,
        _ => 3,
    }
}

fn compare_numbers(left: f64, right: f64) -> i32 {
    if left == right {
        return 0;
    }
    if (left - right).abs() > left.abs().max(right.abs()) * 1e-14 {
        return if left < right { -1 } else { 1 };
    }
    let left = round15(left);
    let right = round15(right);
    if left == right {
        0
    } else if left < right {
        -1
    } else {
        1
    }
}

fn compare_text(left: &str, right: &str) -> i32 {
    let left = left.to_lowercase();
    let right = right.to_lowercase();
    if left == right {
        0
    } else if left < right {
        -1
    } else {
        1
    }
}

fn round15(value: f64) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let negative = value.is_sign_negative();
    let (digits, exp) = exp_15(value.abs());
    let mut scientific = String::new();
    scientific.push(digits.as_bytes()[0] as char);
    scientific.push('.');
    scientific.push_str(&digits[1..]);
    scientific.push('e');
    scientific.push(if exp < 0 { '-' } else { '+' });
    scientific.push_str(&exp.abs().to_string());
    let parsed = scientific.parse::<f64>().unwrap_or(value.abs());
    if negative {
        -parsed
    } else {
        parsed
    }
}

fn number_to_text(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if !value.is_finite() {
        return "#NUM!".to_owned();
    }
    let rounded = round15(value);
    let negative = rounded < 0.0;
    let (digits, exponent) = exp_15(rounded.abs());
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let plain = if exponent >= 0 {
        if digits.len() as i32 <= exponent + 1 {
            format!(
                "{digits}{}",
                "0".repeat((exponent + 1 - digits.len() as i32) as usize)
            )
        } else {
            format!(
                "{}.{}",
                &digits[..(exponent as usize + 1)],
                &digits[(exponent as usize + 1)..]
            )
        }
    } else {
        format!("0.{}{digits}", "0".repeat((-exponent - 1) as usize))
    };
    if plain.len() <= 20 {
        return if negative { format!("-{plain}") } else { plain };
    }
    let mantissa = if digits.len() > 1 {
        format!("{}.{}", &digits[..1], &digits[1..])
    } else {
        digits.to_owned()
    };
    let exp = format!(
        "{}{:02}",
        if exponent < 0 { '-' } else { '+' },
        exponent.abs()
    );
    format!("{}{mantissa}E{exp}", if negative { "-" } else { "" })
}

fn exp_15(abs: f64) -> (String, i32) {
    let rendered = format!("{abs:.40e}");
    let pos = rendered.find(['e', 'E']).unwrap_or(rendered.len());
    let mantissa = &rendered[..pos];
    let mut exp: i32 = rendered[pos + 1..].parse().unwrap_or(0);
    let raw: Vec<u8> = mantissa.bytes().filter(|byte| *byte != b'.').collect();
    let mut head: Vec<u8> = raw.iter().copied().take(15).collect();
    while head.len() < 15 {
        head.push(b'0');
    }
    let next = raw.get(15).copied().unwrap_or(b'0');
    if next >= b'5' {
        let mut index = head.len() as i32 - 1;
        while index >= 0 && head[index as usize] == b'9' {
            head[index as usize] = b'0';
            index -= 1;
        }
        if index < 0 {
            head.insert(0, b'1');
            head.truncate(15);
            exp += 1;
        } else {
            head[index as usize] += 1;
        }
    }
    (
        String::from_utf8(head).unwrap_or_else(|_| "0".repeat(15)),
        exp,
    )
}

fn parse_number_text(input: &str, date1904: bool) -> Option<f64> {
    let mut text = input.trim().to_owned();
    if text.is_empty() {
        return None;
    }
    let mut scale = 1.0;
    if text.ends_with('%') {
        scale = 0.01;
        text = text[..text.len() - 1].trim_end().to_owned();
    }
    let mut sign = 1.0;
    if text.starts_with('(') && text.ends_with(')') {
        sign = -1.0;
        text = text[1..text.len() - 1].trim().to_owned();
    }
    if let Some(captures) = re(r"^([+-]?)\s*\$?\s*([+-]?)\s*").captures(&text) {
        let sign_text = format!("{}{}", captures.get(1)?.as_str(), captures.get(2)?.as_str());
        if sign_text.len() <= 1 {
            let body = &text[captures.get(0)?.end()..];
            if let Some(plain) = parse_plain_number(body) {
                let signed = if sign_text == "-" { -1.0 } else { 1.0 };
                return Some(signed * sign * plain * scale);
            }
        }
    }
    if scale != 1.0 || sign != 1.0 {
        return None;
    }
    if let Some(fraction) = re(r"^([+-]?)(?:(\d+)\s+)?(\d+)/(\d+)$").captures(&text) {
        if fraction.get(2).is_some() {
            let den: f64 = fraction.get(4)?.as_str().parse().ok()?;
            if den == 0.0 {
                return None;
            }
            let whole: f64 = fraction.get(2)?.as_str().parse().ok()?;
            let num: f64 = fraction.get(3)?.as_str().parse().ok()?;
            let value = whole + num / den;
            return Some(if fraction.get(1)?.as_str() == "-" {
                -value
            } else {
                value
            });
        }
    }
    if let Some(time) = parse_time(&text) {
        return Some(time);
    }
    if let Some(date) = parse_date(&text, date1904) {
        return Some(date);
    }
    if let Some(space) = text.rfind(' ') {
        if space > 0 {
            if let (Some(date), Some(time)) = (
                parse_date(text[..space].trim(), date1904),
                parse_time(text[space + 1..].trim()),
            ) {
                return Some(date + time);
            }
            if let Some(captures) = re(r"(?i)^(.*\S)\s+(\S+\s*(?:am|pm))$").captures(&text) {
                if let (Some(date), Some(time)) = (
                    parse_date(captures.get(1)?.as_str(), date1904),
                    parse_time(captures.get(2)?.as_str()),
                ) {
                    return Some(date + time);
                }
            }
        }
    }
    None
}

fn parse_plain_number(text: &str) -> Option<f64> {
    let captures = re(r"(?i)^(\d+(?:,\d{3,})+|\d*)(\.\d*)?(?:e([+-]?\d+))?$").captures(text)?;
    let int = captures
        .get(1)
        .map(|m| m.as_str())
        .unwrap_or("")
        .replace(',', "");
    let frac = captures.get(2).map(|m| m.as_str()).unwrap_or("");
    if int.is_empty() && frac.len() < 2 {
        return None;
    }
    let exp = captures
        .get(3)
        .map(|m| format!("e{}", m.as_str()))
        .unwrap_or_default();
    let frac = if frac == "." { "" } else { frac };
    let value: f64 = format!(
        "{}{frac}{exp}",
        if int.is_empty() { "0" } else { int.as_str() }
    )
    .parse()
    .ok()?;
    value.is_finite().then_some(value)
}

fn parse_time(text: &str) -> Option<f64> {
    let captures =
        re(r"(?i)^(\d{1,4})(?::(\d+))?(?::(\d+(?:\.\d*)?))?(?:\s+(am|pm|a|p))?$").captures(text)?;
    let mut hours: f64 = captures.get(1)?.as_str().parse().ok()?;
    let minutes: f64 = captures
        .get(2)
        .map(|m| m.as_str().parse().unwrap_or(0.0))
        .unwrap_or(0.0);
    let seconds: f64 = captures
        .get(3)
        .map(|m| m.as_str().parse().unwrap_or(0.0))
        .unwrap_or(0.0);
    let meridiem = captures.get(4).map(|m| m.as_str().to_lowercase());
    if captures.get(2).is_none() && (meridiem.is_none() || captures.get(3).is_some()) {
        return None;
    }
    if let Some(meridiem) = meridiem {
        if hours > 12.0 {
            return None;
        }
        if hours == 12.0 {
            hours = 0.0;
        }
        if meridiem.starts_with('p') {
            hours += 12.0;
        }
    }
    Some((hours * 3600.0 + minutes * 60.0 + seconds) / 86_400.0)
}

fn parse_date(text: &str, date1904: bool) -> Option<f64> {
    let year_now = utc_year();
    if let Some(captures) = re(r"^(\d{4})[-/](\d{1,2})[-/](\d{1,2})$").captures(text) {
        return serial_or_undefined(
            captures.get(1)?.as_str().parse().ok()?,
            captures.get(2)?.as_str().parse().ok()?,
            captures.get(3)?.as_str().parse().ok()?,
            date1904,
        );
    }
    if let Some(captures) = re(r"^(\d{1,2})/(\d{1,2})(?:/(\d{2}|\d{4}))?$").captures(text) {
        let year = captures
            .get(3)
            .map(|m| full_year(m.as_str()))
            .unwrap_or(year_now);
        return serial_or_undefined(
            year,
            captures.get(1)?.as_str().parse().ok()?,
            captures.get(2)?.as_str().parse().ok()?,
            date1904,
        );
    }
    if let Some(captures) =
        re(r"(?i)^(\d{1,2})[-\s]([a-z]+)\.?(?:[-\s,]+(\d{2}|\d{4}))?$").captures(text)
    {
        let month = month_from_name(captures.get(2)?.as_str())?;
        let year = captures
            .get(3)
            .map(|m| full_year(m.as_str()))
            .unwrap_or(year_now);
        return serial_or_undefined(
            year,
            month,
            captures.get(1)?.as_str().parse().ok()?,
            date1904,
        );
    }
    if let Some(captures) = re(r"(?i)^([a-z]+)\.?[-\s](\d{1,2})(?:,?\s*(\d{4}))?$").captures(text) {
        let month = month_from_name(captures.get(1)?.as_str())?;
        let year = captures
            .get(3)
            .map(|m| m.as_str().parse().unwrap_or(year_now))
            .unwrap_or(year_now);
        return serial_or_undefined(
            year,
            month,
            captures.get(2)?.as_str().parse().ok()?,
            date1904,
        );
    }
    if let Some(captures) = re(r"(?i)^([a-z]+)\.?[-\s](\d{4})$").captures(text) {
        let month = month_from_name(captures.get(1)?.as_str())?;
        return serial_or_undefined(captures.get(2)?.as_str().parse().ok()?, month, 1, date1904);
    }
    None
}

fn full_year(text: &str) -> i32 {
    let year: i32 = text.parse().unwrap_or(0);
    if text.len() > 2 {
        year
    } else if year < 30 {
        2000 + year
    } else {
        1900 + year
    }
}

fn month_from_name(name: &str) -> Option<i32> {
    const MONTHS: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    let lower = name.to_lowercase();
    if lower.len() < 3 {
        return None;
    }
    let index = MONTHS
        .iter()
        .position(|month| lower.starts_with(&month[..3]))?;
    let full = MONTHS[index];
    if lower.len() == 3 || full.starts_with(&lower) || lower == "sept" {
        Some(index as i32 + 1)
    } else {
        None
    }
}

fn serial_or_undefined(year: i32, month: i32, day: i32, date1904: bool) -> Option<f64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if year < if date1904 { 1904 } else { 1900 } {
        return None;
    }
    let phantom = !date1904 && year == 1900 && month == 2 && day == 29;
    if !phantom && day > days_in_month(year, month) {
        return None;
    }
    ymd_to_serial(year, month, day, date1904)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn ymd_to_serial(year: i32, month: i32, day: i32, date1904: bool) -> Option<f64> {
    let total = i64::from(year) * 12 + i64::from(month) - 1;
    let y = total.div_euclid(12);
    if y > 9999 {
        return None;
    }
    let month0 = total.rem_euclid(12) as u32;
    let serial = month_start_serial(y as i32, month0, date1904) + f64::from(day) - 1.0;
    let limit = month_start_serial(10_000, 0, date1904);
    (serial >= 0.0 && serial < limit).then_some(serial)
}

fn month_start_serial(year: i32, month0: u32, date1904: bool) -> f64 {
    let month = month0 + 1;
    let (year, month) = if month == 13 {
        (year + 1, 1)
    } else {
        (year, month)
    };
    let epoch = if date1904 {
        days_from_civil(1904, 1, 1)
    } else {
        days_from_civil(1899, 12, 30)
    };
    let days = days_from_civil(year, month, 1) - epoch;
    if !date1904 && days < 61 {
        (days - 1) as f64
    } else {
        days as f64
    }
}

fn days_from_civil(mut year: i32, month: u32, day: u32) -> i64 {
    year -= i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = (year - era * 400) as u64;
    let mp = if month > 2 {
        month as i64 - 3
    } else {
        month as i64 + 9
    };
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    i64::from(era) * 146_097 + doe as i64 - 719_468
}

fn utc_year() -> i32 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let (year, _, _) = civil_from_days(days);
    year
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year as i32, month as u32, day as u32)
}

fn re(pattern: &str) -> Regex {
    let mut cache = REGEX.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(regex) = cache.get(pattern) {
        return regex.clone();
    }
    let regex = Regex::new(pattern).expect("number pattern");
    cache.insert(pattern.to_owned(), regex.clone());
    regex
}

fn json_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

fn bool_json(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn scalar_json(value: &Scalar) -> String {
    match value {
        Scalar::Empty => "null".to_owned(),
        Scalar::Number(n) => js_number_string(*n),
        Scalar::Text(text) => json_string(text),
        Scalar::Bool(value) => bool_json(*value).to_owned(),
        Scalar::Error(code) => format!("{{\"error\":{}}}", json_string(code)),
    }
}

fn areas_json(areas: &[Area]) -> String {
    let mut out = String::from("[");
    for (index, area) in areas.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"sheet\":{},\"range\":{{\"start\":{{\"row\":{},\"col\":{}}},\"end\":{{\"row\":{},\"col\":{}}}}}}}",
            area.sheet,
            area.range.start.row,
            area.range.start.col,
            area.range.end.row,
            area.range.end.col
        ));
    }
    out.push(']');
    out
}

fn load_tables(bytes: &[u8], parts: &[Option<String>]) -> Result<Vec<Vec<TableMeta>>, String> {
    let mut pkg = Package::open(bytes).map_err(|err| err.to_string())?;
    let mut out = Vec::with_capacity(parts.len());
    for part in parts {
        let Some(part) = part else {
            out.push(Vec::new());
            continue;
        };
        let xml = pkg
            .text(part)
            .map_err(|err| err.to_string())?
            .unwrap_or("")
            .to_owned();
        let ids = table_rel_ids(&xml);
        let rels: Vec<(String, Relationship)> = pkg
            .rels(part)
            .map_err(|err| err.to_string())?
            .iter()
            .map(|(id, rel)| (id.clone(), rel.clone()))
            .collect();
        let mut tables = Vec::new();
        for id in ids {
            let Some((_, rel)) = rels.iter().find(|(key, _)| key == &id) else {
                continue;
            };
            if rel.type_ != relationship_types::TABLE && !rel.type_.ends_with("/table") {
                continue;
            }
            let Some(target) = pkg.target(part, rel) else {
                continue;
            };
            let table_xml = pkg
                .text(&target)
                .map_err(|err| err.to_string())?
                .unwrap_or("")
                .to_owned();
            if let Some(table) = parse_table_xml(&table_xml) {
                tables.push(table);
            }
        }
        out.push(tables);
    }
    Ok(out)
}

fn table_rel_ids(xml: &str) -> Vec<String> {
    let owned = split_sheet_data(xml).map(|split| split.shell_xml);
    let text = owned.as_deref().unwrap_or(xml);
    if let Ok(doc) = parse_xml_str(text) {
        let mut ids = Vec::new();
        if let Some(parts) = x_first(Some(&doc), "tableParts") {
            for part in x_children(parts, "tablePart") {
                if let Some(id) = rel_attr(Some(part), "id") {
                    ids.push(id.to_owned());
                }
            }
        }
        return ids;
    }
    let mut ids = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("tablePart") {
        let tag = &rest[start..];
        let end = tag.find('>').unwrap_or(tag.len());
        let element = &tag[..end];
        if let Some(id) = attr_of(element, "r:id") {
            ids.push(id);
        }
        rest = &tag[end..];
    }
    ids
}

fn attr_of(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_owned())
}

fn parse_table_xml(xml: &str) -> Option<TableMeta> {
    let doc = parse_xml_str(xml).ok()?;
    let range = parse_range(att(Some(&doc), "ref")?)?;
    let name = att(Some(&doc), "name")
        .or_else(|| att(Some(&doc), "displayName"))
        .unwrap_or("Table")
        .to_owned();
    let display = att(Some(&doc), "displayName").unwrap_or(&name).to_owned();
    let header = num_attr(Some(&doc), "headerRowCount").unwrap_or(1.0) > 0.0;
    let totals = num_attr(Some(&doc), "totalsRowCount").unwrap_or(0.0) > 0.0;
    let mut columns = Vec::new();
    if let Some(parent) = x_first(Some(&doc), "tableColumns") {
        for column in x_children(parent, "tableColumn") {
            columns.push(att(Some(column), "name").unwrap_or("").to_owned());
        }
    }
    Some(TableMeta {
        name,
        display,
        range,
        header,
        totals,
        columns,
    })
}

/// Open `bytes` and build a calc engine.
pub fn load_calc_engine(bytes: &[u8]) -> Result<CalcEngine, String> {
    CalcEngine::load(bytes)
}

static REGEX: std::sync::LazyLock<std::sync::Mutex<HashMap<String, Regex>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(cells: &[(u32, u32, Option<&str>, f64)]) -> CalcEngine {
        let mut grid = SheetGrid {
            cells: HashMap::new(),
            rows: BTreeMap::new(),
            merges: Vec::new(),
            tables: Vec::new(),
            hidden: BTreeSet::new(),
        };
        let mut locs = Vec::new();
        for (row, col, formula, value) in cells {
            if formula.is_some() {
                locs.push((0, *row, *col));
            }
            grid.insert(
                *row,
                *col,
                HostCell {
                    value: Scalar::Number(*value),
                    formula: formula.map(str::to_owned),
                    spill_anchor: None,
                    legacy: formula.is_some(),
                    array_range: None,
                },
            );
        }
        CalcEngine {
            sheet_names: vec!["Sheet1".to_owned()],
            date1904: false,
            iterate: None,
            warnings: Vec::new(),
            defined: Vec::new(),
            sheets: vec![grid],
            nodes: Vec::new(),
            by_cell: HashMap::new(),
            formula_locs: locs,
            reverse: ReverseIndex::new(),
            prep_at: 0,
            dynamic_ids: Vec::new(),
            unvalued_ids: Vec::new(),
            built: false,
            needs_full: false,
            fresh: true,
            ranked: false,
            eval_active: false,
            iterating: false,
            pending: HashSet::new(),
            in_progress: HashSet::new(),
            cycles: InsSet::default(),
            dynamic_hits: HashSet::new(),
            dynamic_hit: false,
            compute_depth: 0,
            evaluations: 0,
            evaluated_at: HashMap::new(),
            footprint_changes: Vec::new(),
            bounds_cache: vec![None],
            parsed: HashMap::new(),
            rand_seed: Cell::new(42),
        }
    }

    fn node(engine: &CalcEngine, row: u32, col: u32) -> &FormulaNode {
        let id = engine.by_cell[&(0, row, col)];
        &engine.nodes[id as usize]
    }

    #[test]
    fn self_cycle_stores_zero() {
        // Catches: a formula that references its own cell keeps a cached non-zero value.
        let mut engine = engine(&[(0, 0, Some("=A1"), 7.0)]);
        engine.prepare(None);
        engine.recalculate_all();
        let formula = node(&engine, 0, 0);
        assert!(formula.cyclic);
        assert!(engine.cycles.contains(engine.by_cell[&(0, 0, 0)]));
        assert!(matches!(engine.cell_scalar(0, 0, 0), Scalar::Number(n) if n == 0.0));
    }

    #[test]
    fn offset_and_index_colon_are_not_static_cycles() {
        // Catches: OFFSET's reference argument and ':' between INDEX calls become static self-cycles.
        let mut engine = engine(&[
            (1, 1, Some("=OFFSET(B2,-1,0)+1"), 5.0),
            (1, 0, Some("=INDEX(A:A,1):INDEX(A:A,3)"), 4.0),
        ]);
        engine.prepare(None);
        engine.recalculate_all();
        let offset = node(&engine, 1, 1);
        assert!(offset
            .dynamic_deps
            .iter()
            .any(|area| { area.range.start.row == 1 && area.range.start.col == 1 }));
        assert!(!offset.deps.iter().any(|area| range_has(&area.range, 1, 1)));
        assert!(!offset.cyclic);
        assert!(!engine.cycles.contains(engine.by_cell[&(0, 1, 1)]));
        let index = node(&engine, 1, 0);
        assert!(!index.dynamic_deps.is_empty());
        assert!(!index.cyclic);
        // The range covers the anchor, so evaluating it records a dynamic cycle.
        // The static `cyclic` flag stays clear.
        assert!(engine.cycles.contains(engine.by_cell[&(0, 1, 0)]));
    }

    #[test]
    fn volatile_and_dynamic_deps_recompute_on_unrelated_edit() {
        // Catches: recalculateFrom skips volatile NOW and an INDEX whose dynamic dep covers the edit.
        let mut engine = engine(&[
            (4, 0, None, 3.0),
            (0, 1, Some("=2+2"), 0.0),
            (0, 2, Some("=INDEX(A:A,1)"), 9.0),
            (0, 3, Some("=NOW()"), 1.0),
        ]);
        engine.prepare(None);
        engine.recalculate_all();
        assert!(matches!(engine.cell_scalar(0, 0, 1), Scalar::Number(n) if n == 4.0));
        let plain = node(&engine, 0, 1).computes;
        let index = node(&engine, 0, 2).computes;
        let now = node(&engine, 0, 3).computes;
        assert!(node(&engine, 0, 3).volatile);
        assert!(!node(&engine, 0, 2).volatile);
        engine.set_cell(0, 4, 0, Scalar::Number(8.0));
        engine.recalculate_from(&[CellPos {
            sheet: 0,
            row: 4,
            col: 0,
        }]);
        assert_eq!(node(&engine, 0, 1).computes, plain);
        assert!(node(&engine, 0, 2).computes > index);
        assert!(node(&engine, 0, 3).computes > now);
    }

    #[test]
    fn catches_catalogued_call_keeping_the_cached_value() {
        // Catches: SUM(B1,C1) and LET(x,SUM(1,2),x) leaving the cached 99 instead of calling the function library.
        let mut engine = engine(&[
            (0, 1, None, 2.0),
            (0, 2, None, 3.0),
            (1, 0, Some("=SUM(B1,C1)"), 99.0),
            (1, 1, Some("=LET(x,SUM(1,2),x)"), 99.0),
            (1, 2, Some("=NOSUCH(1)"), 99.0),
        ]);
        engine.prepare(None);
        engine.recalculate_all();
        assert!(
            matches!(engine.cell_scalar(0, 1, 0), Scalar::Number(n) if n == 5.0),
            "SUM(B1,C1) must be 5, got {:?}",
            engine.cell_scalar(0, 1, 0)
        );
        assert!(
            matches!(engine.cell_scalar(0, 1, 1), Scalar::Number(n) if n == 3.0),
            "LET must publish SUM(1,2) as 3, got {:?}",
            engine.cell_scalar(0, 1, 1)
        );
        assert!(
            matches!(engine.cell_scalar(0, 1, 2), Scalar::Number(n) if n == 99.0),
            "an unknown name with a cached value stays cached"
        );
    }
}
