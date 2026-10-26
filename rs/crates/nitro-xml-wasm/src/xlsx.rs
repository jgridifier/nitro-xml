// SPDX-License-Identifier: MIT
//! Xlsx viewer state and engine API (#19).

use crate::events::{Event, MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT};
use crate::transport::StringTable;
use nitro_xml_xlsx::editor::EditorSession;
use nitro_xml_xlsx::model::ActiveWorksheet;
use nitro_xml_xlsx::model::{CellAddress, CellRange, SheetSelection};
use nitro_xml_xlsx::render::{
    all_range, arrow_origin, clamped_hit, current_region_or_all, extend_selection, header_sizes,
    hit, moving_end, navigate_arrow, scroll_to_reveal, select_cell, selection_ref, Arrow, HitArea,
    HitInput,
};
use nitro_xml_xlsx::render::{EditOverlay, EditorPaintHints};
use nitro_xml_xlsx::view::metrics::{create_grid_metrics, GridMetrics, GridMetricsOptions};
use std::sync::Arc;

use nitro_xml_xlsx::{
    cell_text_tsv, hyperlink_at, hyperlinks_in_sheet, load_workbook_meta, open_first_view_arc,
    paint_grid_frame, paint_grid_frame_host, parse_all_worksheets, parse_range, parse_worksheet_at,
    search_workbook, visible_sheets_json, FirstView, GridPaintViewport, SearchOptions,
    ViewportRect, WorkbookSlice,
};
use serde_json::Value;
const DEFAULT_W: i32 = 800;
const DEFAULT_H: i32 = 600;

/// Fill-handle drag: `src` is the selection being extended, `to` the pending extension.
struct FillDrag {
    src: CellRange,
    to: Option<(&'static str, u32)>,
}

impl FillDrag {
    /// The block the fill will occupy (source plus extension).
    fn area(&self) -> CellRange {
        let mut r = self.src.clone();
        match self.to {
            Some(("down", n)) => r.end.row += n,
            Some(("up", n)) => r.start.row -= n.min(r.start.row),
            Some(("right", n)) => r.end.col += n,
            Some(("left", n)) => r.start.col -= n.min(r.start.col),
            _ => {}
        }
        r
    }
}

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
    /// Last cell visited while a primary-button cell drag is active.
    drag: Option<(u32, u32)>,
    /// In-flight fill-handle drag (#fill): the source block and the area it will cover.
    fill: Option<FillDrag>,
    /// Viewport-sized workbook. `None` after a search or hyperlink lookup upgrades to the full
    /// full materialise path (those need every cell, not the painted window).
    lazy: Option<FirstView>,
    /// Live edit session when the document is not read-only (#69).
    pub edit: Option<EditorSession>,
    /// Re-apply `topLeftCell` after the host's first real `Resize` (open uses a default
    /// 800×600 until the visual/loader reports the stage size).
    pending_view_scroll: bool,
    /// Defer `ensure_sheet_complete` until the next frame so first paint stays O(viewport).
    /// Set after the first paint when the active sheet is still truncated.
    pending_tail_complete: bool,
    /// Edit host owns the full grid; viewer `sheets` slots stay empty until a structure sync.
    pub(crate) grid_in_host: bool,
    /// `(origin col, row, col)` of the last Tab move. Enter returns to the origin column while
    /// the active cell is still where that Tab left it (Excel's type-across-a-row-then-Enter).
    pub(crate) tab_trail: Option<(u32, u32, u32)>,
}

impl XlsxState {
    /// Open via the shared package `Arc` so [`FirstView`] does not retain a second copy.
    pub fn open(bytes: Arc<[u8]>) -> Result<(Self, Vec<String>), String> {
        let mut first = open_first_view_arc(bytes, default_cell_rect())?;
        let warnings = first.workbook().warnings.clone();
        let workbook = first.workbook().clone();
        let mut sheets: Vec<Option<ActiveWorksheet>> = vec![None; workbook.sheets.len()];
        let active = workbook.active_sheet as usize;
        // `workbook.active_worksheet` stays `None`: nothing in the wasm path reads it (it only feeds
        // the dump/JSON oracle), and filling it cost a full second copy of the sheet.
        // When the first-view sheet is already complete, take it out of FirstView so open does
        // not hold two copies (#30 peak_mem). Truncated sheets keep a clone — `ensure_sheet`
        // still needs the lazy copy to preserve merges/CF across re-stream.
        if active < sheets.len() {
            sheets[active] = if first.is_sheet_complete(active) {
                first.take_sheet(active)
            } else {
                first.sheet(active).cloned()
            };
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
                drag: None,
                fill: None,
                lazy: Some(first),
                edit: None,
                // Apply after the host Resize so reveal uses the real stage size.
                pending_view_scroll: true,
                pending_tail_complete: false,
                grid_in_host: false,
                tab_trail: None,
            },
            warnings,
        ))
    }

    /// Mutable worksheet for navigation / selection (viewer slot or edit host when [`grid_in_host`]).
    pub(crate) fn sheet_mut(&mut self, index: usize) -> Option<&mut ActiveWorksheet> {
        if self.grid_in_host {
            return self
                .edit
                .as_mut()
                .and_then(|e| e.host_mut().sheets.get_mut(index));
        }
        self.sheets.get_mut(index)?.as_mut()
    }

    pub(crate) fn sheet_ref(&self, index: usize) -> Option<&ActiveWorksheet> {
        if self.grid_in_host {
            return self.edit.as_ref().and_then(|e| e.host().sheets.get(index));
        }
        self.sheets.get(index)?.as_ref()
    }

    /// True when the active sheet's post-`sheetData` shell (merges/CF/DV) is loaded.
    pub fn sheet_tail_complete(&self) -> bool {
        match &self.lazy {
            None => true,
            Some(lazy) => lazy.is_sheet_complete(self.active) && !self.pending_tail_complete,
        }
    }

    /// Run deferred `ensure_sheet_complete` once (next tick after first paint).
    /// Returns true when the shell was (re)loaded and a repaint is required.
    pub fn flush_pending_tail(&mut self) -> bool {
        if !self.pending_tail_complete {
            return false;
        }
        self.pending_tail_complete = false;
        match self.ensure_merges_ready() {
            Ok(()) => true,
            Err(_) => false,
        }
    }

    pub fn set_zoom_pct(&mut self, zoom_pct: f64) {
        self.zoom_pct = Some(zoom_pct);
    }

    /// Active sheet `zoomScale` (percent). Used to seed shell zoom on open/sheet switch
    /// so live paint matches upstream (`sheet.view.zoom` when no override).
    pub fn active_view_zoom(&self) -> f64 {
        self.sheets
            .get(self.active)
            .and_then(|s| s.as_ref())
            .map(|s| s.view.zoom)
            .filter(|z| z.is_finite() && *z > 0.0)
            .unwrap_or(100.0)
    }

    /// Apply an A1 selection ref to the active sheet view (paints `.xg-outline`).
    /// Returns the normalized ref string (`A1` / `A1:B2`) for shell UI parity.
    pub fn apply_selection_ref(&mut self, range_ref: &str) -> Result<String, String> {
        let _ = self.ensure_merges_ready();
        let range = parse_range(range_ref).ok_or_else(|| format!("invalid range {range_ref}"))?;
        let sheet = self
            .sheet_mut(self.active)
            .ok_or_else(|| "no active sheet".to_string())?;
        let selection = SheetSelection {
            active: range.start,
            anchor: range.start,
            ranges: vec![CellRange {
                start: range.start,
                end: range.end,
            }],
        };
        let normalized = selection_ref(&selection);
        sheet.view.selection = Some(selection);
        Ok(normalized)
    }

    /// Load post-`sheetData` merges/CF/DV when first-view stopped early on `<dimension>`.
    /// Called from selection and from the deferred post-first-paint flush — not from the
    /// first paint itself (that stays O(viewport) for WASM TTFV).
    fn ensure_merges_ready(&mut self) -> Result<(), String> {
        self.pending_tail_complete = false;
        let Some(lazy) = self.lazy.as_mut() else {
            return Ok(());
        };
        let active = self.active;
        if !lazy.ensure_sheet_complete(active)? {
            return Ok(());
        }
        let Some(ws) = lazy.sheet(active).cloned() else {
            return Ok(());
        };
        let kept_sel = self
            .sheets
            .get(active)
            .and_then(|s| s.as_ref())
            .and_then(|s| s.view.selection.clone());
        let mut ws = ws;
        if let Some(sel) = kept_sel {
            ws.view.selection = Some(sel);
        }
        if let Some(slot) = self.sheets.get_mut(active) {
            *slot = Some(ws);
        }
        Ok(())
    }

    /// Painted selection as an A1 ref (`1:1048576` for the whole sheet).
    pub fn selection_ref_now(&self) -> Option<String> {
        self.sheet_ref(self.active)
            .and_then(|s| s.view.selection.as_ref())
            .map(selection_ref)
    }

    /// Scroll so `(row, col)` is fully visible (upstream `GridView.reveal` / `scrollToReveal`).
    pub fn scroll_to_cell(&mut self, row: u32, col: u32) {
        let Some((metrics, header, frozen_rows, frozen_cols)) = self
            .sheets
            .get(self.active)
            .and_then(|s| s.as_ref())
            .map(|sheet| {
                let metrics = grid_metrics(sheet, self.zoom_pct);
                let header = header_sizes(sheet, &metrics, metrics.last_row);
                let (frozen_rows, frozen_cols) = sheet
                    .view
                    .freeze
                    .as_ref()
                    .map(|f| (f.rows, f.cols))
                    .unwrap_or((0, 0));
                (metrics, header, frozen_rows, frozen_cols)
            })
        else {
            self.scroll_x = 0;
            self.scroll_y = 0;
            return;
        };
        let (sx, sy) = scroll_to_reveal(
            &metrics,
            self.scroll_x,
            self.scroll_y,
            self.viewport_w,
            self.viewport_h,
            header.w,
            header.h,
            frozen_rows,
            frozen_cols,
            row,
            col,
        );
        self.scroll_x = sx;
        self.scroll_y = sy;
    }

    /// Apply `sheetView/@topLeftCell` like upstream `grid/index.ts` on workbook/sheet show:
    /// reset scroll to origin, then `reveal(topLeft)`.
    pub fn apply_saved_top_left(&mut self) {
        self.scroll_x = 0;
        self.scroll_y = 0;
        let top = self
            .sheets
            .get(self.active)
            .and_then(|s| s.as_ref())
            .and_then(|s| s.view.top_left);
        if let Some(addr) = top {
            self.scroll_to_cell(addr.row, addr.col);
        }
    }

    fn ensure_sheet_parsed(&mut self, bytes: &[u8], index: usize) -> Result<(), String> {
        if index >= self.sheets.len() {
            return Err(format!("sheet index {} out of range", index));
        }
        if self.lazy.is_some() {
            let rect = self.current_cell_rect();
            return self.sync_lazy_sheet(index, rect);
        }
        if self.sheets[index].is_none() {
            let ws = parse_worksheet_at(bytes, &self.workbook, index)?;
            self.sheets[index] = Some(ws);
        }
        Ok(())
    }

    /// Drop the lazy first-view (and any deferred SeekTail). Used when materialising the
    /// full workbook for search / hyperlinks — not on `ensure_edit` (heap cost).
    pub fn drop_lazy_view(&mut self) {
        self.lazy = None;
        self.pending_tail_complete = false;
    }

    /// Test seam: whether the open path still holds a lazy FirstView.
    #[cfg(test)]
    pub fn lazy_is_some_for_test(&self) -> bool {
        self.lazy.is_some()
    }

    pub(crate) fn lazy_first_view_mut(&mut self) -> Option<&mut FirstView> {
        self.lazy.as_mut()
    }

    pub(crate) fn clear_pending_tail_complete(&mut self) {
        self.pending_tail_complete = false;
    }

    /// Replace the lazy window with a full parse so search and hyperlinks see every cell.
    fn materialise_full(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.lazy.is_none() {
            return Ok(());
        }
        let workbook = load_workbook_meta(bytes)?;
        let parsed = parse_all_worksheets(bytes, &workbook)?;
        self.sheets = parsed;
        self.workbook = workbook;
        self.drop_lazy_view();
        Ok(())
    }

    fn sync_lazy_sheet(&mut self, index: usize, rect: ViewportRect) -> Result<(), String> {
        let lazy = self
            .lazy
            .as_mut()
            .ok_or_else(|| "lazy view closed".to_string())?;
        let has_prev = self.sheets.get(index).and_then(|s| s.as_ref()).is_some();
        // Covered window + local sheet: keep runtime selection / view state (no 1M-cell clone).
        if has_prev && lazy.covers_viewport(index, rect) {
            return Ok(());
        }
        let prev = self.sheets.get(index).and_then(|s| s.as_ref()).cloned();
        let reloaded = lazy.ensure_sheet(index, rect)?;
        if !reloaded && prev.is_some() {
            return Ok(());
        }
        let ws = self
            .lazy
            .as_ref()
            .and_then(|lazy| lazy.sheet(index).cloned());
        if let Some(mut ws) = ws {
            if let Some(old) = prev {
                // FirstView clones the file's sheetView; preserve the live selection.
                ws.view.selection = old.view.selection;
            }
            if index < self.sheets.len() {
                self.sheets[index] = Some(ws);
            }
        }
        Ok(())
    }

    fn current_cell_rect(&self) -> ViewportRect {
        let Some(sheet) = self.sheets.get(self.active).and_then(|s| s.as_ref()) else {
            return default_cell_rect();
        };
        let metrics = grid_metrics(sheet, self.zoom_pct);
        let header = header_sizes(sheet, &metrics, metrics.last_row);
        let (frozen_rows, frozen_cols) = sheet
            .view
            .freeze
            .as_ref()
            .map(|f| (f.rows, f.cols))
            .unwrap_or((0, 0));
        let frozen_h = if frozen_rows > 0 {
            metrics.row_top(frozen_rows)
        } else {
            0
        };
        let frozen_w = if frozen_cols > 0 {
            metrics.col_left(frozen_cols)
        } else {
            0
        };
        let top = frozen_h + self.scroll_y.max(0);
        let left = frozen_w + self.scroll_x.max(0);
        let bottom = top + (self.viewport_h - header.h - frozen_h).max(0);
        let right = left + (self.viewport_w - header.w - frozen_w).max(0);
        let r0 = metrics.row_at(top);
        let r1 = metrics.row_at(bottom.saturating_sub(1).max(top));
        let c0 = metrics.col_at(left);
        let c1 = metrics.col_at(right.saturating_sub(1).max(left));
        ViewportRect {
            r0: r0.min(r1),
            c0: c0.min(c1),
            r1: r0.max(r1),
            c1: c0.max(c1),
        }
    }

    pub fn set_active_sheet(&mut self, bytes: &[u8], index: usize) -> Result<(), String> {
        if index >= self.workbook.sheets.len() {
            return Err(format!("sheet index {} out of range", index));
        }
        self.ensure_sheet_parsed(bytes, index)?;
        self.active = index;
        self.workbook.active_sheet = index as u32;
        // Upstream resets scroll then reveal(topLeft) on sheet change.
        self.apply_saved_top_left();
        self.pending_view_scroll = false;
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
        if self.lazy.is_some() {
            // Inert lazy while editing: paint uses the edit-synced grid + cell_pos without
            // re-stream/replace (cells_1m edit-tick outliers).
            if self.edit.is_none() {
                // First paint stays O(viewport). Schedule post-sheetData shell for the next tick.
                let needs_tail = self
                    .lazy
                    .as_ref()
                    .is_some_and(|lazy| !lazy.is_sheet_complete(self.active));
                if needs_tail {
                    self.pending_tail_complete = true;
                }
                let rect = self.current_cell_rect();
                let active = self.active;
                if self.sync_lazy_sheet(active, rect).is_err() {
                    return;
                }
            }
        } else if self.ensure_sheet_parsed(bytes, self.active).is_err() {
            return;
        }
        let viewport = self.viewport();
        let hints = self.editor_paint_hints();
        let active = self.active;
        let workbook = &self.workbook;
        let mut intern = |s: &str| strings.intern(s);
        if self.grid_in_host {
            let host_sheets = &self.edit.as_ref().expect("grid_in_host").host().sheets;
            paint_grid_frame_host(
                commands,
                &mut intern,
                measures,
                workbook,
                host_sheets,
                active,
                &viewport,
                &hints,
            );
        } else {
            let sheets_ref = &self.sheets;
            let sheet = sheets_ref[active].as_ref().expect("parsed");
            paint_grid_frame(
                commands,
                &mut intern,
                measures,
                workbook,
                sheet,
                active,
                sheets_ref,
                &viewport,
                &hints,
            );
        }
    }

    fn editor_paint_hints(&self) -> EditorPaintHints {
        let Some(edit) = &self.edit else {
            return EditorPaintHints::default();
        };
        let stale = edit.stale_cells(self.active as u32);
        let edit_overlay = if edit.cell().is_editing() && edit.cell().sheet() == self.active as u32
        {
            Some(EditOverlay {
                row: edit.cell().row(),
                col: edit.cell().col(),
                text: edit.cell().text().to_owned(),
                caret: edit.cell().focus(),
            })
        } else {
            None
        };
        let single = self
            .active_sheet()
            .and_then(|s| s.view.selection.as_ref())
            .is_some_and(|sel| sel.ranges.len() == 1);
        EditorPaintHints {
            stale,
            edit: edit_overlay,
            fill_handle: single,
            fill_preview: self
                .fill
                .as_ref()
                .filter(|f| f.to.is_some())
                .map(FillDrag::area),
        }
    }

    /// Apply a frame of input. Returns the new selection ref when the painted selection changed.
    pub fn process_events(&mut self, events: &[Event], strings: &StringTable) -> Option<String> {
        let mut changed = None;
        for ev in events {
            if let Some(r) = self.apply_event(ev, strings) {
                changed = Some(r);
            }
        }
        changed
    }

    fn apply_event(&mut self, ev: &Event, strings: &StringTable) -> Option<String> {
        match ev {
            Event::Resize { width, height, .. } => {
                self.viewport_w = (*width).max(1.0) as i32;
                self.viewport_h = (*height).max(1.0) as i32;
                if self.pending_view_scroll {
                    self.apply_saved_top_left();
                    self.pending_view_scroll = false;
                }
                None
            }
            Event::Wheel { dx, dy, .. } => {
                self.scroll_x = (self.scroll_x + *dx as i32).max(0);
                self.scroll_y = (self.scroll_y + *dy as i32).max(0);
                None
            }
            Event::Pointer {
                type_,
                button,
                x,
                y,
                mods,
                ..
            } => self.on_pointer(*type_, *button, *x, *y, *mods),
            Event::Keyboard {
                type_,
                key_id,
                mods,
                ..
            } if *type_ == 0 => {
                let key = strings.resolve(*key_id).unwrap_or("");
                self.on_key(key, *mods)
            }
            _ => None,
        }
    }

    fn on_pointer(&mut self, type_: u8, button: u8, x: f32, y: f32, mods: u8) -> Option<String> {
        let px = x.round() as i32;
        let py = y.round() as i32;
        match type_ {
            0 if button == 0 => {
                if self
                    .edit
                    .as_ref()
                    .is_some_and(|edit| edit.cell().is_editing())
                {
                    let _ = self.commit_live_edit();
                }
                if let Some(src) = self.fill_handle_at(px, py) {
                    self.fill = Some(FillDrag { src, to: None });
                    self.drag = None;
                    return None;
                }
                let hit = {
                    let zoom = self.zoom_pct;
                    let scroll_x = self.scroll_x;
                    let scroll_y = self.scroll_y;
                    let width = self.viewport_w;
                    let height = self.viewport_h;
                    let sheet = self.active_sheet()?;
                    let metrics = grid_metrics(sheet, zoom);
                    let input = grid_hit_input(&metrics, sheet, scroll_x, scroll_y, width, height);
                    hit(&input, px, py)
                };
                if hit.area != HitArea::Cell || hit.row < 0 || hit.col < 0 {
                    self.drag = None;
                    return None;
                }
                let at = CellAddress {
                    row: hit.row as u32,
                    col: hit.col as u32,
                };
                self.drag = Some((at.row, at.col));
                self.commit_selection(at, mods & MOD_SHIFT != 0)
            }
            1 if self.fill.is_some() => self.fill_move(px, py),
            2 if self.fill.is_some() => self.fill_finish(),
            3 if self.fill.is_some() => {
                self.fill = None;
                self.selection_text()
            }
            1 => {
                let (last_row, last_col) = self.drag?;
                let (row, col) = {
                    let zoom = self.zoom_pct;
                    let scroll_x = self.scroll_x;
                    let scroll_y = self.scroll_y;
                    let width = self.viewport_w;
                    let height = self.viewport_h;
                    let sheet = self.active_sheet()?;
                    let metrics = grid_metrics(sheet, zoom);
                    let input = grid_hit_input(&metrics, sheet, scroll_x, scroll_y, width, height);
                    clamped_hit(&input, px, py)
                };
                if row == last_row && col == last_col {
                    return None;
                }
                self.drag = Some((row, col));
                self.commit_selection(CellAddress { row, col }, true)
            }
            2 | 3 => {
                self.drag = None;
                None
            }
            _ => None,
        }
    }

    /// The selection block when `(px, py)` lands on its fill handle (editable sheet, one range).
    fn fill_handle_at(&self, px: i32, py: i32) -> Option<CellRange> {
        self.edit.as_ref()?;
        let sheet = self.active_sheet()?;
        let sel = sheet.view.selection.as_ref()?;
        let [range] = sel.ranges.as_slice() else {
            return None;
        };
        let metrics = grid_metrics(sheet, self.zoom_pct);
        let input = grid_hit_input(
            &metrics,
            sheet,
            self.scroll_x,
            self.scroll_y,
            self.viewport_w,
            self.viewport_h,
        );
        let x = metrics.col_left(range.end.col + 1);
        let y = metrics.row_top(range.end.row + 1);
        let vx = input.header_w + x
            - if range.end.col < input.frozen_cols {
                0
            } else {
                self.scroll_x.max(0)
            };
        let vy = input.header_h + y
            - if range.end.row < input.frozen_rows {
                0
            } else {
                self.scroll_y.max(0)
            };
        let r = nitro_xml_xlsx::render::FILL_HANDLE_PX / 2 + 2;
        ((px - vx).abs() <= r && (py - vy).abs() <= r).then(|| range.clone())
    }

    fn selection_text(&self) -> Option<String> {
        self.active_sheet()?
            .view
            .selection
            .as_ref()
            .map(selection_ref)
    }

    /// Dragging the handle: extend along the axis the pointer left the block on furthest.
    fn fill_move(&mut self, px: i32, py: i32) -> Option<String> {
        let src = self.fill.as_ref()?.src.clone();
        let (row, col) = {
            let sheet = self.active_sheet()?;
            let metrics = grid_metrics(sheet, self.zoom_pct);
            let input = grid_hit_input(
                &metrics,
                sheet,
                self.scroll_x,
                self.scroll_y,
                self.viewport_w,
                self.viewport_h,
            );
            clamped_hit(&input, px, py)
        };
        let dr = if row > src.end.row {
            (row - src.end.row, "down")
        } else {
            (src.start.row.saturating_sub(row), "up")
        };
        let dc = if col > src.end.col {
            (col - src.end.col, "right")
        } else {
            (src.start.col.saturating_sub(col), "left")
        };
        let to = match (dr.0, dc.0) {
            (0, 0) => None,
            (r, c) if r >= c => Some((dr.1, r)),
            (_, c) => Some((dc.1, c)),
        };
        let fill = self.fill.as_mut()?;
        if fill.to == to {
            return None;
        }
        fill.to = to;
        // Repaint for the preview outline.
        self.selection_text()
    }

    /// Release: one `fillRange` command (one undo step), then select the filled block.
    fn fill_finish(&mut self) -> Option<String> {
        let fill = self.fill.take()?;
        let (dir, count) = fill.to?;
        let sheet = self.active as u32;
        let args = serde_json::json!({
            "command": "fillRange", "sheet": sheet,
            "range": nitro_xml_xlsx::address::format_range(&fill.src),
            "direction": dir, "count": count,
        });
        self.apply_raw_edit(&args.to_string()).ok()?;
        let area = fill.area();
        let active_sheet = self.sheet_mut(self.active)?;
        let active = active_sheet
            .view
            .selection
            .as_ref()
            .map_or(area.start, |s| s.active);
        let next = SheetSelection {
            active,
            anchor: area.start,
            ranges: vec![area],
        };
        let text = selection_ref(&next);
        active_sheet.view.selection = Some(next);
        Some(text)
    }

    fn on_key(&mut self, key: &str, mods: u8) -> Option<String> {
        if self
            .edit
            .as_ref()
            .is_some_and(|edit| edit.cell().is_editing())
        {
            return None;
        }
        if mods & MOD_ALT != 0 {
            return None;
        }
        let ctrl = mods & (MOD_CTRL | MOD_META) != 0;
        let shift = mods & MOD_SHIFT != 0;
        if ctrl && !shift && (key == "a" || key == "A") {
            return self.select_region_or_all();
        }
        let arrow = Arrow::from_key(key)?;
        let _ = self.ensure_merges_ready();
        let active = self.active;
        let zoom = self.zoom_pct;
        let sheet = self.sheet_mut(active)?;
        let metrics = grid_metrics(sheet, zoom);
        let current = sheet
            .view
            .selection
            .clone()
            .unwrap_or_else(|| select_cell(sheet, CellAddress { row: 0, col: 0 }));
        let from = if shift {
            moving_end(&current)
        } else {
            arrow_origin(sheet, &current, arrow)
        };
        let to = navigate_arrow(&metrics, from, arrow);
        let next = if shift {
            extend_selection(&current, sheet, to)
        } else {
            select_cell(sheet, to)
        };
        let text = selection_ref(&next);
        sheet.view.selection = Some(next);
        Some(text)
    }

    fn select_region_or_all(&mut self) -> Option<String> {
        let _ = self.ensure_merges_ready();
        let active = self.active;
        let lazy = self.lazy.is_some();
        let (current, region) = {
            let sheet = self.sheet_ref(active)?;
            let current = sheet
                .view
                .selection
                .clone()
                .unwrap_or_else(|| select_cell(sheet, CellAddress { row: 0, col: 0 }));
            let mut region = current_region_or_all(&self.workbook, sheet, &current);
            // Viewport cells alone under-grow current_region; prefer the streamed used extent.
            if lazy {
                if let Some(dim) = sheet.dimension.clone() {
                    let single = dim.start.row == dim.end.row && dim.start.col == dim.end.col;
                    let same = current.ranges.last().is_some_and(|last| {
                        last.start.row == dim.start.row
                            && last.start.col == dim.start.col
                            && last.end.row == dim.end.row
                            && last.end.col == dim.end.col
                    });
                    if same {
                        region = None;
                    } else if !single {
                        region = Some(dim);
                    }
                }
            }
            (current, region)
        };
        let sheet = self.sheet_mut(active)?;
        let next = match region {
            Some(range) => SheetSelection {
                active: current.active,
                anchor: current.active,
                ranges: vec![range],
            },
            None => SheetSelection {
                active: current.active,
                anchor: CellAddress { row: 0, col: 0 },
                ranges: vec![all_range()],
            },
        };
        let text = selection_ref(&next);
        sheet.view.selection = Some(next);
        Some(text)
    }

    /// `extend` grows from the anchor (shift-click / drag); otherwise `select_cell`.
    fn commit_selection(&mut self, at: CellAddress, extend: bool) -> Option<String> {
        let _ = self.ensure_merges_ready();
        let active = self.active;
        let sheet = self.sheet_mut(active)?;
        let next = if extend {
            let current = sheet
                .view
                .selection
                .clone()
                .unwrap_or_else(|| select_cell(sheet, at));
            extend_selection(&current, sheet, at)
        } else {
            select_cell(sheet, at)
        };
        let text = selection_ref(&next);
        sheet.view.selection = Some(next);
        Some(text)
    }

    fn active_sheet(&self) -> Option<&ActiveWorksheet> {
        self.sheet_ref(self.active)
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
        if self.lazy.is_some() {
            if let Some(r) = parse_range(range) {
                // Copy / TSV must see every cell in the selection, not only the paint window.
                let rect = ViewportRect {
                    r0: r.start.row,
                    c0: r.start.col,
                    r1: r.end.row,
                    c1: r.end.col,
                };
                self.sync_lazy_sheet(sheet_index, rect)?;
            } else {
                self.ensure_sheet_parsed(bytes, sheet_index)?;
            }
        } else {
            self.ensure_sheet_parsed(bytes, sheet_index)?;
        }
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
        self.materialise_full(bytes)?;
        let workbook = &self.workbook;
        let sheet = self
            .sheets
            .get(sheet_index)
            .and_then(|s| s.as_ref())
            .ok_or_else(|| format!("sheet index {sheet_index} out of range"))?;
        Ok(hyperlink_at(workbook, sheet, sheet_index, row, col))
    }

    pub fn search(
        &mut self,
        bytes: &[u8],
        query: &str,
        opts: SearchOptions,
    ) -> Result<Value, String> {
        self.materialise_full(bytes)?;
        Ok(search_workbook(&self.workbook, &self.sheets, query, opts))
    }
}

fn grid_metrics(sheet: &ActiveWorksheet, zoom: Option<f64>) -> GridMetrics {
    create_grid_metrics(
        sheet,
        GridMetricsOptions {
            zoom,
            ..GridMetricsOptions::default()
        },
    )
}

fn grid_hit_input<'a>(
    metrics: &'a GridMetrics,
    sheet: &ActiveWorksheet,
    scroll_x: i32,
    scroll_y: i32,
    width: i32,
    height: i32,
) -> HitInput<'a> {
    let header = header_sizes(sheet, metrics, metrics.last_row);
    let (frozen_rows, frozen_cols) = sheet
        .view
        .freeze
        .as_ref()
        .map(|f| (f.rows, f.cols))
        .unwrap_or((0, 0));
    HitInput {
        metrics,
        header_w: header.w,
        header_h: header.h,
        width,
        height,
        scroll_x,
        scroll_y,
        frozen_rows,
        frozen_cols,
    }
}

fn default_cell_rect() -> ViewportRect {
    // 800×600 host, default column ~64px and row ~20px, minus headers.
    ViewportRect {
        r0: 0,
        c0: 0,
        r1: 36,
        c1: 14,
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
    let selection = if let Some(ref mut xlsx) = handle.xlsx {
        xlsx.process_events(&events, &handle.strings)
    } else {
        None
    };
    if let Some(r) = selection {
        if handle.shell.selection_ref != r {
            handle.shell.selection_ref = r;
            handle.shell.dirty_ui = true;
        }
    }
    handle.events.drain();
    if let Some(ref mut xlsx) = handle.xlsx {
        // Second+ frames: finish deferred SeekTail/SkipRows shell, then paint.
        // First openDocument frame paints without this (pending set during that paint).
        let _ = xlsx.flush_pending_tail();
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
