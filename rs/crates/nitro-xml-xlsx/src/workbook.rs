// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/load.ts, workbook-part.ts,
// formula-text.ts (stripFuturePrefixes), and defaults from src/core/xlsx/workbook.ts; changes
// listed in PROVENANCE.md.
//! Workbook part parse, Stage-3 load orchestration, and defaults.

use std::collections::HashMap;

use nitro_xml_opc::opc::{detect_digital_signatures, relationship_types, Package, Relationship};
use nitro_xml_opc::xml::parse_xml_str;

use crate::address::parse_range;
use crate::formula_text::strip_future_prefixes;
use crate::model::{
    ActiveWorksheet, Border, CellStyle, Color, DefinedName, Fill, Font, ModernPasswordHash,
    PageSetup, SheetState, SheetStub, ThemePalette, WorkbookSlice,
};
use crate::password_hash::read_modern_hash;
use crate::shared_strings::parse_shared_strings;
use crate::styles::parse_styles;
use crate::theme::parse_theme;
use crate::worksheet::{load_dynamic_cells, parse_worksheet, WorksheetContext};
use crate::xml_util::{att, bool_attr_or, num_attr, rel_attr, x_children, x_first, x_text};

/// The Office 2013+ default theme ("Office") palette.
pub fn default_theme() -> ThemePalette {
    ThemePalette {
        colors: vec![
            "FFFFFF".into(),
            "000000".into(),
            "E7E6E6".into(),
            "44546A".into(),
            "4472C4".into(),
            "ED7D31".into(),
            "A5A5A5".into(),
            "FFC000".into(),
            "5B9BD5".into(),
            "70AD47".into(),
            "0563C1".into(),
            "954F72".into(),
        ],
        major_font: "Calibri Light".into(),
        minor_font: "Calibri".into(),
    }
}

/// The default cell format (11pt Calibri, General, no fill or border).
pub fn default_cell_style() -> CellStyle {
    CellStyle {
        font: Font {
            name: Some("Calibri".into()),
            size: Some(11.0),
            bold: None,
            italic: None,
            underline: None,
            strike: None,
            color: Some(Color {
                rgb: None,
                theme: Some(1.0),
                tint: None,
                indexed: None,
                auto: None,
            }),
            vert_align: None,
            family: Some(2.0),
            scheme: Some("minor".into()),
        },
        fill: Fill::Pattern {
            pattern: "none".into(),
            fg_color: None,
            bg_color: None,
        },
        border: Border::default(),
        num_fmt: "General".into(),
        alignment: None,
        protection: None,
        cell_style_name: None,
        quote_prefix: None,
        pivot_button: None,
    }
}

pub(crate) struct SheetEntry {
    pub(crate) name: String,
    pub(crate) sheet_id: u32,
    pub(crate) state: SheetState,
    pub(crate) rel_id: String,
}

pub(crate) struct WorkbookPart {
    pub(crate) sheets: Vec<SheetEntry>,
    pub(crate) defined_names: Vec<DefinedName>,
    pub(crate) date1904: bool,
    pub(crate) active_tab: u32,
    pub(crate) structure_locked: bool,
    pub(crate) full_calc_on_load: bool,
    pub(crate) workbook_password_hash: Option<String>,
    pub(crate) workbook_modern_hash: Option<ModernPasswordHash>,
    pub(crate) calc_mode: Option<String>,
    pub(crate) iterate: Option<(f64, f64)>,
}

fn push_warn(warnings: &mut Vec<String>, message: String) {
    if !warnings.contains(&message) {
        warnings.push(message);
    }
}

const PRINT_AREA: &str = "_xlnm.Print_Area";

const CONTENT_TYPE_WORKBOOK_MACRO: &str = "application/vnd.ms-excel.sheet.macroEnabled.main+xml";
const CONTENT_TYPE_TEMPLATE_MACRO: &str = "application/vnd.ms-excel.template.macroEnabled.main+xml";

pub(crate) fn parse_workbook_part(xml: &str) -> Result<WorkbookPart, String> {
    let doc = parse_xml_str(xml).map_err(|e| e.to_string())?;
    let root = &doc.root;
    let mut sheets = Vec::new();
    let sheets_parent = x_first(Some(root), "sheets").unwrap_or(root);
    for (index, node) in x_children(sheets_parent, "sheet").into_iter().enumerate() {
        let state = match att(Some(node), "state") {
            Some("hidden") => SheetState::Hidden,
            Some("veryHidden") => SheetState::VeryHidden,
            _ => SheetState::Visible,
        };
        sheets.push(SheetEntry {
            name: att(Some(node), "name")
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Sheet{}", index + 1)),
            sheet_id: num_attr(Some(node), "sheetId")
                .map(|n| n as u32)
                .unwrap_or((index + 1) as u32),
            state,
            rel_id: rel_attr(Some(node), "id").unwrap_or("").to_owned(),
        });
    }
    let mut defined_names = Vec::new();
    let names_parent = x_first(Some(root), "definedNames").unwrap_or(root);
    for node in x_children(names_parent, "definedName") {
        let name = att(Some(node), "name").unwrap_or("").to_owned();
        let formula = strip_future_prefixes(&x_text(Some(node)));
        let mut entry = DefinedName {
            name,
            formula,
            local_sheet: None,
            hidden: None,
            comment: None,
        };
        if let Some(local) = num_attr(Some(node), "localSheetId") {
            entry.local_sheet = Some(local as u32);
        }
        if bool_attr_or(Some(node), "hidden", false) {
            entry.hidden = Some(true);
        }
        if let Some(comment) = att(Some(node), "comment").filter(|c| !c.is_empty()) {
            entry.comment = Some(comment.to_owned());
        }
        if !entry.name.is_empty() {
            defined_names.push(entry);
        }
    }
    let view = x_first(x_first(Some(root), "bookViews"), "workbookView");
    let protection = x_first(Some(root), "workbookProtection");
    let password = att(protection, "workbookPassword")
        .filter(|p| !p.is_empty())
        .map(str::to_owned);
    let modern_hash = read_modern_hash(protection, "workbook");
    let calc_pr = x_first(Some(root), "calcPr");
    let calc_mode = att(calc_pr, "calcMode");
    let iterate = if bool_attr_or(calc_pr, "iterate", false) {
        Some((
            num_attr(calc_pr, "iterateCount").unwrap_or(100.0),
            num_attr(calc_pr, "iterateDelta").unwrap_or(0.001),
        ))
    } else {
        None
    };
    Ok(WorkbookPart {
        sheets,
        defined_names,
        date1904: bool_attr_or(x_first(Some(root), "workbookPr"), "date1904", false),
        active_tab: num_attr(view, "activeTab").unwrap_or(0.0) as u32,
        structure_locked: bool_attr_or(protection, "lockStructure", false),
        full_calc_on_load: bool_attr_or(calc_pr, "fullCalcOnLoad", false),
        workbook_password_hash: password,
        workbook_modern_hash: modern_hash,
        calc_mode: if calc_mode == Some("manual") {
            Some("manual".into())
        } else {
            None
        },
        iterate,
    })
}

/// Match `/^(?:'(?:[^']|'')+'|[^!,]+)!([$A-Z0-9:]+)$/i` and return the range capture.
fn match_print_area_formula(formula: &str) -> Option<&str> {
    let trimmed = formula.trim();
    let bang = trimmed.rfind('!')?;
    let sheet = &trimmed[..bang];
    let range = &trimmed[bang + 1..];
    if range.is_empty()
        || !range
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '$' || c == ':')
    {
        return None;
    }
    // Sheet side: quoted `'Name''s'!` or unquoted without `!` or `,`.
    if let Some(rest) = sheet.strip_prefix('\'') {
        if !rest.ends_with('\'') {
            return None;
        }
        let inner = &rest[..rest.len() - 1];
        // Allow '' escapes; reject bare '
        let mut i = 0;
        let bytes = inner.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'\'' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                } else {
                    return None;
                }
            } else {
                i += 1;
            }
        }
    } else if sheet.is_empty() || sheet.contains('!') || sheet.contains(',') {
        return None;
    }
    Some(range)
}

pub(crate) fn lift_print_areas(workbook: &mut WorkbookSlice) {
    let mut kept = Vec::new();
    for name in workbook.defined_names.drain(..) {
        if name.name != PRINT_AREA || name.local_sheet.is_none() {
            kept.push(name);
            continue;
        }
        let local = name.local_sheet.unwrap() as usize;
        let Some(sheet) = workbook.sheets.get_mut(local) else {
            kept.push(name);
            continue;
        };
        let Some(range_text) = match_print_area_formula(&name.formula) else {
            kept.push(name);
            continue;
        };
        let Some(range) = parse_range(range_text) else {
            kept.push(name);
            continue;
        };
        let mut page_setup = sheet
            .page_setup
            .clone()
            .unwrap_or(PageSetup { print_area: None });
        page_setup.print_area = Some(range);
        sheet.page_setup = Some(page_setup);
    }
    workbook.defined_names = kept;
}

/// Workbook shell without shared-string materialisation or worksheet cell parse.
///
/// [`load_workbook`] fills shared strings and the active sheet on top of this. The lazy first
/// view reuses it so those two stay proportional to the viewport, not the file.
pub(crate) struct WorkbookShell {
    pub workbook: WorkbookSlice,
    pub strings_part: Option<String>,
    pub sheet_is_worksheet: Vec<bool>,
    pub dynamic_cells: std::collections::HashSet<u32>,
}

/// Eager package parts: content types (via open), root rels, workbook, workbook rels, styles, theme.
/// Does not inflate `sharedStrings` or any worksheet part.
pub(crate) fn load_workbook_shell(pkg: &mut Package) -> Result<WorkbookShell, String> {
    let workbook_part = pkg
        .target_of_type("", relationship_types::OFFICE_DOCUMENT)
        .map_err(|e| e.to_string())?
        .filter(|p| pkg.has(p))
        .or_else(|| {
            if pkg.has("xl/workbook.xml") {
                Some("xl/workbook.xml".into())
            } else {
                None
            }
        })
        .ok_or_else(|| "XLSX package has no workbook part (xl/workbook.xml)".to_owned())?;
    let workbook_xml = pkg
        .text(&workbook_part)
        .map_err(|e| e.to_string())?
        .unwrap_or("")
        .to_owned();
    let book = parse_workbook_part(&workbook_xml)?;
    let mut warnings: Vec<String> = Vec::new();

    let styles_part = pkg
        .target_of_type(&workbook_part, relationship_types::STYLES)
        .map_err(|e| e.to_string())?;
    let styles_xml = match &styles_part {
        Some(p) => pkg.text(p).map_err(|e| e.to_string())?.map(str::to_owned),
        None => None,
    };
    let styles = parse_styles(styles_xml.as_deref(), &mut warnings);

    let theme_part = pkg
        .target_of_type(&workbook_part, relationship_types::THEME)
        .map_err(|e| e.to_string())?;
    let theme_xml = match &theme_part {
        Some(p) => pkg.text(p).map_err(|e| e.to_string())?.map(str::to_owned),
        None => None,
    };
    let theme = parse_theme(theme_xml.as_deref());

    let strings_part = pkg
        .target_of_type(&workbook_part, relationship_types::SHARED_STRINGS)
        .map_err(|e| e.to_string())?;

    let rels_map: HashMap<String, Relationship> = pkg
        .rels(&workbook_part)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let dynamic_cells = load_dynamic_cells(pkg, &workbook_part);

    let mut sheets: Vec<SheetStub> = Vec::new();
    let mut sheet_is_worksheet: Vec<bool> = Vec::new();
    for entry in &book.sheets {
        let rel = rels_map.get(&entry.rel_id);
        let target = rel.and_then(|r| pkg.target(&workbook_part, r));
        let is_worksheet = rel.map(|r| r.type_.as_str()) == Some(relationship_types::WORKSHEET);
        sheet_is_worksheet.push(is_worksheet);
        let mut sheet = SheetStub {
            name: entry.name.clone(),
            sheet_id: entry.sheet_id,
            state: entry.state,
            part_name: None,
            page_setup: None,
        };
        if rel.map(|r| r.type_.as_str()) == Some(relationship_types::WORKSHEET) && target.is_some()
        {
            sheet.part_name = target;
        } else {
            if let Some(t) = target {
                sheet.part_name = Some(t);
            }
            let kind = rel
                .and_then(|r| r.type_.rsplit('/').next())
                .unwrap_or("sheet");
            push_warn(
                &mut warnings,
                format!(
                    "Sheet \"{}\" is a {}; it is kept on save but cannot be shown or edited.",
                    entry.name, kind
                ),
            );
        }
        sheets.push(sheet);
    }

    let vba = pkg.has("xl/vbaProject.bin")
        || pkg
            .target_of_type(&workbook_part, relationship_types::VBA_PROJECT)
            .map_err(|e| e.to_string())?
            .is_some();
    if vba {
        push_warn(
            &mut warnings,
            "The workbook contains VBA macros; they are kept on save but never run.".into(),
        );
    }
    if pkg
        .target_of_type(&workbook_part, relationship_types::PIVOT_CACHE_DEFINITION)
        .map_err(|e| e.to_string())?
        .is_some()
    {
        push_warn(
            &mut warnings,
            "Pivot tables are kept on save but are not refreshed or shown as pivot tables.".into(),
        );
    }
    if pkg
        .target_of_type(&workbook_part, relationship_types::EXTERNAL_LINK)
        .map_err(|e| e.to_string())?
        .is_some()
    {
        push_warn(
            &mut warnings,
            "External workbook links are kept but not updated.".into(),
        );
    }
    if !detect_digital_signatures(pkg.part_names()).is_empty() {
        push_warn(
            &mut warnings,
            "The workbook is digitally signed. The signature is not verified, and saving removes it because any change invalidates it.".into(),
        );
    }

    let content_type = pkg
        .content_type(&workbook_part)
        .unwrap_or("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml");
    let format = if content_type == CONTENT_TYPE_WORKBOOK_MACRO
        || content_type == CONTENT_TYPE_TEMPLATE_MACRO
    {
        "xlsm"
    } else {
        "xlsx"
    };

    if sheets.is_empty() {
        return Err("XLSX workbook has no sheets".into());
    }
    let active_sheet = book.active_tab.min(sheets.len().saturating_sub(1) as u32);

    let mut workbook = WorkbookSlice {
        format: format.into(),
        date1904: book.date1904,
        active_sheet,
        sheets,
        defined_names: book.defined_names,
        theme,
        styles: styles.styles,
        named_styles: styles.named_styles,
        xf_map: styles.xf_map,
        palette: styles.palette,
        dxfs: styles.dxfs,
        shared_strings: Vec::new(),
        warnings,
        full_calc_on_load: book.full_calc_on_load.then_some(true),
        structure_locked: book.structure_locked.then_some(true),
        calc_mode: book.calc_mode,
        iterate: book.iterate,
        workbook_password_hash: book.workbook_password_hash,
        workbook_modern_hash: book.workbook_modern_hash,
        active_worksheet: None,
    };
    lift_print_areas(&mut workbook);

    Ok(WorkbookShell {
        workbook,
        strings_part,
        sheet_is_worksheet,
        dynamic_cells,
    })
}

/// Load Stage-3 workbook slice (workbook / styles / theme / shared strings; sheet stubs only)
/// plus the active worksheet's full cell table.
pub fn load_workbook(bytes: &[u8]) -> Result<WorkbookSlice, String> {
    let mut pkg = Package::open(bytes).map_err(|e| e.to_string())?;
    let mut shell = load_workbook_shell(&mut pkg)?;
    if let Some(part) = shell.strings_part.clone() {
        let strings_xml = pkg
            .text(&part)
            .map_err(|e| e.to_string())?
            .map(str::to_owned);
        shell.workbook.shared_strings =
            parse_shared_strings(strings_xml.as_deref(), shell.workbook.palette.as_deref());
    }
    let active_idx = shell.workbook.active_sheet as usize;
    if shell.sheet_is_worksheet.get(active_idx) == Some(&true) {
        let stub = &shell.workbook.sheets[active_idx];
        if let Some(part) = stub.part_name.clone() {
            let mut ws_ctx = WorksheetContext {
                // Move SST into the worksheet context for the parse, then restore —
                // avoids a full second copy of every shared string during load.
                shared_strings: std::mem::take(&mut shell.workbook.shared_strings),
                xf_map: shell.workbook.xf_map.clone(),
                date1904: shell.workbook.date1904,
                palette: shell.workbook.palette.clone(),
                dxfs: shell.workbook.dxfs.clone(),
                dynamic_cells: shell.dynamic_cells.clone(),
                warnings: Vec::new(),
            };
            let name = stub.name.clone();
            let sheet_id = stub.sheet_id;
            let state = stub.state;
            let ws = parse_worksheet(&mut ws_ctx, &mut pkg, &part, &name, sheet_id, state);
            shell.workbook.shared_strings = std::mem::take(&mut ws_ctx.shared_strings);
            for w in std::mem::take(&mut ws_ctx.warnings) {
                push_warn(&mut shell.workbook.warnings, w);
            }
            shell.workbook.active_worksheet = Some(ws);
        }
    }
    Ok(shell.workbook)
}

impl WorkbookSlice {
    /// Materialize [`ActiveWorksheet::cells`] for the active sheet when the load path kept a living
    /// [`ActiveWorksheet::cell_store`].
    pub fn ensure_active_dump_cells(&mut self) {
        if let Some(ws) = &mut self.active_worksheet {
            let sst = &self.shared_strings;
            ws.ensure_dump_cells(sst);
        }
    }
}

pub(crate) fn stub_worksheet(stub: &SheetStub) -> crate::model::ActiveWorksheet {
    crate::model::ActiveWorksheet {
        name: stub.name.clone(),
        sheet_id: stub.sheet_id,
        state: stub.state,
        part_name: stub.part_name.clone(),
        default_row_height: 15.0,
        default_col_width: None,
        format: None,
        view: crate::sheet_props::default_sheet_view(),
        columns: Vec::new(),
        merges: Vec::new(),
        row_info: std::collections::BTreeMap::new(),
        cell_store: None,
        cells: Vec::new(),
        data_validations: Vec::new(),
        comment_addresses: Vec::new(),
        hyperlinks: Vec::new(),
        ext_lst: Vec::new(),
        sparkline_groups: Vec::new(),
        drawings: Vec::new(),
        conditional_formats: Vec::new(),
        dimension: None,
    }
}

/// Parse every worksheet part (for view-IR dumps). Chart sheets and other non-worksheet parts use
/// upstream `createWorksheet` defaults (empty grid, 15pt rows, no default column width).
pub fn parse_all_worksheets(
    bytes: &[u8],
    workbook: &WorkbookSlice,
) -> Result<Vec<Option<crate::model::ActiveWorksheet>>, String> {
    let mut pkg = Package::open(bytes).map_err(|e| e.to_string())?;
    let workbook_part = pkg
        .target_of_type("", relationship_types::OFFICE_DOCUMENT)
        .map_err(|e| e.to_string())?
        .filter(|p| pkg.has(p))
        .or_else(|| {
            if pkg.has("xl/workbook.xml") {
                Some("xl/workbook.xml".into())
            } else {
                None
            }
        })
        .ok_or_else(|| "XLSX package has no workbook part (xl/workbook.xml)".to_owned())?;
    let workbook_xml = pkg
        .text(&workbook_part)
        .map_err(|e| e.to_string())?
        .unwrap_or("")
        .to_owned();
    let book = parse_workbook_part(&workbook_xml)?;
    let rels_map: HashMap<String, Relationship> = pkg
        .rels(&workbook_part)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let dynamic_cells = load_dynamic_cells(&mut pkg, &workbook_part);
    let mut out = Vec::with_capacity(workbook.sheets.len());
    for (entry, stub) in book.sheets.iter().zip(workbook.sheets.iter()) {
        let rel = rels_map.get(&entry.rel_id);
        let is_worksheet = rel.map(|r| r.type_.as_str()) == Some(relationship_types::WORKSHEET);
        if is_worksheet {
            let Some(part) = stub.part_name.as_ref() else {
                out.push(Some(stub_worksheet(stub)));
                continue;
            };
            let mut ws_ctx = WorksheetContext {
                shared_strings: workbook.shared_strings.clone(),
                xf_map: workbook.xf_map.clone(),
                date1904: workbook.date1904,
                palette: workbook.palette.clone(),
                dxfs: workbook.dxfs.clone(),
                dynamic_cells: dynamic_cells.clone(),
                warnings: Vec::new(),
            };
            let ws = parse_worksheet(
                &mut ws_ctx,
                &mut pkg,
                part,
                &stub.name,
                stub.sheet_id,
                stub.state,
            );
            out.push(Some(ws));
        } else {
            out.push(Some(stub_worksheet(stub)));
        }
    }
    Ok(out)
}

/// Parse one worksheet by workbook sheet index (lazy viewer load).
pub fn parse_worksheet_at(
    bytes: &[u8],
    workbook: &WorkbookSlice,
    sheet_index: usize,
) -> Result<ActiveWorksheet, String> {
    let stub = workbook
        .sheets
        .get(sheet_index)
        .ok_or_else(|| format!("sheet index {} out of range", sheet_index))?;
    let mut pkg = Package::open(bytes).map_err(|e| e.to_string())?;
    let workbook_part = pkg
        .target_of_type("", relationship_types::OFFICE_DOCUMENT)
        .map_err(|e| e.to_string())?
        .filter(|p| pkg.has(p))
        .or_else(|| {
            if pkg.has("xl/workbook.xml") {
                Some("xl/workbook.xml".into())
            } else {
                None
            }
        })
        .ok_or_else(|| "XLSX package has no workbook part (xl/workbook.xml)".to_owned())?;
    let dynamic_cells = load_dynamic_cells(&mut pkg, &workbook_part);
    let part = stub
        .part_name
        .as_ref()
        .ok_or_else(|| format!("sheet \"{}\" has no worksheet part", stub.name))?;
    let mut ws_ctx = WorksheetContext {
        shared_strings: workbook.shared_strings.clone(),
        xf_map: workbook.xf_map.clone(),
        date1904: workbook.date1904,
        palette: workbook.palette.clone(),
        dxfs: workbook.dxfs.clone(),
        dynamic_cells,
        warnings: Vec::new(),
    };
    Ok(parse_worksheet(
        &mut ws_ctx,
        &mut pkg,
        part,
        &stub.name,
        stub.sheet_id,
        stub.state,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CellAddress;

    #[test]
    fn print_area_formula_match() {
        // Catches: Print_Area formulas with quoted sheet names containing doubled quotes must still lift.
        assert_eq!(
            match_print_area_formula("'O''Brien''s'!$A$1:$B$2"),
            Some("$A$1:$B$2")
        );
        assert_eq!(
            match_print_area_formula("Sheet1!$A$1:$C$3"),
            Some("$A$1:$C$3")
        );
        assert!(match_print_area_formula("Sheet1!$A$1,Sheet1!$C$1").is_none());
    }

    #[test]
    fn parse_range_whole_column() {
        // Catches: whole-column Print_Area refs (A:C) must expand to MAX_ROW, not fail the lift.
        let r = parse_range("A:C").expect("range");
        assert_eq!(r.start, CellAddress { row: 0, col: 0 });
        assert_eq!(r.end.col, 2);
        assert_eq!(r.end.row, crate::address::MAX_ROW);
    }
}
