// SPDX-License-Identifier: MIT
//! Fast first view of an xlsx package.
//!
//! Eager parts are the content types, package rels, workbook, workbook rels, styles and theme.
//! Shared strings resolve only indices seen in the viewport. The active worksheet stream keeps
//! viewport cell rows, then skips the rest of `<sheetData>` so merges / CF / used-range extents
//! still land without storing every cell. Other worksheet parts stay unread until
//! [`FirstView::ensure_sheet`].
//!
//! `load_workbook` / `parse_worksheet` / `parse_all_worksheets` are unchanged and remain the
//! full-fidelity path for the editor, formula engine and oracles.
//!
//! TODO: `ZipPackage` still needs the whole compressed blob to read the central directory. A
//! remote byte-range backend would fetch the EOCD plus the local file ranges of touched parts;
//! [`nitro_xml_opc::opc::Package::stream_part`] already stops inflate early.

mod sheet;
mod sst;

use std::collections::HashSet;

use nitro_xml_opc::opc::Package;

use crate::model::{ActiveWorksheet, SheetState, WorkbookSlice};
use crate::workbook::{load_workbook_shell, stub_worksheet};

use sheet::{COL_SLOP, ROW_SLOP};

/// Inclusive zero-based cell window (same indexing as [`crate::model::DumpCell`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportRect {
    pub r0: u32,
    pub c0: u32,
    pub r1: u32,
    pub c1: u32,
}

/// Workbook opened for a viewer first paint. Cells outside the active window are not stored.
pub struct FirstView {
    bytes: Vec<u8>,
    workbook: WorkbookSlice,
    strings_part: Option<String>,
    sheet_is_worksheet: Vec<bool>,
    dynamic_cells: HashSet<u32>,
    sheets: Vec<Option<ActiveWorksheet>>,
    /// Expanded rect whose cells are currently stored.
    windows: Vec<Option<ViewportRect>>,
    rows_visited: Vec<u32>,
    touched: Vec<String>,
    /// Parts inflated through EOF. Worksheet and shared-string streams that stop early stay out
    /// of this list (they are still reported by [`Self::touched_parts`]).
    parts_read: Vec<String>,
    resolved_sst: HashSet<usize>,
}

/// Open the package and materialise `view` on the active sheet only.
pub fn open_first_view(bytes: &[u8], view: ViewportRect) -> Result<FirstView, String> {
    let mut pkg = Package::open(bytes).map_err(|e| e.to_string())?;
    let shell = load_workbook_shell(&mut pkg)?;
    let n = shell.workbook.sheets.len();
    let active = shell.workbook.active_sheet as usize;
    let mut view_state = FirstView {
        bytes: bytes.to_vec(),
        strings_part: shell.strings_part,
        sheet_is_worksheet: shell.sheet_is_worksheet,
        dynamic_cells: shell.dynamic_cells,
        workbook: shell.workbook,
        sheets: vec![None; n],
        windows: vec![None; n],
        rows_visited: vec![0; n],
        touched: Vec::new(),
        parts_read: Vec::new(),
        resolved_sst: HashSet::new(),
    };
    view_state.merge_pkg(&pkg);
    if active < n {
        view_state.load_sheet(&mut pkg, active, view)?;
        view_state.merge_pkg(&pkg);
    }
    Ok(view_state)
}

impl FirstView {
    /// Load `view` on `sheet` when that rect is not already covered (scroll).
    /// Returns whether the stored window was replaced.
    pub fn ensure_viewport(&mut self, sheet: usize, view: ViewportRect) -> Result<bool, String> {
        self.ensure_sheet(sheet, view)
    }

    /// Whether `view` is already covered by the stored window on `sheet`.
    pub fn covers_viewport(&self, sheet: usize, view: ViewportRect) -> bool {
        self.windows
            .get(sheet)
            .and_then(|w| w.as_ref())
            .is_some_and(|have| covers(have, &view))
    }

    /// Parse `sheet` for `view` without reading any other worksheet part.
    /// Returns `true` when cells were reloaded (caller must refresh viewer state).
    pub fn ensure_sheet(&mut self, sheet: usize, view: ViewportRect) -> Result<bool, String> {
        if sheet >= self.workbook.sheets.len() {
            return Err(format!("sheet index {sheet} out of range"));
        }
        let expanded = expand_view(view);
        if self.windows.get(sheet).and_then(|w| w.as_ref()) == Some(&expanded)
            || self
                .windows
                .get(sheet)
                .and_then(|w| w.as_ref())
                .is_some_and(|have| covers(have, &view))
        {
            return Ok(false);
        }
        let bytes = self.bytes.clone();
        let mut pkg = Package::open(&bytes).map_err(|e| e.to_string())?;
        self.load_sheet(&mut pkg, sheet, view)?;
        self.merge_pkg(&pkg);
        Ok(true)
    }

    pub fn sheet(&self, sheet: usize) -> Option<&ActiveWorksheet> {
        self.sheets.get(sheet).and_then(|s| s.as_ref())
    }

    /// Styles, theme and sheet stubs. [`WorkbookSlice::shared_strings`] stays empty; string cells
    /// in the window already hold their text.
    pub fn workbook(&self) -> &WorkbookSlice {
        &self.workbook
    }

    /// Parts whose inflate started, including streams that stopped early.
    pub fn touched_parts(&self) -> &[String] {
        &self.touched
    }

    /// `<row>` elements visited on `sheet` for the current window (not the sheet's row count).
    pub fn rows_visited(&self, sheet: usize) -> u32 {
        self.rows_visited.get(sheet).copied().unwrap_or(0)
    }

    /// Shared-string entries materialised for viewport cells, not `uniqueCount`.
    pub fn shared_strings_resolved(&self) -> usize {
        self.resolved_sst.len()
    }

    fn load_sheet(
        &mut self,
        pkg: &mut Package,
        sheet: usize,
        view: ViewportRect,
    ) -> Result<(), String> {
        let is_ws = self.sheet_is_worksheet.get(sheet).copied().unwrap_or(false);
        let stub = self
            .workbook
            .sheets
            .get(sheet)
            .ok_or_else(|| format!("sheet index {sheet} out of range"))?;
        if !is_ws || stub.part_name.is_none() {
            if stub.state == SheetState::Visible && stub.part_name.is_none() {
                // Chart / non-worksheet stubs carry no grid part.
            }
            self.sheets[sheet] = Some(stub_worksheet(stub));
            self.windows[sheet] = Some(expand_view(view));
            self.rows_visited[sheet] = 0;
            return Ok(());
        }
        let part = stub.part_name.clone().unwrap_or_default();
        let name = stub.name.clone();
        let sheet_id = stub.sheet_id;
        let state = stub.state;
        let req = sheet::SheetRequest {
            part_name: &part,
            view,
            xf_map: &self.workbook.xf_map,
            date1904: self.workbook.date1904,
            palette: self.workbook.palette.as_deref(),
            dynamic_cells: &self.dynamic_cells,
            strings_part: self.strings_part.as_deref(),
            name: &name,
            sheet_id,
            state,
        };
        let mut warnings = Vec::new();
        let (ws, visited) =
            sheet::load_sheet_window(pkg, &req, &mut self.resolved_sst, &mut warnings)?;
        for w in warnings {
            if !self.workbook.warnings.contains(&w) {
                self.workbook.warnings.push(w);
            }
        }
        self.rows_visited[sheet] = visited;
        self.sheets[sheet] = Some(ws);
        self.windows[sheet] = Some(expand_view(view));
        Ok(())
    }

    fn merge_pkg(&mut self, pkg: &Package) {
        for part in pkg.touched_parts() {
            push_unique(&mut self.touched, part);
        }
        for part in pkg.parts_read() {
            push_unique(&mut self.parts_read, part);
        }
    }
}

fn expand_view(view: ViewportRect) -> ViewportRect {
    ViewportRect {
        r0: view.r0.saturating_sub(ROW_SLOP),
        c0: view.c0.saturating_sub(COL_SLOP),
        r1: view.r1.saturating_add(ROW_SLOP),
        c1: view.c1.saturating_add(COL_SLOP),
    }
}

fn covers(have: &ViewportRect, view: &ViewportRect) -> bool {
    view.r0 >= have.r0 && view.r1 <= have.r1 && view.c0 >= have.c0 && view.c1 <= have.c1
}

fn push_unique(list: &mut Vec<String>, part: &str) {
    if !list.iter().any(|p| p == part) {
        list.push(part.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DumpCellValue;
    use crate::sheet_access::CellIndex;
    use crate::view::cell_view::{cell_view, CellViewOptions};
    use crate::view::metrics::{create_grid_metrics, GridMetricsOptions};
    use crate::workbook::{load_workbook, parse_all_worksheets, parse_worksheet_at};

    fn cell_texts(
        book: &WorkbookSlice,
        sheet: &ActiveWorksheet,
        rect: ViewportRect,
    ) -> Vec<String> {
        let index = CellIndex::build(sheet);
        let mut out = Vec::new();
        for row in rect.r0..=rect.r1 {
            for col in rect.c0..=rect.c1 {
                let text = cell_view(
                    book,
                    sheet,
                    &index,
                    row,
                    col,
                    CellViewOptions {
                        sheet_index: Some(0),
                        ..CellViewOptions::default()
                    },
                )
                .text;
                out.push(text);
            }
        }
        out
    }

    fn assert_rect_matches(bytes: &[u8], rect: ViewportRect) {
        let full = load_workbook(bytes).expect("full workbook");
        let full_sheet = full
            .active_worksheet
            .as_ref()
            .expect("active sheet on the full path");
        let lazy = open_first_view(bytes, rect).expect("first view");
        let lazy_sheet = lazy.sheet(full.active_sheet as usize).expect("lazy sheet");
        let left = cell_texts(&full, full_sheet, rect);
        let right = cell_texts(lazy.workbook(), lazy_sheet, rect);
        assert_eq!(left, right);
    }

    // Catches: lazy first-view dropping row `ht` below the cell window so topLeft reveal scrolls short.
    #[test]
    fn lazy_keeps_row_heights_below_viewport() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/real/xlsx/poi-45540_classic_Footer.xlsx");
        let bytes = std::fs::read(&path).expect("fixture");
        let full = load_workbook(&bytes).expect("full");
        let full_sheet =
            parse_worksheet_at(&bytes, &full, full.active_sheet as usize).expect("full sheet");
        let lazy = open_first_view(
            &bytes,
            ViewportRect {
                r0: 0,
                c0: 0,
                r1: 36,
                c1: 14,
            },
        )
        .expect("first view");
        let lazy_sheet = lazy
            .sheet(full.active_sheet as usize)
            .expect("lazy sheet");
        let full_m = create_grid_metrics(&full_sheet, GridMetricsOptions::default());
        let lazy_m = create_grid_metrics(lazy_sheet, GridMetricsOptions::default());
        // A91 is topLeftCell; row tops through that cell must match the full load.
        assert_eq!(lazy_m.row_top(90), full_m.row_top(90));
        assert_eq!(lazy_m.row_height(90), full_m.row_height(90));
    }

    // Catches: Apache POI 56644 (real corpus) lazy first-view must paint Customer row like full load.
    #[test]
    fn poi_56644_viewport_cell_text_matches_full_load() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/real/xlsx/poi-56644.xlsx");
        let bytes = std::fs::read(&path).expect("fixture");
        assert_rect_matches(
            &bytes,
            ViewportRect {
                r0: 0,
                c0: 0,
                r1: 36,
                c1: 14,
            },
        );
    }

    // Catches: a viewport-sized parse dropping or rewriting cell text relative to load_workbook.
    #[test]
    fn viewport_cell_text_matches_full_load() {
        let cells_1k = include_bytes!("../../../../../fixtures/generated/xlsx/cells_1k.xlsx");
        let cells_100k = include_bytes!("../../../../../fixtures/generated/xlsx/cells_100k.xlsx");
        let top = ViewportRect {
            r0: 0,
            c0: 0,
            r1: 30,
            c1: 8,
        };
        assert_rect_matches(cells_1k, top);
        assert_rect_matches(cells_100k, top);
        let scrolled = ViewportRect {
            r0: 400,
            c0: 0,
            r1: 440,
            c1: 10,
        };
        let full = load_workbook(cells_100k).expect("full");
        let full_sheet = full.active_worksheet.as_ref().expect("sheet");
        let mut lazy = open_first_view(cells_100k, top).expect("first view");
        assert!(lazy.ensure_viewport(0, scrolled).expect("scroll"));
        let lazy_sheet = lazy.sheet(0).expect("scrolled sheet");
        assert_eq!(
            cell_texts(&full, full_sheet, scrolled),
            cell_texts(lazy.workbook(), lazy_sheet, scrolled)
        );
        let total_rows = full_sheet.cells.iter().map(|c| c.row).max().unwrap_or(0) + 1;
        assert!(
            lazy.rows_visited(0) * 4 < total_rows,
            "rows_visited {} should be well under {total_rows}",
            lazy.rows_visited(0)
        );
        assert!(lazy.shared_strings_resolved() < full.shared_strings.len() / 2);
        assert!(lazy.shared_strings_resolved() < 200);
    }

    // Catches: first view inflating sheet2, pad.bin, the whole shared-string table, or every row.
    #[test]
    fn first_view_skips_other_sheets_pad_and_full_tables() {
        let pad = include_bytes!("../../../../../fixtures/generated/xlsx/cells_1k_pad_s.xlsx");
        let view = open_first_view(
            pad,
            ViewportRect {
                r0: 0,
                c0: 0,
                r1: 20,
                c1: 5,
            },
        )
        .expect("pad fixture");
        assert!(
            view.touched_parts().iter().any(|p| p.contains("sheet1")),
            "active sheet should be touched, got {:?}",
            view.touched_parts()
        );
        assert!(
            !view.touched_parts().iter().any(|p| p.contains("pad.bin")),
            "pad.bin was inflated: {:?}",
            view.touched_parts()
        );
        // Active sheet is streamed through EOF so merges/CF after sheetData are available, but
        // pad.bin and other sheets must stay untouched.
        assert!(
            view.parts_read.iter().any(|p| p.contains("sheet1")),
            "active sheet should finish so the post-sheetData tail is captured: {:?}",
            view.parts_read
        );

        let bytes = two_sheet_package();
        let full = load_workbook(&bytes).expect("full synthetic");
        let all = parse_all_worksheets(&bytes, &full).expect("editor path reads every sheet");
        let sheet2 = all.get(1).and_then(|s| s.as_ref()).expect("sheet2");
        assert!(
            sheet2.cells.iter().any(|c| matches!(
                &c.value,
                DumpCellValue::String(s) if s == "only-sheet2"
            )),
            "parse_all_worksheets must still materialise sheet2"
        );
        let lazy = open_first_view(
            &bytes,
            ViewportRect {
                r0: 0,
                c0: 0,
                r1: 5,
                c1: 2,
            },
        )
        .expect("lazy synthetic");
        assert!(lazy.sheet(1).is_none());
        assert!(
            !lazy.touched_parts().iter().any(|p| p.contains("sheet2")),
            "sheet2 touched: {:?}",
            lazy.touched_parts()
        );
        assert!(
            !lazy.touched_parts().iter().any(|p| p.contains("pad.bin")),
            "synthetic pad touched: {:?}",
            lazy.touched_parts()
        );
        assert!(lazy.shared_strings_resolved() < 40);
        assert!(lazy.shared_strings_resolved() * 3 < full.shared_strings.len());
        assert!(
            !lazy.parts_read.iter().any(|p| p.contains("sharedStrings")),
            "shared strings inflated to EOF: {:?}",
            lazy.parts_read
        );
        assert!(lazy.rows_visited(0) < 40);
        // Viewport cell store stays small even though the sheet part finished.
        let cells = lazy.sheet(0).map(|s| s.cells.len()).unwrap_or(0);
        assert!(
            cells < 80,
            "kept {cells} cells; viewport should stay sparse"
        );
    }

    // Catches: early sheetData stop dropping mergeCells / used-range so selection parity fails.
    #[test]
    fn first_view_keeps_merges_and_used_extent() {
        let bytes = include_bytes!("../../../../../fixtures/generated/xlsx/cells_1k_merges.xlsx");
        let lazy = open_first_view(
            bytes,
            ViewportRect {
                r0: 0,
                c0: 0,
                r1: 25,
                c1: 10,
            },
        )
        .expect("merges fixture");
        let sheet = lazy.sheet(0).expect("active sheet");
        assert!(
            sheet.merges.iter().any(|m| {
                m.start.row == 0 && m.start.col == 0 && m.end.row == 1 && m.end.col == 1
            }),
            "A1:B2 merge missing: {:?}",
            sheet.merges
        );
        let dim = sheet.dimension.as_ref().expect("dimension");
        assert!(
            dim.end.row >= 49 && dim.end.col >= 19,
            "used extent should cover A1:T50-ish, got {:?}:{:?}",
            dim.start,
            dim.end
        );
        assert!(
            sheet.cells.len() < 600,
            "must not keep all 1k cells, kept {}",
            sheet.cells.len()
        );
        assert!(
            sheet.cells.len() < 1000,
            "viewport+slop store must stay under the full sheet"
        );
        assert!(lazy.rows_visited(0) < 50);
    }

    fn two_sheet_package() -> Vec<u8> {
        let sst_count = 280;
        let mut sis = String::new();
        for i in 0..sst_count {
            sis.push_str(&format!("<si><t>s{i}</t></si>"));
        }
        let sst = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"{sst_count}\" uniqueCount=\"{sst_count}\">{sis}</sst>"
        );
        let mut rows = String::new();
        for r in 1..=80 {
            rows.push_str(&format!(
                "<row r=\"{r}\"><c r=\"A{r}\" t=\"s\"><v>{}</v></c><c r=\"B{r}\"><v>{}</v></c></row>",
                r - 1,
                r * 3
            ));
        }
        let sheet1 = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>{rows}</sheetData></worksheet>"
        );
        let sheet2 = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>only-sheet2</t></is></c></row></sheetData></worksheet>";
        let workbook = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/><sheet name=\"Sheet2\" sheetId=\"2\" r:id=\"rId2\"/></sheets></workbook>";
        let wb_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet2.xml\"/><Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/><Relationship Id=\"rId4\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>";
        let root_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>";
        let styles = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts><fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills><borders count=\"1\"><border/></borders><cellStyleXfs count=\"1\"><xf/></cellStyleXfs><cellXfs count=\"1\"><xf xfId=\"0\"/></cellXfs></styleSheet>";
        let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/worksheets/sheet2.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/><Override PartName=\"/xl/media/pad.bin\" ContentType=\"application/octet-stream\"/></Types>";
        let pad = vec![0xABu8; 64];
        stored_zip(&[
            ("[Content_Types].xml", content_types.as_bytes()),
            ("_rels/.rels", root_rels.as_bytes()),
            ("xl/workbook.xml", workbook.as_bytes()),
            ("xl/_rels/workbook.xml.rels", wb_rels.as_bytes()),
            ("xl/worksheets/sheet1.xml", sheet1.as_bytes()),
            ("xl/worksheets/sheet2.xml", sheet2.as_bytes()),
            ("xl/sharedStrings.xml", sst.as_bytes()),
            ("xl/styles.xml", styles.as_bytes()),
            ("xl/media/pad.bin", &pad),
        ])
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    fn stored_zip(parts: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in parts {
            let offset = u32::try_from(out.len()).unwrap_or(u32::MAX);
            let crc = crc32(data);
            let size = u32::try_from(data.len()).unwrap_or(u32::MAX);
            let name_len = u16::try_from(name.len()).unwrap_or(u16::MAX);
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&name_len.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);

            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&name_len.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_offset = u32::try_from(out.len()).unwrap_or(u32::MAX);
        let count = u16::try_from(parts.len()).unwrap_or(u16::MAX);
        let cd_size = u32::try_from(central.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }
}
