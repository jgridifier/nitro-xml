// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/model.ts (style/theme/name types);
// changes listed in PROVENANCE.md.
//! SpreadsheetML model types used by the Stage-3 workbook read path.

use serde_json::{json, Map, Number, Value};

/// Serialize a finite f64 the way `JSON.stringify` does (no trailing `.0` on integers).
pub fn js_num(n: f64) -> Value {
    if n.is_finite() && n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
        Value::Number((n as i64).into())
    } else if let Some(num) = Number::from_f64(n) {
        Value::Number(num)
    } else {
        Value::Null
    }
}

fn format_number_token(num: &Number) -> String {
    if let Some(i) = num.as_i64() {
        return i.to_string();
    }
    if let Some(u) = num.as_u64() {
        return u.to_string();
    }
    if let Some(f) = num.as_f64() {
        if f.is_finite() && f.fract() == 0.0 {
            return format!("{:.0}", f);
        }
        return serde_json::to_string(&Value::Number(num.clone()))
            .unwrap_or_else(|_| f.to_string());
    }
    num.to_string()
}

/// `JSON.stringify` for gate dumps (integral doubles beyond `i64` stay decimal, not scientific).
pub fn stringify_json(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        Value::Number(n) => format_number_token(n),
        Value::String(s) => {
            if let Some(raw) = s.strip_prefix("__NUM__:") {
                raw.to_string()
            } else {
                serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
            }
        }
        Value::Array(a) => {
            let parts = a.iter().map(stringify_json).collect::<Vec<_>>();
            format!("[{}]", parts.join(","))
        }
        Value::Object(o) => {
            let parts = o
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap_or_else(|_| "\"\"".into()),
                        stringify_json(v)
                    )
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", parts.join(","))
        }
    }
}

/// `AARRGGBB` or `RRGGBB`, upper case; theme slot; tint; indexed; or auto.
#[derive(Debug, Clone, PartialEq)]
pub struct Color {
    pub rgb: Option<String>,
    pub theme: Option<f64>,
    pub tint: Option<f64>,
    pub indexed: Option<f64>,
    pub auto: Option<bool>,
}

impl Color {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(v) = &self.rgb {
            m.insert("rgb".into(), Value::String(v.clone()));
        }
        if let Some(v) = self.theme {
            m.insert("theme".into(), js_num(v));
        }
        if let Some(v) = self.tint {
            m.insert("tint".into(), js_num(v));
        }
        if let Some(v) = self.indexed {
            m.insert("indexed".into(), js_num(v));
        }
        if let Some(v) = self.auto {
            m.insert("auto".into(), Value::Bool(v));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    pub name: Option<String>,
    pub size: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<String>,
    pub strike: Option<bool>,
    pub color: Option<Color>,
    pub vert_align: Option<String>,
    pub family: Option<f64>,
    pub scheme: Option<String>,
}

impl Font {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(v) = &self.name {
            m.insert("name".into(), Value::String(v.clone()));
        }
        if let Some(v) = self.size {
            m.insert("size".into(), js_num(v));
        }
        if let Some(v) = self.bold {
            m.insert("bold".into(), Value::Bool(v));
        }
        if let Some(v) = self.italic {
            m.insert("italic".into(), Value::Bool(v));
        }
        if let Some(v) = &self.underline {
            m.insert("underline".into(), Value::String(v.clone()));
        }
        if let Some(v) = self.strike {
            m.insert("strike".into(), Value::Bool(v));
        }
        if let Some(v) = &self.color {
            m.insert("color".into(), v.to_json());
        }
        if let Some(v) = &self.vert_align {
            m.insert("vertAlign".into(), Value::String(v.clone()));
        }
        if let Some(v) = self.family {
            m.insert("family".into(), js_num(v));
        }
        if let Some(v) = &self.scheme {
            m.insert("scheme".into(), Value::String(v.clone()));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GradientStop {
    pub position: f64,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    Pattern {
        pattern: String,
        fg_color: Option<Color>,
        bg_color: Option<Color>,
    },
    Gradient {
        gradient: String,
        degree: Option<f64>,
        stops: Vec<GradientStop>,
    },
}

impl Fill {
    pub fn to_json(&self) -> Value {
        match self {
            Fill::Pattern {
                pattern,
                fg_color,
                bg_color,
            } => {
                let mut m = Map::new();
                m.insert("type".into(), Value::String("pattern".into()));
                m.insert("pattern".into(), Value::String(pattern.clone()));
                if let Some(c) = fg_color {
                    m.insert("fgColor".into(), c.to_json());
                }
                if let Some(c) = bg_color {
                    m.insert("bgColor".into(), c.to_json());
                }
                Value::Object(m)
            }
            Fill::Gradient {
                gradient,
                degree,
                stops,
            } => {
                let mut m = Map::new();
                m.insert("type".into(), Value::String("gradient".into()));
                m.insert("gradient".into(), Value::String(gradient.clone()));
                if let Some(d) = degree {
                    m.insert("degree".into(), js_num(*d));
                }
                let stops_json: Vec<Value> = stops
                    .iter()
                    .map(|s| {
                        let mut sm = Map::new();
                        sm.insert("position".into(), js_num(s.position));
                        sm.insert("color".into(), s.color.to_json());
                        Value::Object(sm)
                    })
                    .collect();
                m.insert("stops".into(), Value::Array(stops_json));
                Value::Object(m)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BorderEdge {
    pub style: String,
    pub color: Option<Color>,
}

impl BorderEdge {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("style".into(), Value::String(self.style.clone()));
        if let Some(c) = &self.color {
            m.insert("color".into(), c.to_json());
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Border {
    pub left: Option<BorderEdge>,
    pub right: Option<BorderEdge>,
    pub top: Option<BorderEdge>,
    pub bottom: Option<BorderEdge>,
    pub diagonal: Option<BorderEdge>,
    pub diagonal_up: Option<bool>,
    pub diagonal_down: Option<bool>,
}

impl Border {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(e) = &self.left {
            m.insert("left".into(), e.to_json());
        }
        if let Some(e) = &self.right {
            m.insert("right".into(), e.to_json());
        }
        if let Some(e) = &self.top {
            m.insert("top".into(), e.to_json());
        }
        if let Some(e) = &self.bottom {
            m.insert("bottom".into(), e.to_json());
        }
        if let Some(e) = &self.diagonal {
            m.insert("diagonal".into(), e.to_json());
        }
        if let Some(v) = self.diagonal_up {
            m.insert("diagonalUp".into(), Value::Bool(v));
        }
        if let Some(v) = self.diagonal_down {
            m.insert("diagonalDown".into(), Value::Bool(v));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alignment {
    pub horizontal: Option<String>,
    pub vertical: Option<String>,
    pub wrap_text: Option<bool>,
    pub shrink_to_fit: Option<bool>,
    pub indent: Option<f64>,
    pub text_rotation: Option<f64>,
    pub reading_order: Option<f64>,
}

impl Alignment {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(v) = &self.horizontal {
            m.insert("horizontal".into(), Value::String(v.clone()));
        }
        if let Some(v) = &self.vertical {
            m.insert("vertical".into(), Value::String(v.clone()));
        }
        if let Some(v) = self.wrap_text {
            m.insert("wrapText".into(), Value::Bool(v));
        }
        if let Some(v) = self.shrink_to_fit {
            m.insert("shrinkToFit".into(), Value::Bool(v));
        }
        if let Some(v) = self.indent {
            m.insert("indent".into(), js_num(v));
        }
        if let Some(v) = self.text_rotation {
            m.insert("textRotation".into(), js_num(v));
        }
        if let Some(v) = self.reading_order {
            m.insert("readingOrder".into(), js_num(v));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Protection {
    pub locked: Option<bool>,
    pub hidden: Option<bool>,
}

impl Protection {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(v) = self.locked {
            m.insert("locked".into(), Value::Bool(v));
        }
        if let Some(v) = self.hidden {
            m.insert("hidden".into(), Value::Bool(v));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CellStyle {
    pub font: Font,
    pub fill: Fill,
    pub border: Border,
    pub num_fmt: String,
    pub alignment: Option<Alignment>,
    pub protection: Option<Protection>,
    pub cell_style_name: Option<String>,
    pub quote_prefix: Option<bool>,
    pub pivot_button: Option<bool>,
}

impl CellStyle {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("font".into(), self.font.to_json());
        m.insert("fill".into(), self.fill.to_json());
        m.insert("border".into(), self.border.to_json());
        m.insert("numFmt".into(), Value::String(self.num_fmt.clone()));
        if let Some(a) = &self.alignment {
            m.insert("alignment".into(), a.to_json());
        }
        if let Some(p) = &self.protection {
            m.insert("protection".into(), p.to_json());
        }
        if let Some(v) = &self.cell_style_name {
            m.insert("cellStyleName".into(), Value::String(v.clone()));
        }
        if self.quote_prefix == Some(true) {
            m.insert("quotePrefix".into(), Value::Bool(true));
        }
        if self.pivot_button == Some(true) {
            m.insert("pivotButton".into(), Value::Bool(true));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DifferentialStyle {
    pub font: Option<Font>,
    pub fill: Option<Fill>,
    pub border: Option<Border>,
    pub num_fmt: Option<String>,
}

impl DifferentialStyle {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(f) = &self.font {
            m.insert("font".into(), f.to_json());
        }
        if let Some(f) = &self.fill {
            m.insert("fill".into(), f.to_json());
        }
        if let Some(b) = &self.border {
            m.insert("border".into(), b.to_json());
        }
        if let Some(n) = &self.num_fmt {
            m.insert("numFmt".into(), Value::String(n.clone()));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichTextRun {
    pub text: String,
    pub font: Option<Font>,
}

impl RichTextRun {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("text".into(), Value::String(self.text.clone()));
        if let Some(f) = &self.font {
            m.insert("font".into(), f.to_json());
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModernPasswordHash {
    pub algorithm_name: String,
    pub hash_value: String,
    pub salt_value: String,
    pub spin_count: Option<f64>,
}

impl ModernPasswordHash {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert(
            "algorithmName".into(),
            Value::String(self.algorithm_name.clone()),
        );
        m.insert("hashValue".into(), Value::String(self.hash_value.clone()));
        m.insert("saltValue".into(), Value::String(self.salt_value.clone()));
        if let Some(s) = self.spin_count {
            m.insert("spinCount".into(), js_num(s));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinedName {
    pub name: String,
    pub formula: String,
    pub local_sheet: Option<u32>,
    pub hidden: Option<bool>,
    pub comment: Option<String>,
}

impl DefinedName {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("name".into(), Value::String(self.name.clone()));
        m.insert("formula".into(), Value::String(self.formula.clone()));
        if let Some(i) = self.local_sheet {
            m.insert("localSheet".into(), json!(i));
        }
        if self.hidden == Some(true) {
            m.insert("hidden".into(), Value::Bool(true));
        }
        if let Some(c) = &self.comment {
            m.insert("comment".into(), Value::String(c.clone()));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePalette {
    pub colors: Vec<String>,
    pub major_font: String,
    pub minor_font: String,
}

impl ThemePalette {
    pub fn to_json(&self) -> Value {
        json!({
            "colors": self.colors,
            "majorFont": self.major_font,
            "minorFont": self.minor_font,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetState {
    Visible,
    Hidden,
    VeryHidden,
}

impl SheetState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Hidden => "hidden",
            Self::VeryHidden => "veryHidden",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellAddress {
    pub row: u32,
    pub col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellRange {
    pub start: CellAddress,
    pub end: CellAddress,
}

impl CellRange {
    pub fn to_json(&self) -> Value {
        json!({
            "start": { "row": self.start.row, "col": self.start.col },
            "end": { "row": self.end.row, "col": self.end.col },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSetup {
    pub print_area: Option<CellRange>,
}

impl PageSetup {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(r) = &self.print_area {
            m.insert("printArea".into(), r.to_json());
        }
        Value::Object(m)
    }
}

/// Sheet stub for Stage 3 (before worksheet cell parse).
#[derive(Debug, Clone, PartialEq)]
pub struct SheetStub {
    pub name: String,
    pub sheet_id: u32,
    pub state: SheetState,
    pub part_name: Option<String>,
    pub page_setup: Option<PageSetup>,
}

impl SheetStub {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("name".into(), Value::String(self.name.clone()));
        m.insert("sheetId".into(), json!(self.sheet_id));
        m.insert("state".into(), Value::String(self.state.as_str().into()));
        if let Some(p) = &self.part_name {
            m.insert("partName".into(), Value::String(p.clone()));
        }
        if let Some(ps) = &self.page_setup {
            m.insert("pageSetup".into(), ps.to_json());
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedStyle {
    pub name: String,
    pub style: CellStyle,
    pub builtin_id: Option<f64>,
}

impl NamedStyle {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("name".into(), Value::String(self.name.clone()));
        m.insert("style".into(), self.style.to_json());
        if let Some(id) = self.builtin_id {
            m.insert("builtinId".into(), js_num(id));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedString {
    pub text: String,
    pub runs: Option<Vec<RichTextRun>>,
}

impl SharedString {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("text".into(), Value::String(self.text.clone()));
        if let Some(runs) = &self.runs {
            m.insert(
                "runs".into(),
                Value::Array(runs.iter().map(RichTextRun::to_json).collect()),
            );
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SheetFormat {
    pub zero_height: Option<bool>,
    pub base_col_width: Option<f64>,
    pub custom_height: Option<bool>,
    pub thick_top: Option<bool>,
    pub thick_bottom: Option<bool>,
}

impl SheetFormat {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(v) = self.zero_height {
            m.insert("zeroHeight".into(), Value::Bool(v));
        }
        if let Some(v) = self.base_col_width {
            m.insert("baseColWidth".into(), js_num(v));
        }
        if let Some(v) = self.custom_height {
            m.insert("customHeight".into(), Value::Bool(v));
        }
        if let Some(v) = self.thick_top {
            m.insert("thickTop".into(), Value::Bool(v));
        }
        if let Some(v) = self.thick_bottom {
            m.insert("thickBottom".into(), Value::Bool(v));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FreezePane {
    pub rows: u32,
    pub cols: u32,
}

impl FreezePane {
    pub fn to_json(&self) -> Value {
        json!({ "rows": self.rows, "cols": self.cols })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SheetSelection {
    pub active: CellAddress,
    /// Cell that Shift+extend and drag grow from. File load sets this to `active`.
    /// Omitted from [`Self::to_json`] so sheet-view dumps stay aligned with upstream
    /// (`selection?: { active, ranges }`).
    pub anchor: CellAddress,
    pub ranges: Vec<CellRange>,
}

impl SheetSelection {
    pub fn to_json(&self) -> Value {
        json!({
            "active": { "row": self.active.row, "col": self.active.col },
            "ranges": self.ranges.iter().map(CellRange::to_json).collect::<Vec<_>>(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SheetView {
    pub show_grid_lines: bool,
    pub show_headers: bool,
    pub show_zeros: bool,
    pub right_to_left: bool,
    pub zoom: f64,
    pub freeze: Option<FreezePane>,
    pub top_left: Option<CellAddress>,
    pub selection: Option<SheetSelection>,
    pub show_formulas: Option<bool>,
}

impl SheetView {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("showGridLines".into(), Value::Bool(self.show_grid_lines));
        m.insert("showHeaders".into(), Value::Bool(self.show_headers));
        m.insert("showZeros".into(), Value::Bool(self.show_zeros));
        m.insert("rightToLeft".into(), Value::Bool(self.right_to_left));
        m.insert("zoom".into(), js_num(self.zoom));
        if let Some(f) = &self.freeze {
            m.insert("freeze".into(), f.to_json());
        }
        if let Some(t) = &self.top_left {
            m.insert("topLeft".into(), json!({ "row": t.row, "col": t.col }));
        }
        if let Some(s) = &self.selection {
            m.insert("selection".into(), s.to_json());
        }
        if self.show_formulas == Some(true) {
            m.insert("showFormulas".into(), Value::Bool(true));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnInfo {
    pub min: u32,
    pub max: u32,
    pub width: Option<f64>,
    pub custom_width: Option<bool>,
    pub hidden: Option<bool>,
    pub style_id: Option<u32>,
    pub outline_level: Option<f64>,
    pub collapsed: Option<bool>,
    pub best_fit: Option<bool>,
}

impl ColumnInfo {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("min".into(), json!(self.min));
        m.insert("max".into(), json!(self.max));
        if let Some(v) = self.width {
            m.insert("width".into(), js_num(v));
        }
        if self.custom_width == Some(true) {
            m.insert("customWidth".into(), Value::Bool(true));
        }
        if self.hidden == Some(true) {
            m.insert("hidden".into(), Value::Bool(true));
        }
        if let Some(s) = self.style_id {
            m.insert("styleId".into(), json!(s));
        }
        if let Some(o) = self.outline_level {
            m.insert("outlineLevel".into(), js_num(o));
        }
        if self.collapsed == Some(true) {
            m.insert("collapsed".into(), Value::Bool(true));
        }
        if self.best_fit == Some(true) {
            m.insert("bestFit".into(), Value::Bool(true));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RowInfo {
    pub height: Option<f64>,
    pub custom_height: Option<bool>,
    pub hidden: Option<bool>,
    pub style_id: Option<u32>,
    pub outline_level: Option<f64>,
    pub collapsed: Option<bool>,
}

impl RowInfo {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(v) = self.height {
            m.insert("height".into(), js_num(v));
        }
        if self.custom_height == Some(true) {
            m.insert("customHeight".into(), Value::Bool(true));
        }
        if self.hidden == Some(true) {
            m.insert("hidden".into(), Value::Bool(true));
        }
        if let Some(s) = self.style_id {
            m.insert("styleId".into(), json!(s));
        }
        if let Some(o) = self.outline_level {
            m.insert("outlineLevel".into(), js_num(o));
        }
        if self.collapsed == Some(true) {
            m.insert("collapsed".into(), Value::Bool(true));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DumpCellValue {
    Null,
    Number(f64),
    /// Numeric `<v>` text preserved for `JSON.stringify` parity on large integers.
    NumberRaw(String),
    String(String),
    Bool(bool),
    Error(String),
}

impl DumpCellValue {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Number(n) => js_num(*n),
            Self::NumberRaw(s) => Value::String(format!("__NUM__:{}", s)),
            Self::String(s) => Value::String(s.clone()),
            Self::Bool(b) => Value::Bool(*b),
            Self::Error(e) => json!({ "error": e }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DumpCell {
    pub row: u32,
    pub col: u32,
    pub value: DumpCellValue,
    pub type_tag: Option<String>,
    pub formula: Option<String>,
    pub style_id: Option<u32>,
    pub rich_text: Option<Vec<RichTextRun>>,
    pub array_range: Option<CellRange>,
    pub legacy_formula: Option<bool>,
    pub dynamic_array: Option<bool>,
    pub spill_anchor: Option<CellAddress>,
    pub cell_metadata: Option<u32>,
    pub value_metadata: Option<u32>,
}

impl DumpCell {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("r".into(), json!(self.row));
        m.insert("c".into(), json!(self.col));
        m.insert("v".into(), self.value.to_json());
        if let Some(t) = &self.type_tag {
            m.insert("t".into(), Value::String(t.clone()));
        }
        if let Some(f) = &self.formula {
            m.insert("f".into(), Value::String(f.clone()));
        }
        if let Some(s) = self.style_id {
            m.insert("s".into(), json!(s));
        }
        if let Some(rt) = &self.rich_text {
            m.insert(
                "richText".into(),
                Value::Array(rt.iter().map(RichTextRun::to_json).collect()),
            );
        }
        if let Some(r) = &self.array_range {
            m.insert("arrayRange".into(), r.to_json());
        }
        if self.legacy_formula == Some(true) {
            m.insert("legacyFormula".into(), Value::Bool(true));
        }
        if self.dynamic_array == Some(true) {
            m.insert("dynamicArray".into(), Value::Bool(true));
        }
        if let Some(a) = &self.spill_anchor {
            m.insert("spillAnchor".into(), json!({ "row": a.row, "col": a.col }));
        }
        if let Some(cm) = self.cell_metadata {
            m.insert("cellMetadata".into(), json!(cm));
        }
        if let Some(vm) = self.value_metadata {
            m.insert("valueMetadata".into(), json!(vm));
        }
        Value::Object(m)
    }
}

/// Cell marker inside a spreadsheet drawing anchor (EMUs for offsets).
#[derive(Debug, Clone, PartialEq)]
pub struct DrawingMarker {
    pub row: u32,
    pub col: u32,
    pub row_offset: i64,
    pub col_offset: i64,
}

/// Two-cell / one-cell / absolute drawing anchor (plane geometry via `anchor_to_pixel_box`).
#[derive(Debug, Clone, PartialEq)]
pub struct DrawingAnchor {
    pub from: DrawingMarker,
    pub to: Option<DrawingMarker>,
    /// Extent in EMUs (`cx`, `cy`) for one-cell / absolute anchors.
    pub ext: Option<(i64, i64)>,
}

/// Sheet drawing for the live grid (`#150` soft pixel / drawings layer).
#[derive(Debug, Clone, PartialEq)]
pub enum DrawingObject {
    Unsupported {
        anchor: DrawingAnchor,
        description: String,
    },
    Chart {
        anchor: DrawingAnchor,
        part_name: String,
        name: Option<String>,
        chart_xml: String,
    },
    Image {
        anchor: DrawingAnchor,
        part_name: String,
        content_type: String,
        name: Option<String>,
        description: Option<String>,
        /// `data:` URL for the picture part (embedded at load so the painter can emit `<img>`).
        data_url: Option<String>,
    },
}

impl DrawingObject {
    pub fn anchor(&self) -> &DrawingAnchor {
        match self {
            DrawingObject::Unsupported { anchor, .. }
            | DrawingObject::Chart { anchor, .. }
            | DrawingObject::Image { anchor, .. } => anchor,
        }
    }
}

/// List / custom validation rule ranges (view-IR only; not in Stage-16 worksheet JSON).
#[derive(Debug, Clone, PartialEq)]
pub struct DataValidation {
    pub ranges: Vec<CellRange>,
    pub validation_type: String,
    pub show_drop_down: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HyperlinkInfo {
    pub range: CellRange,
    pub tooltip: Option<String>,
    pub target: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sparkline {
    pub formula: Option<String>,
    pub host: CellRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SparklineGroup {
    pub type_: String,
    pub line_weight: f64,
    pub date_axis: bool,
    pub date_formula: Option<String>,
    pub markers: bool,
    pub high: bool,
    pub low: bool,
    pub first: bool,
    pub last: bool,
    pub negative: bool,
    pub display_empty_cells_as: String,
    pub display_x_axis: bool,
    pub display_hidden: bool,
    pub right_to_left: bool,
    pub min_axis_type: String,
    pub max_axis_type: String,
    pub manual_min: Option<f64>,
    pub manual_max: Option<f64>,
    pub color_series: Option<Color>,
    pub color_negative: Option<Color>,
    pub color_axis: Option<Color>,
    pub color_markers: Option<Color>,
    pub color_first: Option<Color>,
    pub color_last: Option<Color>,
    pub color_high: Option<Color>,
    pub color_low: Option<Color>,
    pub sparklines: Vec<Sparkline>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveWorksheet {
    pub name: String,
    pub sheet_id: u32,
    pub state: SheetState,
    pub part_name: Option<String>,
    pub default_row_height: f64,
    pub default_col_width: Option<f64>,
    pub format: Option<SheetFormat>,
    pub view: SheetView,
    pub columns: Vec<ColumnInfo>,
    pub merges: Vec<CellRange>,
    pub row_info: std::collections::BTreeMap<u32, RowInfo>,
    pub cells: Vec<DumpCell>,
    pub data_validations: Vec<DataValidation>,
    pub comment_addresses: Vec<CellAddress>,
    pub hyperlinks: Vec<HyperlinkInfo>,
    /// Preserved `extLst` XML fragments (sparklines and other extensions).
    pub ext_lst: Vec<String>,
    pub sparkline_groups: Vec<SparklineGroup>,
    /// Pictures / charts / unsupported shapes from the worksheet drawing part.
    pub drawings: Vec<DrawingObject>,
    /// Full grid extent for metrics. `None` on the full-fidelity load path (extent comes from
    /// stored cells). The lazy first view sets this from `<dimension>` or the streamed row
    /// prefix plus a stub so the grid does not collapse to the loaded window. Omitted from
    /// [`Self::to_json`] so dump parity stays on the full path.
    pub dimension: Option<CellRange>,
}

impl ActiveWorksheet {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("name".into(), Value::String(self.name.clone()));
        m.insert("sheetId".into(), json!(self.sheet_id));
        m.insert("state".into(), Value::String(self.state.as_str().into()));
        if let Some(p) = &self.part_name {
            m.insert("partName".into(), Value::String(p.clone()));
        }
        m.insert("defaultRowHeight".into(), js_num(self.default_row_height));
        if let Some(w) = self.default_col_width {
            m.insert("defaultColWidth".into(), js_num(w));
        }
        if let Some(f) = &self.format {
            m.insert("format".into(), f.to_json());
        }
        m.insert("view".into(), self.view.to_json());
        m.insert(
            "columns".into(),
            Value::Array(self.columns.iter().map(ColumnInfo::to_json).collect()),
        );
        m.insert(
            "merges".into(),
            Value::Array(self.merges.iter().map(CellRange::to_json).collect()),
        );
        let mut row_info = Map::new();
        for (k, v) in &self.row_info {
            row_info.insert(k.to_string(), v.to_json());
        }
        m.insert("rowInfo".into(), Value::Object(row_info));
        m.insert(
            "cells".into(),
            Value::Array(self.cells.iter().map(DumpCell::to_json).collect()),
        );
        Value::Object(m)
    }
}

/// Stage-3 workbook slice (pre-worksheet parse).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkbookSlice {
    pub format: String,
    pub date1904: bool,
    pub active_sheet: u32,
    pub sheets: Vec<SheetStub>,
    pub defined_names: Vec<DefinedName>,
    pub theme: ThemePalette,
    pub styles: Vec<CellStyle>,
    pub named_styles: Vec<NamedStyle>,
    pub xf_map: Vec<u32>,
    pub palette: Option<Vec<String>>,
    pub dxfs: Vec<DifferentialStyle>,
    pub shared_strings: Vec<SharedString>,
    pub warnings: Vec<String>,
    pub full_calc_on_load: Option<bool>,
    pub structure_locked: Option<bool>,
    pub calc_mode: Option<String>,
    pub iterate: Option<(f64, f64)>,
    pub workbook_password_hash: Option<String>,
    pub workbook_modern_hash: Option<ModernPasswordHash>,
    pub active_worksheet: Option<ActiveWorksheet>,
}

impl WorkbookSlice {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("format".into(), Value::String(self.format.clone()));
        m.insert("date1904".into(), Value::Bool(self.date1904));
        m.insert("activeSheet".into(), json!(self.active_sheet));
        m.insert(
            "sheets".into(),
            Value::Array(self.sheets.iter().map(SheetStub::to_json).collect()),
        );
        m.insert(
            "definedNames".into(),
            Value::Array(
                self.defined_names
                    .iter()
                    .map(DefinedName::to_json)
                    .collect(),
            ),
        );
        m.insert("theme".into(), self.theme.to_json());
        m.insert(
            "styles".into(),
            Value::Array(self.styles.iter().map(CellStyle::to_json).collect()),
        );
        m.insert(
            "namedStyles".into(),
            Value::Array(self.named_styles.iter().map(NamedStyle::to_json).collect()),
        );
        m.insert(
            "xfMap".into(),
            Value::Array(self.xf_map.iter().map(|n| json!(n)).collect()),
        );
        if let Some(p) = &self.palette {
            m.insert(
                "palette".into(),
                Value::Array(p.iter().map(|s| Value::String(s.clone())).collect()),
            );
        }
        m.insert(
            "dxfs".into(),
            Value::Array(self.dxfs.iter().map(DifferentialStyle::to_json).collect()),
        );
        m.insert(
            "sharedStrings".into(),
            Value::Array(
                self.shared_strings
                    .iter()
                    .map(SharedString::to_json)
                    .collect(),
            ),
        );
        m.insert(
            "warnings".into(),
            Value::Array(
                self.warnings
                    .iter()
                    .map(|w| Value::String(w.clone()))
                    .collect(),
            ),
        );
        if self.full_calc_on_load == Some(true) {
            m.insert("fullCalcOnLoad".into(), Value::Bool(true));
        }
        if self.structure_locked == Some(true) {
            m.insert("structureLocked".into(), Value::Bool(true));
        }
        if let Some(mode) = &self.calc_mode {
            m.insert("calcMode".into(), Value::String(mode.clone()));
        }
        if let Some((count, delta)) = self.iterate {
            let mut it = Map::new();
            it.insert("count".into(), js_num(count));
            it.insert("delta".into(), js_num(delta));
            m.insert("iterate".into(), Value::Object(it));
        }
        if let Some(h) = &self.workbook_password_hash {
            m.insert("workbookPasswordHash".into(), Value::String(h.clone()));
        }
        if let Some(h) = &self.workbook_modern_hash {
            m.insert("workbookModernHash".into(), h.to_json());
        }
        if let Some(ws) = &self.active_worksheet {
            m.insert("activeWorksheet".into(), ws.to_json());
        }
        Value::Object(m)
    }
}
