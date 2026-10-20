// SPDX-License-Identifier: MIT
//! Stream one worksheet part and keep only the viewport's cells.
//!
//! The shell (views, columns, format, dimension) is split off the front of the part. `<sheetData>`
//! rows are visited in order; cell records are kept for the viewport plus slop (and frozen panes).
//! When row numbers are monotonic past the window and `<dimension>` is trustworthy:
//! - **Early stop** (`allow_early_stop`): inflate stops at the viewport (O(viewport) first open).
//! - **SeekTail** (`allow_early_stop = false` with dimension): byte-scan to `</sheetData>` without
//!   row/cell tokenizing so merges / CF / DV land — used by [`super::FirstView::ensure_sheet_complete`].
//!
//! Without a trustworthy dimension, `SkipRows` walks remaining rows for extents and sparse heights.

use std::collections::{BTreeMap, HashMap, HashSet};

use nitro_xml_opc::opc::Package;
use nitro_xml_opc::xml::parse_xml_str;

use crate::address::{parse_address, parse_range, MAX_COL, MAX_ROW};
use crate::cells::CellStore;
use crate::cf::read_conditional_formats;
use crate::comments::load_comment_addresses;
use crate::drawing::load_sheet_drawings;
use crate::hyperlinks::read_hyperlinks;
use crate::model::{
    ActiveWorksheet, CellAddress, CellRange, DifferentialStyle, RowInfo, SheetState,
};
use crate::sheet_data::{read_sheet_data_xml, store_to_dump_cells_with_sst, CellContext};
use crate::sheet_props::{
    read_columns, read_data_validations, read_row_info, read_sheet_format, read_sheet_view,
};
use crate::sparklines::{extract_ext_lst, read_sparkline_groups_from_ext};
use crate::xml_util::{att, elements, x_children};

use super::sst::{collect_shared_indices, resolve_shared_strings, rewrite_shared_indices};
use super::ViewportRect;

pub(crate) const ROW_SLOP: u32 = 8;
pub(crate) const COL_SLOP: u32 = 4;
/// Rows added past a truncated stream so grid metrics are not the loaded window alone.
pub(crate) const EXTENT_STUB_ROWS: u32 = 256;
pub(crate) const EXTENT_STUB_COLS: u32 = 8;

pub(crate) struct SheetRequest<'a> {
    pub part_name: &'a str,
    pub view: ViewportRect,
    pub xf_map: &'a [u32],
    pub date1904: bool,
    pub palette: Option<&'a [String]>,
    pub dxfs: &'a [DifferentialStyle],
    pub dynamic_cells: &'a HashSet<u32>,
    pub strings_part: Option<&'a str>,
    pub name: &'a str,
    pub sheet_id: u32,
    pub state: SheetState,
    /// When true (default), a trustworthy `<dimension>` stops inflate at the viewport.
    /// When false with a trustworthy dimension, byte-seek to `</sheetData>` (SeekTail) so the
    /// post-sheetData shell lands without SkipRows tokenizing. When dimension is missing/wrong,
    /// false still uses SkipRows.
    pub allow_early_stop: bool,
}

/// `(sheet, rows_visited, truncated)` — `truncated` means inflate stopped before `</sheetData>`.
pub(crate) fn load_sheet_window(
    pkg: &mut Package,
    req: &SheetRequest<'_>,
    resolved: &mut crate::fast_hash::FastSet<usize>,
    warnings: &mut Vec<String>,
) -> Result<(ActiveWorksheet, u32, bool), String> {
    let mut parser = SheetParser::new(req.view, req.xf_map.to_vec(), req.allow_early_stop);
    let streamed = pkg
        .stream_part(req.part_name, |chunk| {
            parser.push_bytes(chunk);
            if let Some(prefix) = parser.prefix_for_freeze() {
                let (rows, cols) = freeze_from_prefix(&prefix);
                parser.apply_freeze(rows, cols);
            }
            parser.wants_more()
        })
        .map_err(|e| e.to_string())?;
    if streamed.is_none() {
        warnings.push(format!(
            "Sheet \"{}\" has no part {}; it was loaded empty.",
            req.name, req.part_name
        ));
        return Ok((empty_sheet(req), 0, false));
    }
    parser.finish();
    let capture = parser.into_capture();
    let truncated = capture.truncated;

    let mut needed = crate::fast_hash::FastSet::default();
    for row in &capture.kept {
        collect_shared_indices(&row.xml, &mut needed);
    }
    let resolved_now = if let Some(part) = req.strings_part {
        resolve_shared_strings(pkg, part, &needed, req.palette).map_err(|e| e.to_string())?
    } else {
        Default::default()
    };
    for idx in resolved_now.keys() {
        resolved.insert(*idx);
    }
    let (compact, remap) = compact_strings(resolved_now);

    let mut body = String::new();
    for row in &capture.kept {
        body.push_str(&rewrite_shared_indices(&row.xml, &remap));
    }
    let mut store = CellStore::new();
    let mut parsed_info: HashMap<u32, RowInfo> = HashMap::new();
    let mut cell_warnings = Vec::new();
    {
        let mut ctx = CellContext {
            shared_strings: &compact,
            xf_map: req.xf_map,
            date1904: req.date1904,
            palette: req.palette,
            dynamic_cells: req.dynamic_cells,
            warnings: &mut cell_warnings,
        };
        if !body.is_empty() {
            read_sheet_data_xml(&mut ctx, &body, &mut store, &mut parsed_info);
        }
    }
    for w in cell_warnings {
        push_warn(warnings, w);
    }

    // Keep every height/hidden override from the stream. Truncating to the cell
    // window drops `ht` below the fold and skews `topLeftCell` reveal scroll
    // (e.g. poi-45540_classic_Footer A91 was 4px short of upstream).
    let mut row_info = capture.row_info;
    for (k, v) in parsed_info {
        row_info.insert(k, v);
    }

    let shell = shell_document(&capture.prefix, &capture.tail, capture.truncated);
    let mut sheet = worksheet_from_shell(pkg, req, &shell, warnings);
    let cells = store_to_dump_cells_with_sst(&store, &compact);
    let freeze_rows = sheet.view.freeze.as_ref().map(|f| f.rows).unwrap_or(0);
    let freeze_cols = sheet.view.freeze.as_ref().map(|f| f.cols).unwrap_or(0);
    sheet.cells = cells
        .into_iter()
        .filter(|c| keep_cell(c.row, c.col, req.view, freeze_rows, freeze_cols))
        .collect();
    sheet.row_info = row_info;
    let max_row = if capture.saw_row { capture.max_row } else { 0 };
    let max_col = capture.max_col;
    sheet.dimension = Some(grid_extent(
        dimension_ref(&shell),
        max_row,
        max_col,
        capture.truncated,
    ));
    Ok((sheet, capture.rows_visited, truncated))
}

fn empty_sheet(req: &SheetRequest<'_>) -> ActiveWorksheet {
    ActiveWorksheet {
        name: req.name.to_owned(),
        sheet_id: req.sheet_id,
        state: req.state,
        part_name: Some(req.part_name.to_owned()),
        default_row_height: 15.0,
        default_col_width: None,
        format: None,
        view: crate::sheet_props::default_sheet_view(),
        columns: Vec::new(),
        merges: Vec::new(),
        row_info: BTreeMap::new(),
        cell_store: None,
        cells: Vec::new(),
        data_validations: Vec::new(),
        comment_addresses: Vec::new(),
        hyperlinks: Vec::new(),
        ext_lst: Vec::new(),
        sparkline_groups: Vec::new(),
        drawings: Vec::new(),
        conditional_formats: Vec::new(),
        dimension: Some(grid_extent(None, 0, 0, false)),
    }
}

fn compact_strings(
    resolved: crate::fast_hash::FastMap<usize, crate::model::SharedString>,
) -> (
    Vec<crate::model::SharedString>,
    crate::fast_hash::FastMap<usize, usize>,
) {
    let mut keys: Vec<usize> = resolved.keys().copied().collect();
    keys.sort_unstable();
    let mut compact = Vec::with_capacity(keys.len());
    let mut remap =
        crate::fast_hash::FastMap::with_capacity_and_hasher(keys.len(), Default::default());
    for (new_idx, old_idx) in keys.into_iter().enumerate() {
        if let Some(entry) = resolved.get(&old_idx) {
            remap.insert(old_idx, new_idx);
            compact.push(entry.clone());
        }
    }
    (compact, remap)
}

fn keep_cell(row: u32, col: u32, view: ViewportRect, freeze_rows: u32, freeze_cols: u32) -> bool {
    let r0 = view.r0.saturating_sub(ROW_SLOP);
    let r1 = view.r1.saturating_add(ROW_SLOP);
    let c0 = view.c0.saturating_sub(COL_SLOP);
    let c1 = view.c1.saturating_add(COL_SLOP);
    let in_r = row >= r0 && row <= r1;
    let in_c = col >= c0 && col <= c1;
    (in_r && in_c) || (row < freeze_rows && in_c) || (col < freeze_cols && in_r)
}

fn grid_extent(
    dimension: Option<CellRange>,
    max_row: u32,
    max_col: u32,
    truncated: bool,
) -> CellRange {
    if let Some(dim) = dimension {
        return dim;
    }
    let (extra_r, extra_c) = if truncated {
        (EXTENT_STUB_ROWS, EXTENT_STUB_COLS)
    } else {
        (0, 0)
    };
    CellRange {
        start: CellAddress { row: 0, col: 0 },
        end: CellAddress {
            row: max_row.saturating_add(extra_r).min(MAX_ROW),
            col: max_col.saturating_add(extra_c).min(MAX_COL),
        },
    }
}

fn dimension_ref(shell_xml: &str) -> Option<CellRange> {
    // Mid-stream prefix ends at `<sheetData…>`; a finished shell is already closed.
    let for_parse = if shell_xml.contains("</worksheet") {
        shell_xml.to_owned()
    } else {
        shell_document(shell_xml, "", true)
    };
    let doc = parse_xml_str(&for_parse).ok()?;
    let node = elements(&doc.root).find(|e| e.local == "dimension")?;
    att(Some(node), "ref").and_then(parse_range)
}

/// Whether `<dimension>` is safe to trust for a byte-level seek past the viewport (no SkipRows).
fn dimension_trustworthy(
    dimension: Option<&CellRange>,
    view: &ViewportRect,
    capture: &Capture,
) -> bool {
    let Some(dim) = dimension else {
        return false;
    };
    if dim.end.row < dim.start.row || dim.end.col < dim.start.col {
        return false;
    }
    let limit = view.r1.saturating_add(ROW_SLOP);
    // Must cover the viewport we are about to leave behind.
    if dim.end.row < limit {
        return false;
    }
    // Observed cells past the declared end → stale or wrong dimension; fall back.
    if capture.saw_row && capture.max_row > dim.end.row {
        return false;
    }
    if capture.max_col > dim.end.col {
        return false;
    }
    true
}

fn shell_document(prefix: &str, tail: &str, _truncated: bool) -> String {
    let mut xml = if prefix_has_sheet_data(prefix) {
        if tail.is_empty() {
            format!("{prefix}</sheetData>")
        } else {
            format!("{prefix}{tail}")
        }
    } else {
        prefix.to_owned()
    };
    if xml.contains("<worksheet") && !xml.contains("</worksheet") {
        xml.push_str("</worksheet>");
    }
    xml
}

fn prefix_has_sheet_data(prefix: &str) -> bool {
    prefix.contains("<sheetData") || prefix.contains(":sheetData")
}

fn worksheet_from_shell(
    pkg: &mut Package,
    req: &SheetRequest<'_>,
    shell_xml: &str,
    _warnings: &mut Vec<String>,
) -> ActiveWorksheet {
    let doc = parse_xml_str(shell_xml).ok();
    let root = doc.as_ref().map(|d| &d.root);
    let mut by_name = HashMap::new();
    if let Some(root) = root {
        for node in elements(root) {
            if !by_name.contains_key(&node.local) {
                by_name.insert(node.local.clone(), node);
            }
        }
    }
    let view = read_sheet_view(by_name.get("sheetViews").copied());
    let (default_row_height, default_col_width, format) =
        read_sheet_format(by_name.get("sheetFormatPr").copied());
    let columns = read_columns(by_name.get("cols").copied(), req.xf_map);
    let merges = root
        .map(|root| {
            let merges_parent = by_name.get("mergeCells").copied().unwrap_or(root);
            x_children(merges_parent, "mergeCell")
                .into_iter()
                .filter_map(|node| att(Some(node), "ref").and_then(parse_range))
                .collect()
        })
        .unwrap_or_default();
    let data_validations = read_data_validations(by_name.get("dataValidations").copied());
    let ext_lst = extract_ext_lst(shell_xml);
    let sparkline_groups = by_name
        .get("extLst")
        .map(|e| read_sparkline_groups_from_ext(e))
        .unwrap_or_default();
    let comment_addresses = load_comment_addresses(pkg, req.part_name);
    let hyperlinks = read_hyperlinks(pkg, req.part_name, by_name.get("hyperlinks").copied());
    let mut warn = |m: String| {
        if !_warnings.iter().any(|w| w == &m) {
            _warnings.push(m);
        }
    };
    let drawings = load_sheet_drawings(pkg, req.part_name, root, &mut warn);
    let cf_blocks: Vec<_> = root
        .map(|r| {
            elements(r)
                .filter(|e| e.local == "conditionalFormatting")
                .collect()
        })
        .unwrap_or_default();
    let mut conditional_formats =
        read_conditional_formats(&cf_blocks, req.dxfs, req.palette, &mut warn);
    crate::cf::link_data_bar_extensions(&mut conditional_formats, &ext_lst);
    ActiveWorksheet {
        name: req.name.to_owned(),
        sheet_id: req.sheet_id,
        state: req.state,
        part_name: Some(req.part_name.to_owned()),
        default_row_height,
        default_col_width,
        format,
        view,
        columns,
        merges,
        row_info: BTreeMap::new(),
        cell_store: None,
        cells: Vec::new(),
        data_validations,
        comment_addresses,
        hyperlinks,
        ext_lst,
        sparkline_groups,
        drawings,
        conditional_formats,
        dimension: None,
    }
}

fn freeze_from_prefix(prefix: &str) -> (u32, u32) {
    let shell = shell_document(prefix, "", true);
    let Ok(doc) = parse_xml_str(&shell) else {
        return (0, 0);
    };
    let node = elements(&doc.root).find(|e| e.local == "sheetViews");
    let view = read_sheet_view(node);
    view.freeze.map(|f| (f.rows, f.cols)).unwrap_or((0, 0))
}

fn push_warn(warnings: &mut Vec<String>, message: String) {
    if !warnings.contains(&message) {
        warnings.push(message);
    }
}

struct KeptRow {
    row: u32,
    xml: String,
}

struct Capture {
    prefix: String,
    tail: String,
    truncated: bool,
    rows_visited: u32,
    max_row: u32,
    max_col: u32,
    saw_row: bool,
    kept: Vec<KeptRow>,
    row_info: BTreeMap<u32, RowInfo>,
}

struct SheetParser {
    view: ViewportRect,
    xf_map: Vec<u32>,
    text: String,
    pending: Vec<u8>,
    phase: Phase,
    capture: Capture,
    pending_rows: Vec<KeptRow>,
    row_fallback: u32,
    last_row: Option<u32>,
    monotonic: bool,
    freeze_rows: u32,
    stop: bool,
    prefix_ready: bool,
    asked_freeze: bool,
    freeze_applied: bool,
    bom_checked: bool,
    /// From `<dimension ref>` in the prefix when present and later validated.
    dimension: Option<CellRange>,
    allow_early_stop: bool,
}

enum Phase {
    Prefix,
    Rows,
    /// Past the viewport: discard row XML but keep scanning for extents + `</sheetData>`.
    SkipRows,
    /// Past the viewport with a trustworthy `<dimension>`: discard bytes until `</sheetData>`
    /// without parsing `<row>` / `<c>` (still captures the post-sheetData tail).
    SeekTail,
    Tail,
}

impl SheetParser {
    fn new(view: ViewportRect, xf_map: Vec<u32>, allow_early_stop: bool) -> Self {
        Self {
            view,
            xf_map,
            text: String::new(),
            pending: Vec::new(),
            phase: Phase::Prefix,
            capture: Capture {
                prefix: String::new(),
                tail: String::new(),
                truncated: false,
                rows_visited: 0,
                max_row: 0,
                max_col: 0,
                saw_row: false,
                kept: Vec::new(),
                row_info: BTreeMap::new(),
            },
            pending_rows: Vec::new(),
            row_fallback: 0,
            last_row: None,
            monotonic: true,
            freeze_rows: 0,
            stop: false,
            prefix_ready: false,
            asked_freeze: false,
            freeze_applied: false,
            bom_checked: false,
            dimension: None,
            allow_early_stop,
        }
    }

    fn push_bytes(&mut self, chunk: &[u8]) {
        if self.stop {
            return;
        }
        self.pending.extend_from_slice(chunk);
        let valid = match std::str::from_utf8(&self.pending) {
            Ok(s) => s.len(),
            Err(e) => e.valid_up_to(),
        };
        if valid > 0 {
            let s = std::str::from_utf8(&self.pending[..valid]).unwrap_or("");
            self.text.push_str(s);
            self.pending.drain(..valid);
        }
        self.strip_bom();
        self.pump();
    }

    fn finish(&mut self) {
        if !self.pending.is_empty() {
            let lossy = String::from_utf8_lossy(&self.pending);
            self.text.push_str(&lossy);
            self.pending.clear();
            self.strip_bom();
            if !self.stop {
                self.pump();
            }
        }
        if !self.freeze_applied {
            self.apply_freeze(0, 0);
        }
        if self.stop {
            self.capture.truncated = true;
            return;
        }
        match self.phase {
            Phase::Tail => {
                self.capture.tail.push_str(&self.text);
                self.text.clear();
                // Cells may still be window-only; extents + merges came from a full sheetData scan.
                self.capture.truncated = false;
            }
            Phase::Rows | Phase::SkipRows | Phase::SeekTail => {
                if let Some(at) = find_sheet_data_close(&self.text) {
                    self.capture.tail = self.text[at..].to_owned();
                    self.capture.truncated = false;
                } else {
                    // Never reached `</sheetData>` — extents may be incomplete.
                    self.capture.truncated = true;
                }
            }
            Phase::Prefix => {
                if self.capture.prefix.is_empty() {
                    self.capture.prefix = self.text.clone();
                }
                self.capture.truncated = false;
            }
        }
    }

    fn into_capture(self) -> Capture {
        self.capture
    }

    fn wants_more(&self) -> bool {
        !self.stop
    }

    fn prefix_for_freeze(&mut self) -> Option<String> {
        if self.prefix_ready && !self.asked_freeze {
            self.asked_freeze = true;
            Some(self.capture.prefix.clone())
        } else {
            None
        }
    }

    fn apply_freeze(&mut self, rows: u32, cols: u32) {
        self.freeze_rows = rows;
        let _ = cols;
        self.freeze_applied = true;
        let pending = std::mem::take(&mut self.pending_rows);
        for row in pending {
            if self.should_keep_row(row.row) {
                self.capture.kept.push(row);
            }
        }
    }

    fn should_keep_row(&self, row: u32) -> bool {
        let r0 = self.view.r0.saturating_sub(ROW_SLOP);
        let r1 = self.view.r1.saturating_add(ROW_SLOP);
        (row >= r0 && row <= r1) || row < self.freeze_rows
    }

    fn strip_bom(&mut self) {
        if self.bom_checked || self.text.is_empty() {
            return;
        }
        self.bom_checked = true;
        if self.text.starts_with('\u{feff}') {
            self.text.replace_range(..'\u{feff}'.len_utf8(), "");
        }
    }

    fn pump(&mut self) {
        loop {
            if self.stop {
                return;
            }
            match self.phase {
                Phase::Prefix => match find_sheet_data_open(&self.text) {
                    OpenScan::NeedMore => return,
                    OpenScan::Open { end, self_close } => {
                        self.capture.prefix = self.text[..end].to_owned();
                        self.text.drain(..end);
                        self.dimension = dimension_ref(&self.capture.prefix);
                        self.prefix_ready = true;
                        self.phase = if self_close { Phase::Tail } else { Phase::Rows };
                    }
                },
                Phase::Rows => match next_row(&self.text) {
                    RowScan::NeedMore => return,
                    RowScan::End { at } => {
                        self.capture.tail = self.text[at..].to_owned();
                        self.text.clear();
                        self.phase = Phase::Tail;
                    }
                    RowScan::Row { open_end, end } => {
                        let open = &self.text[..open_end];
                        let row0 = parse_row_r(open).unwrap_or(self.row_fallback);
                        if let Some(prev) = self.last_row {
                            if row0 < prev {
                                self.monotonic = false;
                            }
                        }
                        let limit = self.view.r1.saturating_add(ROW_SLOP);
                        if self.monotonic && row0 > limit {
                            let dim_ok = dimension_trustworthy(
                                self.dimension.as_ref(),
                                &self.view,
                                &self.capture,
                            );
                            if dim_ok && self.allow_early_stop {
                                // O(viewport) first open; caller SeekTails via ensure_sheet_complete.
                                self.stop = true;
                                self.capture.truncated = true;
                                return;
                            }
                            if dim_ok {
                                // Complete shell without tokenizing every remaining row.
                                self.phase = Phase::SeekTail;
                                continue;
                            }
                            // No trustworthy dimension: SkipRows for extents + sparse row heights.
                            self.phase = Phase::SkipRows;
                            continue;
                        }
                        let xml = self.text[..end].to_owned();
                        self.text.drain(..end);
                        self.consume_row(row0, xml);
                    }
                },
                Phase::SkipRows => match next_row(&self.text) {
                    RowScan::NeedMore => return,
                    RowScan::End { at } => {
                        self.capture.tail = self.text[at..].to_owned();
                        self.text.clear();
                        self.phase = Phase::Tail;
                    }
                    RowScan::Row { open_end, end } => {
                        let open = &self.text[..open_end];
                        let row0 = parse_row_r(open).unwrap_or(self.row_fallback);
                        let xml = &self.text[..end];
                        self.capture.max_row = self.capture.max_row.max(row0);
                        self.capture.saw_row = true;
                        note_cols(xml, &mut self.capture.max_col);
                        // Sparse height/hidden flags still feed grid metrics when scrolled.
                        if let Some(info) = row_info_from_xml(xml, &self.xf_map) {
                            self.capture.row_info.insert(row0, info);
                        }
                        self.last_row = Some(row0);
                        self.row_fallback = row0.saturating_add(1);
                        self.text.drain(..end);
                    }
                },
                Phase::SeekTail => {
                    if let Some(at) = find_sheet_data_close(&self.text) {
                        self.capture.tail = self.text[at..].to_owned();
                        self.text.clear();
                        self.phase = Phase::Tail;
                        self.capture.truncated = false;
                    } else {
                        // Keep a suffix so a close tag split across inflate chunks is still found.
                        const KEEP: usize = 64;
                        if self.text.len() > KEEP {
                            let mut drop_to = self.text.len() - KEEP;
                            while drop_to > 0 && !self.text.is_char_boundary(drop_to) {
                                drop_to -= 1;
                            }
                            self.text.drain(..drop_to);
                        }
                        return;
                    }
                }
                Phase::Tail => return,
            }
        }
    }

    fn consume_row(&mut self, row0: u32, xml: String) {
        self.capture.rows_visited = self.capture.rows_visited.saturating_add(1);
        self.capture.max_row = self.capture.max_row.max(row0);
        self.capture.saw_row = true;
        note_cols(&xml, &mut self.capture.max_col);
        self.last_row = Some(row0);
        self.row_fallback = row0.saturating_add(1);
        if let Some(info) = row_info_from_xml(&xml, &self.xf_map) {
            self.capture.row_info.insert(row0, info);
        }
        let kept = KeptRow { row: row0, xml };
        if self.freeze_applied {
            if self.should_keep_row(row0) {
                self.capture.kept.push(kept);
            }
        } else {
            self.pending_rows.push(kept);
        }
    }
}

enum OpenScan {
    NeedMore,
    Open { end: usize, self_close: bool },
}

enum RowScan {
    NeedMore,
    End { at: usize },
    Row { open_end: usize, end: usize },
}

fn find_sheet_data_open(text: &str) -> OpenScan {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if end_of_tag(text, i).is_none() {
                return OpenScan::NeedMore;
            }
            if let TagKind::Open(local) = tag_kind(text, i) {
                if local == "sheetData" {
                    if let Some((end, self_close)) = end_of_tag(text, i) {
                        return OpenScan::Open { end, self_close };
                    }
                    return OpenScan::NeedMore;
                }
            }
        }
        i += 1;
    }
    OpenScan::NeedMore
}

fn next_row(text: &str) -> RowScan {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match memchr::memchr(b'<', &bytes[i..]) {
            Some(off) => i += off,
            None => break,
        }
        if i + 1 >= bytes.len() {
            return RowScan::NeedMore;
        }
        match tag_kind(text, i) {
            TagKind::Incomplete => return RowScan::NeedMore,
            TagKind::Close("sheetData") => return RowScan::End { at: i },
            TagKind::Open("row") => {
                let Some((open_end, self_close)) = end_of_tag(text, i) else {
                    return RowScan::NeedMore;
                };
                if self_close {
                    return RowScan::Row {
                        open_end,
                        end: open_end,
                    };
                }
                let Some(end) = find_close(text, open_end, "row") else {
                    return RowScan::NeedMore;
                };
                return RowScan::Row { open_end, end };
            }
            TagKind::Open(_) | TagKind::Close(_) | TagKind::Other => {
                if end_of_tag(text, i).is_none() {
                    return RowScan::NeedMore;
                }
            }
        }
        i += 1;
    }
    RowScan::NeedMore
}

fn find_sheet_data_close(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(off) = memchr::memchr(b'<', &bytes[i..]) {
        i += off;
        if let TagKind::Close(local) = tag_kind(text, i) {
            if local == "sheetData" {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn parse_row_r(open_tag: &str) -> Option<u32> {
    let bytes = open_tag.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b'r'
            && bytes[i + 1] == b'='
            && (bytes[i + 2] == b'"' || bytes[i + 2] == b'\'')
        {
            let q = bytes[i + 2] as char;
            let start = i + 3;
            let end = open_tag[start..].find(q)?;
            let token = &open_tag[start..start + end];
            if let Ok(n) = token.parse::<u32>() {
                if n > 0 {
                    return Some(n - 1);
                }
            }
        }
        i += 1;
    }
    None
}

fn note_cols(xml: &str, max_col: &mut u32) {
    let bytes = xml.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        let Some(off) = memchr::memmem::find(&bytes[i..], b"r=") else {
            break;
        };
        i += off;
        if i + 3 < bytes.len() && (bytes[i + 2] == b'"' || bytes[i + 2] == b'\'') {
            let q = bytes[i + 2];
            let start = i + 3;
            if let Some(end) = memchr::memchr(q, &bytes[start..]) {
                let token = &xml[start..start + end];
                if let Some(addr) = parse_address(token) {
                    *max_col = (*max_col).max(addr.col);
                }
                i = start + end + 1;
                continue;
            }
        }
        i += 1;
    }
}

fn row_info_from_xml(xml: &str, xf_map: &[u32]) -> Option<RowInfo> {
    let open_end = end_of_tag(xml, 0)?.0;
    let open = &xml[..open_end];
    if !open.contains("ht=")
        && !open.contains("hidden=")
        && !open.contains("customFormat=")
        && !open.contains("outlineLevel=")
        && !open.contains("collapsed=")
        && !open.contains(" s=")
    {
        return None;
    }
    let inner = if open.ends_with("/>") {
        open.to_owned()
    } else {
        format!("{open}</row>")
    };
    let wrapped = format!(
        "<sheetData xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">{inner}</sheetData>"
    );
    let doc = parse_xml_str(&wrapped).ok()?;
    let row = x_children(&doc.root, "row").into_iter().next()?;
    read_row_info(row, xf_map)
}

enum TagKind<'a> {
    Incomplete,
    Open(&'a str),
    Close(&'a str),
    Other,
}

fn tag_kind(s: &str, lt: usize) -> TagKind<'_> {
    let bytes = s.as_bytes();
    if lt >= bytes.len() || bytes[lt] != b'<' {
        return TagKind::Other;
    }
    let mut j = lt + 1;
    if j >= bytes.len() {
        return TagKind::Incomplete;
    }
    let is_close = if bytes[j] == b'/' {
        j += 1;
        true
    } else if bytes[j] == b'!' || bytes[j] == b'?' {
        return TagKind::Other;
    } else {
        false
    };
    if j >= bytes.len() {
        return TagKind::Incomplete;
    }
    let mut local_start = j;
    let mut k = j;
    while k < bytes.len() && is_name_byte(bytes[k]) {
        k += 1;
    }
    if k < bytes.len() && bytes[k] == b':' {
        local_start = k + 1;
        k = local_start;
        while k < bytes.len() && is_name_byte(bytes[k]) {
            k += 1;
        }
    }
    if k == local_start {
        return if k >= bytes.len() {
            TagKind::Incomplete
        } else {
            TagKind::Other
        };
    }
    if k >= bytes.len() {
        return TagKind::Incomplete;
    }
    let local = &s[local_start..k];
    if is_close {
        TagKind::Close(local)
    } else {
        TagKind::Open(local)
    }
}

fn is_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

fn end_of_tag(s: &str, lt: usize) -> Option<(usize, bool)> {
    let bytes = s.as_bytes();
    let mut i = lt + 1;
    let mut quote = 0u8;
    while i < bytes.len() {
        let c = bytes[i];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            let self_close = i > lt && bytes[i - 1] == b'/';
            return Some((i + 1, self_close));
        }
        i += 1;
    }
    None
}

fn find_close(s: &str, from: usize, local: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = from;
    while let Some(off) = memchr::memchr(b'<', &bytes[i..]) {
        i += off;
        match tag_kind(s, i) {
            TagKind::Close(name) if name == local => {
                return end_of_tag(s, i).map(|(end, _)| end);
            }
            TagKind::Incomplete => return None,
            _ => {}
        }
        i += 1;
    }
    None
}
