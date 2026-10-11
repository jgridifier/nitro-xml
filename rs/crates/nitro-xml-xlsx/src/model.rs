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

#[derive(Debug, Clone, PartialEq, Eq)]
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
        Value::Object(m)
    }
}
