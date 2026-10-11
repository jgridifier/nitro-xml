// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/load.ts, workbook-part.ts,
// formula-text.ts (stripFuturePrefixes), and defaults from src/core/xlsx/workbook.ts; changes
// listed in PROVENANCE.md.
//! Workbook part parse, Stage-3 load orchestration, and defaults.

use std::collections::HashMap;

use nitro_xml_opc::opc::{detect_digital_signatures, relationship_types, Package, Relationship};
use nitro_xml_opc::xml::parse_xml_str;

use crate::address::parse_range;
use crate::model::{
    Border, CellStyle, Color, DefinedName, Fill, Font, ModernPasswordHash, PageSetup, SheetState,
    SheetStub, ThemePalette, WorkbookSlice,
};
use crate::password_hash::read_modern_hash;
use crate::shared_strings::parse_shared_strings;
use crate::styles::parse_styles;
use crate::theme::parse_theme;
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

struct SheetEntry {
    name: String,
    sheet_id: u32,
    state: SheetState,
    rel_id: String,
}

struct WorkbookPart {
    sheets: Vec<SheetEntry>,
    defined_names: Vec<DefinedName>,
    date1904: bool,
    active_tab: u32,
    structure_locked: bool,
    full_calc_on_load: bool,
    workbook_password_hash: Option<String>,
    workbook_modern_hash: Option<ModernPasswordHash>,
    calc_mode: Option<String>,
    iterate: Option<(f64, f64)>,
}

fn push_warn(warnings: &mut Vec<String>, message: String) {
    if !warnings.contains(&message) {
        warnings.push(message);
    }
}

const PRINT_AREA: &str = "_xlnm.Print_Area";

const CONTENT_TYPE_WORKBOOK_MACRO: &str = "application/vnd.ms-excel.sheet.macroEnabled.main+xml";
const CONTENT_TYPE_TEMPLATE_MACRO: &str = "application/vnd.ms-excel.template.macroEnabled.main+xml";

/// Removes `_xlfn.` / `_xlws.` function prefixes (and turns `ANCHORARRAY(A1)` into `A1#`).
fn strip_future_prefixes(formula: &str) -> String {
    if !formula.contains("_xl") {
        return formula.to_owned();
    }
    let plain = rewrite_formula(formula, |token, next, out| {
        if !next.starts_with('(') {
            return false;
        }
        let stripped = strip_xl_prefix(token);
        if stripped == token {
            return false;
        }
        out.push_str(stripped);
        true
    });
    from_stored_syntax(&plain)
}

fn strip_xl_prefix(token: &str) -> &str {
    let mut s = token;
    if s.len() >= 6 && s[..6].eq_ignore_ascii_case("_xlfn.") {
        s = &s[6..];
    }
    if s.len() >= 6 && s[..6].eq_ignore_ascii_case("_xlws.") {
        s = &s[6..];
    }
    s
}

fn rewrite_formula(
    formula: &str,
    mut visit: impl FnMut(&str, &str, &mut String) -> bool,
) -> String {
    let chars: Vec<char> = formula.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' || ch == '\'' {
            let quote = ch;
            let mut j = i + 1;
            while j < chars.len() {
                if chars[j] == quote {
                    if j + 1 < chars.len() && chars[j + 1] == quote {
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                j += 1;
            }
            out.extend(chars[i..j].iter());
            i = j;
            continue;
        }
        if ch == '[' {
            let mut depth = 0i32;
            let mut j = i;
            while j < chars.len() {
                if chars[j] == '[' {
                    depth += 1;
                } else if chars[j] == ']' {
                    depth -= 1;
                    if depth == 0 {
                        j += 1;
                        break;
                    }
                }
                j += 1;
            }
            out.extend(chars[i..j].iter());
            i = j;
            continue;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '\\' | '.') {
            let mut j = i;
            while j < chars.len() {
                let c = chars[j];
                if c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '\\' | '.' | '?') {
                    j += 1;
                } else {
                    break;
                }
            }
            let token: String = chars[i..j].iter().collect();
            let next: String = chars[j..].iter().collect();
            if !visit(&token, &next, &mut out) {
                out.push_str(&token);
            }
            i = j;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// Minimal `ANCHORARRAY(x)` → `x#` and `SINGLE(x)` → `@x`.
fn from_stored_syntax(formula: &str) -> String {
    let upper = formula.to_ascii_uppercase();
    if !upper.contains("ANCHORARRAY") && !upper.contains("SINGLE") {
        return formula.to_owned();
    }
    replace_call(formula, "ANCHORARRAY", |inner| format!("{inner}#"))
        .pipe(|s| replace_call(&s, "SINGLE", |inner| format!("@{inner}")))
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R;
}
impl<T> Pipe for T {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}

fn replace_call(formula: &str, name: &str, rewrite: impl Fn(&str) -> String) -> String {
    let chars: Vec<char> = formula.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if i + name_chars.len() < chars.len()
            && chars[i..i + name_chars.len()]
                .iter()
                .zip(name_chars.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
            && chars.get(i + name_chars.len()) == Some(&'(')
        {
            let start_inner = i + name_chars.len() + 1;
            let mut depth = 1i32;
            let mut j = start_inner;
            while j < chars.len() && depth > 0 {
                if chars[j] == '(' {
                    depth += 1;
                } else if chars[j] == ')' {
                    depth -= 1;
                }
                if depth > 0 {
                    j += 1;
                }
            }
            if depth == 0 {
                let inner: String = chars[start_inner..j].iter().collect();
                out.push_str(&rewrite(&inner));
                i = j + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn parse_workbook_part(xml: &str) -> Result<WorkbookPart, String> {
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

fn lift_print_areas(workbook: &mut WorkbookSlice) {
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

/// Load Stage-3 workbook slice (workbook / styles / theme / shared strings; sheet stubs only).
pub fn load_workbook(bytes: &[u8]) -> Result<WorkbookSlice, String> {
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
    let strings_xml = match &strings_part {
        Some(p) => pkg.text(p).map_err(|e| e.to_string())?.map(str::to_owned),
        None => None,
    };
    let shared_strings = parse_shared_strings(strings_xml.as_deref(), styles.palette.as_deref());

    let rels_map: HashMap<String, Relationship> = pkg
        .rels(&workbook_part)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let mut sheets: Vec<SheetStub> = Vec::new();
    for entry in &book.sheets {
        let rel = rels_map.get(&entry.rel_id);
        let target = rel.and_then(|r| pkg.target(&workbook_part, r));
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
        shared_strings,
        warnings,
        full_calc_on_load: book.full_calc_on_load.then_some(true),
        structure_locked: book.structure_locked.then_some(true),
        calc_mode: book.calc_mode,
        iterate: book.iterate,
        workbook_password_hash: book.workbook_password_hash,
        workbook_modern_hash: book.workbook_modern_hash,
    };
    lift_print_areas(&mut workbook);
    Ok(workbook)
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
