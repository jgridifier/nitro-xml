// SPDX-License-Identifier: MIT
//! Xlsx viewer state and engine API (#19).

use crate::events::Event;
use crate::transport::StringTable;
use nitro_xml_xlsx::model::ActiveWorksheet;
use nitro_xml_xlsx::model::{CellRange, SheetSelection};
use nitro_xml_xlsx::{
    cell_text_tsv, format_range, hyperlink_at, hyperlinks_in_sheet, load_workbook,
    paint_grid_frame, parse_range, parse_worksheet_at, search_workbook, visible_sheets_json,
    GridPaintViewport, SearchOptions, WorkbookSlice,
};
use serde_json::Value;
const DEFAULT_W: i32 = 800;
const DEFAULT_H: i32 = 600;

pub struct XlsxState {
    pub workbook: WorkbookSlice,
    pub sheets: Vec<Option<ActiveWorksheet>>,
    pub active: usize,
    pub scroll_x: i32,
    pub scroll_y: i32,
    pub viewport_w: i32,
    pub viewport_h: i32,
    /// Shell-driven zoom percent; `None` uses sheet view zoom.
    pub zoom_pct: Option<f64>,
}

impl XlsxState {
    pub fn open(bytes: &[u8]) -> Result<(Self, Vec<String>), String> {
        let workbook = load_workbook(bytes)?;
        let warnings = workbook.warnings.clone();
        let mut sheets: Vec<Option<ActiveWorksheet>> = vec![None; workbook.sheets.len()];
        let active = workbook.active_sheet as usize;
        if let Some(ws) = workbook.active_worksheet.clone() {
            if active < sheets.len() {
                sheets[active] = Some(ws);
            }
        }
        Ok((
            Self {
                workbook,
                sheets,
                active,
                scroll_x: 0,
                scroll_y: 0,
                viewport_w: DEFAULT_W,
                viewport_h: DEFAULT_H,
                zoom_pct: None,
            },
            warnings,
        ))
    }

    pub fn set_zoom_pct(&mut self, zoom_pct: f64) {
        self.zoom_pct = Some(zoom_pct);
    }

    /// Apply an A1 selection ref to the active sheet view (paints `.xg-outline`).
    /// Returns the normalized ref string (`A1` / `A1:B2`) for shell UI parity.
    pub fn apply_selection_ref(&mut self, range_ref: &str) -> Result<String, String> {
        let range = parse_range(range_ref).ok_or_else(|| format!("invalid range {range_ref}"))?;
        let sheet = self
            .sheets
            .get_mut(self.active)
            .and_then(|s| s.as_mut())
            .ok_or_else(|| "no active sheet".to_string())?;
        sheet.view.selection = Some(SheetSelection {
            active: range.start,
            ranges: vec![CellRange {
                start: range.start,
                end: range.end,
            }],
        });
        Ok(format_range(&range))
    }

    /// Rough scroll so `(row, col)` is near the top-left of the scrollable area.
    pub fn scroll_to_cell(&mut self, row: u32, col: u32) {
        // Approximate default metrics (scaled by zoom later in paint).
        let zoom = self.zoom_pct.unwrap_or(100.0) / 100.0;
        let col_w = (64.0 * zoom).round().max(1.0) as i32;
        let row_h = (20.0 * zoom).round().max(1.0) as i32;
        self.scroll_x = (col as i32).saturating_mul(col_w).max(0);
        self.scroll_y = (row as i32).saturating_mul(row_h).max(0);
    }

    fn ensure_sheet_parsed(&mut self, bytes: &[u8], index: usize) -> Result<(), String> {
        if index >= self.sheets.len() {
            return Err(format!("sheet index {} out of range", index));
        }
        if self.sheets[index].is_none() {
            let ws = parse_worksheet_at(bytes, &self.workbook, index)?;
            self.sheets[index] = Some(ws);
        }
        Ok(())
    }

    pub fn set_active_sheet(&mut self, bytes: &[u8], index: usize) -> Result<(), String> {
        if index >= self.workbook.sheets.len() {
            return Err(format!("sheet index {} out of range", index));
        }
        self.ensure_sheet_parsed(bytes, index)?;
        self.active = index;
        self.workbook.active_sheet = index as u32;
        Ok(())
    }

    pub fn viewport(&self) -> GridPaintViewport {
        let sheet = self.sheets[self.active].as_ref();
        let (frozen_rows, frozen_cols) = sheet
            .and_then(|s| s.view.freeze.as_ref())
            .map(|f| (f.rows, f.cols))
            .unwrap_or((0, 0));
        GridPaintViewport {
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
            width: self.viewport_w,
            height: self.viewport_h,
            frozen_rows,
            frozen_cols,
            zoom_pct: self.zoom_pct,
        }
    }

    pub fn paint(
        &mut self,
        bytes: &[u8],
        strings: &mut StringTable,
        commands: &mut nitro_xml_render::CommandBuffer,
        measures: &mut nitro_xml_render::MeasureBatch,
    ) {
        if self.ensure_sheet_parsed(bytes, self.active).is_err() {
            return;
        }
        let viewport = self.viewport();
        let active = self.active;
        let workbook = &self.workbook;
        let sheets_ref = &self.sheets;
        let sheet = sheets_ref[active].as_ref().expect("parsed");
        let mut intern = |s: &str| strings.intern(s);
        paint_grid_frame(
            commands,
            &mut intern,
            measures,
            workbook,
            sheet,
            active,
            sheets_ref,
            &viewport,
        );
    }

    pub fn process_events(&mut self, events: &[Event]) {
        for ev in events {
            match ev {
                Event::Resize { width, height, .. } => {
                    self.viewport_w = (*width).max(1.0) as i32;
                    self.viewport_h = (*height).max(1.0) as i32;
                }
                Event::Wheel { dx, dy, .. } => {
                    self.scroll_x = (self.scroll_x + *dx as i32).max(0);
                    self.scroll_y = (self.scroll_y + *dy as i32).max(0);
                }
                Event::Pointer { type_, .. } if *type_ == 1 => {
                    // pointer move — reserved for selection (#45)
                }
                _ => {}
            }
        }
    }

    pub fn sheets_json(&self) -> Value {
        visible_sheets_json(&self.workbook)
    }

    pub fn cell_text(
        &mut self,
        bytes: &[u8],
        sheet_index: usize,
        range: &str,
    ) -> Result<String, String> {
        self.ensure_sheet_parsed(bytes, sheet_index)?;
        let workbook = &self.workbook;
        let sheets = &self.sheets;
        let sheet = sheets[sheet_index].as_ref().expect("parsed");
        cell_text_tsv(workbook, sheet, sheet_index, sheets, range)
    }

    pub fn hyperlinks(&mut self, bytes: &[u8], sheet_index: usize) -> Result<Value, String> {
        self.ensure_sheet_parsed(bytes, sheet_index)?;
        let workbook = &self.workbook;
        let sheet = self.sheets[sheet_index].as_ref().expect("parsed");
        Ok(hyperlinks_in_sheet(workbook, sheet, sheet_index))
    }

    pub fn hyperlink_at(
        &mut self,
        bytes: &[u8],
        sheet_index: usize,
        row: u32,
        col: u32,
    ) -> Result<Option<Value>, String> {
        self.ensure_sheet_parsed(bytes, sheet_index)?;
        let workbook = &self.workbook;
        let sheet = self.sheets[sheet_index].as_ref().expect("parsed");
        Ok(hyperlink_at(workbook, sheet, sheet_index, row, col))
    }

    pub fn search(
        &mut self,
        bytes: &[u8],
        query: &str,
        opts: SearchOptions,
    ) -> Result<Value, String> {
        for i in 0..self.sheets.len() {
            if self.sheets[i].is_none()
                && self.workbook.sheets[i].state == nitro_xml_xlsx::model::SheetState::Visible
            {
                let _ = self.ensure_sheet_parsed(bytes, i);
            }
        }
        Ok(search_workbook(&self.workbook, &self.sheets, query, opts))
    }
}

pub fn is_xlsx_extension(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "xlsx" | "xlsm" | "xltx" | "xltm")
}

pub fn search_options_from_json(raw: &str) -> SearchOptions {
    let mut opts = SearchOptions::default();
    if raw.is_empty() {
        return opts;
    }
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        if let Some(b) = v.get("caseSensitive").and_then(|x| x.as_bool()) {
            opts.case_sensitive = b;
        }
        if let Some(b) = v.get("matchEntireCell").and_then(|x| x.as_bool()) {
            opts.match_entire = b;
        }
    }
    opts
}

pub fn process_frame(handle: &mut crate::handle::Handle) {
    let events: Vec<Event> = handle.events.pending_snapshot();
    if let Some(ref mut xlsx) = handle.xlsx {
        xlsx.process_events(&events);
    }
    handle.events.drain();
    if let Some(ref mut xlsx) = handle.xlsx {
        // Drop unacked measure reqs from a prior paint (e.g. openDocument) so this
        // frame's batch matches the commands about to be taken.
        handle.measures.clear_pending();
        let bytes = &handle.bytes;
        xlsx.paint(
            bytes,
            &mut handle.strings,
            &mut handle.commands,
            &mut handle.measures,
        );
    }
}
