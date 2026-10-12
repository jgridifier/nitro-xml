// SPDX-License-Identifier: MIT
//! Workbook writer (#68): dirty saves rebuild modeled parts; a clean host returns the original bytes.

mod dynamic_array;
mod fragments;
mod shared_strings;
mod sheet_data;
mod sheet_props;
mod stale;
mod style_xml;
mod styles;
mod theme;
mod workbook_part;
mod worksheet;
mod xml_out;

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::opc::{is_signature_part, relationship_types as rt};
use nitro_xml_opc::opc::{
    relationships_part_for, resolve_part_path, Package, RelMode, Relationship,
};
use nitro_xml_opc::write::PackageWriter;
use nitro_xml_opc::xml::ns;

use crate::edit::shift::Axis;
use crate::edit::XlsxHost;
use crate::metadata::parse_dynamic_array_metadata;
use crate::model::SheetState;

use self::dynamic_array::DYNAMIC_ARRAY_METADATA_XML;
use self::shared_strings::SharedStringTable;
use self::styles::StyleWriter;
use self::theme::theme_part;
use self::workbook_part::{workbook_xml, SheetRef};
use self::worksheet::{worksheet_xml, SheetWriteExtra};
use self::xml_out::{escape_attr, relative_target};

pub(crate) use stale::stale_formula_cells;

const CT_WORKSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
const CT_WORKBOOK: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const CT_WORKBOOK_MACRO: &str = "application/vnd.ms-excel.sheet.macroEnabled.main+xml";
const CT_SST: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml";
const CT_STYLES: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml";
const CT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
const CT_CHARTSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.chartsheet+xml";
const CT_DIALOGSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.dialogsheet+xml";
const CT_MACROSHEET: &str = "application/vnd.ms-excel.macrosheet+xml";
const CT_SHEET_METADATA: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml";

const SIG_ORIGIN: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/digital-signature/origin";

/// Options for [`save_xlsx_with_options`].
#[derive(Debug, Clone)]
pub struct XlsxSaveOptions {
    /// When true (default), a dirty save sets `calcPr fullCalcOnLoad="1"`.
    pub full_calc_on_load: bool,
}

impl Default for XlsxSaveOptions {
    fn default() -> Self {
        Self {
            full_calc_on_load: true,
        }
    }
}

/// Save `host` over `original` package bytes.
///
/// A host with no dirty sheets, styles, or workbook structure returns `original` unchanged
/// ([`PackageWriter::finish`] identity shortcut). A dirty save rewrites worksheets, shared
/// strings, styles, workbook, theme, and relationships, drops `calcChain`, and carries the rest.
pub fn save_xlsx(host: &XlsxHost, original: &[u8]) -> Result<Vec<u8>, String> {
    save_xlsx_with_options(host, original, &XlsxSaveOptions::default())
}

/// [`save_xlsx`] with an explicit `fullCalcOnLoad` choice. The omit flag is the negative control.
pub fn save_xlsx_with_options(
    host: &XlsxHost,
    original: &[u8],
    options: &XlsxSaveOptions,
) -> Result<Vec<u8>, String> {
    if !host.is_dirty() {
        return PackageWriter::open(original)
            .map_err(|e| e.to_string())?
            .finish()
            .map_err(|e| e.to_string());
    }
    if host.sheets.is_empty() {
        return Err("A workbook needs at least one sheet".into());
    }

    let mut package = Package::open(original).map_err(|e| e.to_string())?;
    let book_part = package
        .target_of_type("", rt::OFFICE_DOCUMENT)
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "xl/workbook.xml".to_owned());
    let book_rels = owned_rels(&mut package, &book_part)?;
    let root_rels = owned_rels(&mut package, "")?;
    let source_parts: HashSet<String> = package.part_names().map(str::to_owned).collect();
    let mut part_ct: HashMap<String, String> = HashMap::new();
    for name in &source_parts {
        if let Some(ct) = package.content_type(name) {
            part_ct.insert(name.clone(), ct.to_owned());
        }
    }
    let styles_part = rel_target(&book_part, &book_rels, rt::STYLES);
    let strings_part = rel_target(&book_part, &book_rels, rt::SHARED_STRINGS);
    let theme_source_part = rel_target(&book_part, &book_rels, rt::THEME);
    let styles_xml = part_text(&mut package, styles_part.as_deref())?;
    let theme_xml = part_text(&mut package, theme_source_part.as_deref())?;
    let metadata_source = rel_target(&book_part, &book_rels, rt::SHEET_METADATA);
    let metadata_xml = part_text(&mut package, metadata_source.as_deref())?;
    let workbook_xml_src = part_text(&mut package, Some(&book_part))?;
    let mut sheet_sources: HashMap<String, String> = HashMap::new();
    for sheet in &host.sheets {
        if let Some(name) = &sheet.part_name {
            if let Some(text) = part_text(&mut package, Some(name))? {
                sheet_sources.insert(name.clone(), text);
            }
        }
    }
    drop(package);

    let mut writer = PackageWriter::open(original).map_err(|e| e.to_string())?;
    let mut rels = RelSet::new();
    let mut kept_vba = false;
    let mut keep_parts: HashSet<String> = HashSet::new();
    keep_parts.insert(book_part.clone());
    for rel in &book_rels {
        if is_regenerated(&rel.type_) {
            continue;
        }
        if rel.type_ == rt::VBA_PROJECT {
            if host.workbook.format != "xlsm" {
                continue;
            }
            kept_vba = true;
        }
        rels.keep(rel);
        if let Some(target) = internal_target(&book_part, rel) {
            keep_parts.insert(target);
        }
    }

    let mut states: Vec<SheetState> = host.sheets.iter().map(|s| s.state).collect();
    if !states.contains(&SheetState::Visible) {
        states[0] = SheetState::Visible;
    }
    let mut active = host.workbook.active_sheet.min(states.len() as u32 - 1);
    if states[active as usize] != SheetState::Visible {
        active = states
            .iter()
            .position(|s| *s == SheetState::Visible)
            .unwrap_or(0) as u32;
    }

    let mut claimed: HashSet<String> = HashSet::new();
    let mut assigned: Vec<Option<String>> = Vec::with_capacity(host.sheets.len());
    for sheet in &host.sheets {
        let reused = sheet.part_name.as_ref().filter(|own| {
            !claimed.contains(own.as_str()) && part_reusable(own, &source_parts, &part_ct)
        });
        if let Some(own) = reused {
            claimed.insert(own.clone());
            assigned.push(Some(own.clone()));
        } else {
            assigned.push(None);
        }
    }

    let style_count = host.workbook.styles.len();
    let dynamic_cm = dynamic_cm_index(metadata_xml.as_deref());
    let mut strings = SharedStringTable::default();
    let mut dynamic_used = false;
    let mut sheet_refs = Vec::with_capacity(host.sheets.len());
    for (index, sheet) in host.sheets.iter().enumerate() {
        let part_name = match assigned[index].clone() {
            Some(name) => name,
            None => {
                let name = unique_sheet_name(&source_parts, &claimed);
                claimed.insert(name.clone());
                name
            }
        };
        let special = sheet_rel_for_ct(part_ct.get(&part_name).map(String::as_str));
        if special.is_none() {
            let shifts: Vec<_> = host
                .axis_shifts
                .iter()
                .filter(|rec| rec.sheet == index as u32)
                .map(|rec| crate::edit::shift::AxisShift {
                    axis: if rec.row { Axis::Row } else { Axis::Col },
                    at: rec.at,
                    count: rec.count,
                })
                .collect();
            let xml = worksheet_xml(
                sheet,
                index as u32 == active,
                style_count,
                &mut strings,
                sheet_sources.get(&part_name).map(String::as_str),
                SheetWriteExtra {
                    shifts: &shifts,
                    dynamic_cm,
                    dynamic_used: &mut dynamic_used,
                },
            );
            writer.add_text(&part_name, xml, Some(CT_WORKSHEET));
        }
        keep_parts.insert(part_name.clone());
        let rel_type = special.unwrap_or(rt::WORKSHEET);
        let rel_id = rels.add(rel_type, relative_target(&book_part, &part_name), false);
        sheet_refs.push(SheetRef {
            name: sheet.name.clone(),
            sheet_id: sheet.sheet_id,
            state: states[index],
            rel_id,
        });
    }

    let out_strings = strings_part
        .clone()
        .unwrap_or_else(|| "xl/sharedStrings.xml".into());
    if !strings.is_empty() {
        writer.add_text(&out_strings, strings.xml(), Some(CT_SST));
        rels.add(
            rt::SHARED_STRINGS,
            relative_target(&book_part, &out_strings),
            false,
        );
        keep_parts.insert(out_strings);
    }
    let out_styles = styles_part
        .clone()
        .unwrap_or_else(|| "xl/styles.xml".into());
    let mut style_writer = StyleWriter::new(
        &host.workbook.styles,
        &host.workbook.named_styles,
        styles_xml.as_deref(),
    );
    writer.add_text(&out_styles, style_writer.xml(), Some(CT_STYLES));
    rels.add(rt::STYLES, relative_target(&book_part, &out_styles), false);
    keep_parts.insert(out_styles);

    let out_theme = theme_source_part
        .clone()
        .unwrap_or_else(|| "xl/theme/theme1.xml".into());
    if let Some(xml) = theme_part(&host.workbook.theme, theme_xml.as_deref()) {
        writer.add_text(&out_theme, xml, Some(CT_THEME));
    }
    rels.add(rt::THEME, relative_target(&book_part, &out_theme), false);
    keep_parts.insert(out_theme);

    write_metadata(
        &mut writer,
        &mut rels,
        &book_part,
        &mut keep_parts,
        metadata_source.as_deref(),
        dynamic_used,
    );

    let book_ct = if kept_vba {
        CT_WORKBOOK_MACRO
    } else {
        CT_WORKBOOK
    };
    writer.add_text(
        &book_part,
        workbook_xml(
            &host.workbook,
            &sheet_refs,
            active,
            workbook_xml_src.as_deref(),
            options.full_calc_on_load,
        ),
        Some(book_ct),
    );
    writer.add_text(relationships_part_for(&book_part), rels.xml(), None);

    let mut root = RelSet::new();
    root.add(rt::OFFICE_DOCUMENT, book_part.clone(), false);
    root.add(rt::CORE_PROPERTIES, "docProps/core.xml".into(), false);
    root.add(rt::EXTENDED_PROPERTIES, "docProps/app.xml".into(), false);
    for rel in &root_rels {
        if is_root_regenerated(&rel.type_) {
            continue;
        }
        let target = internal_target("", rel);
        if rel.type_ == SIG_ORIGIN || target.as_deref().is_some_and(is_signature_part) {
            continue;
        }
        if let Some(target) = target {
            if target != "docProps/core.xml" && target != "docProps/app.xml" {
                root.add(
                    &rel.type_,
                    rel.target.clone(),
                    rel.mode == RelMode::External,
                );
            }
        }
    }
    writer.add_text("_rels/.rels", root.xml(), None);

    let mut dropped: Vec<String> = Vec::new();
    for rel in &book_rels {
        if !(is_sheet_rel(&rel.type_)
            || rel.type_ == rt::CALC_CHAIN
            || rel.type_ == rt::PERSON
            || rel.type_ == rt::SHEET_METADATA
            || rel.type_ == rt::SHARED_STRINGS
            || rel.type_ == rt::STYLES
            || rel.type_ == rt::THEME)
        {
            continue;
        }
        if let Some(target) = internal_target(&book_part, rel) {
            if !keep_parts.contains(&target) {
                dropped.push(target);
            }
        }
    }
    if !dropped.is_empty() {
        {
            let ct = writer.content_types_mut();
            for name in &dropped {
                ct.overrides.remove(&format!("/{name}"));
                let rels_name = relationships_part_for(name);
                ct.overrides.remove(&format!("/{rels_name}"));
            }
        }
        for name in &dropped {
            writer.remove(name);
            writer.remove(&relationships_part_for(name));
        }
    }

    writer.finish().map_err(|e| e.to_string())
}

/// Formula cells on `sheet` whose cached values must be recomputed, sorted by row then column.
pub fn stale_cells(host: &XlsxHost, sheet: u32) -> Vec<(u32, u32)> {
    host.stale_cells(sheet)
}

fn dynamic_cm_index(source: Option<&str>) -> u32 {
    source
        .map(|xml| parse_dynamic_array_metadata(Some(xml)))
        .and_then(|set| set.into_iter().min())
        .unwrap_or(1)
}

/// Keeps a source `xl/metadata.xml` (so existing `cm` / `vm` indexes stay valid).
/// A workbook with no metadata part gets the XLDAPR part when a dynamic-array formula was written.
fn write_metadata(
    writer: &mut PackageWriter<'_>,
    rels: &mut RelSet,
    book_part: &str,
    keep_parts: &mut HashSet<String>,
    source_part: Option<&str>,
    dynamic_used: bool,
) {
    if let Some(part) = source_part {
        keep_parts.insert(part.to_owned());
        rels.add(rt::SHEET_METADATA, relative_target(book_part, part), false);
        return;
    }
    if !dynamic_used {
        return;
    }
    let part = "xl/metadata.xml".to_owned();
    writer.add_text(&part, DYNAMIC_ARRAY_METADATA_XML, Some(CT_SHEET_METADATA));
    rels.add(rt::SHEET_METADATA, relative_target(book_part, &part), false);
    keep_parts.insert(part);
}

fn owned_rels(package: &mut Package<'_>, part: &str) -> Result<Vec<Relationship>, String> {
    let rels = package.rels(part).map_err(|e| e.to_string())?;
    Ok(rels.values().cloned().collect())
}

fn part_text(package: &mut Package<'_>, name: Option<&str>) -> Result<Option<String>, String> {
    let Some(name) = name else {
        return Ok(None);
    };
    match package.text(name).map_err(|e| e.to_string())? {
        Some(text) => Ok(Some(text.to_owned())),
        None => Ok(None),
    }
}

fn rel_target(book: &str, rels: &[Relationship], type_: &str) -> Option<String> {
    rels.iter()
        .find(|rel| rel.type_ == type_ && rel.mode == RelMode::Internal)
        .map(|rel| resolve_part_path(book, &rel.target))
}

fn internal_target(base: &str, rel: &Relationship) -> Option<String> {
    if rel.mode == RelMode::External {
        None
    } else {
        Some(resolve_part_path(base, &rel.target))
    }
}

fn is_regenerated(type_: &str) -> bool {
    type_ == rt::WORKSHEET
        || type_ == rt::CHARTSHEET
        || type_ == rt::DIALOGSHEET
        || type_ == rt::XL_MACROSHEET
        || type_ == rt::STYLES
        || type_ == rt::SHARED_STRINGS
        || type_ == rt::THEME
        || type_ == rt::CALC_CHAIN
        || type_ == rt::PERSON
        || type_ == rt::SHEET_METADATA
}

fn is_sheet_rel(type_: &str) -> bool {
    type_ == rt::WORKSHEET
        || type_ == rt::CHARTSHEET
        || type_ == rt::DIALOGSHEET
        || type_ == rt::XL_MACROSHEET
}

fn is_root_regenerated(type_: &str) -> bool {
    type_ == rt::OFFICE_DOCUMENT || type_ == rt::CORE_PROPERTIES || type_ == rt::EXTENDED_PROPERTIES
}

fn sheet_rel_for_ct(ct: Option<&str>) -> Option<&'static str> {
    match ct {
        Some(CT_CHARTSHEET) => Some(rt::CHARTSHEET),
        Some(CT_DIALOGSHEET) => Some(rt::DIALOGSHEET),
        Some(CT_MACROSHEET) => Some(rt::XL_MACROSHEET),
        _ => None,
    }
}

fn part_reusable(
    name: &str,
    source_parts: &HashSet<String>,
    part_ct: &HashMap<String, String>,
) -> bool {
    if !source_parts.contains(name) {
        return true;
    }
    let ct = part_ct.get(name).map(String::as_str);
    ct == Some(CT_WORKSHEET) || sheet_rel_for_ct(ct).is_some()
}

fn unique_sheet_name(reserved: &HashSet<String>, claimed: &HashSet<String>) -> String {
    let mut n = 1u32;
    loop {
        let name = format!("xl/worksheets/sheet{n}.xml");
        if !reserved.contains(&name) && !claimed.contains(&name) {
            return name;
        }
        n += 1;
    }
}

struct RelItem {
    type_: String,
    target: String,
    external: bool,
}

struct RelSet {
    entries: Vec<(String, RelItem)>,
}

impl RelSet {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn keep(&mut self, rel: &Relationship) {
        self.entries.push((
            rel.id.clone(),
            RelItem {
                type_: rel.type_.clone(),
                target: rel.target.clone(),
                external: rel.mode == RelMode::External,
            },
        ));
    }

    fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|(existing, _)| existing == id)
    }

    fn add(&mut self, type_: &str, target: String, external: bool) -> String {
        let mut n = self.entries.len() + 1;
        while self.contains(&format!("rId{n}")) {
            n += 1;
        }
        let id = format!("rId{n}");
        self.entries.push((
            id.clone(),
            RelItem {
                type_: type_.to_owned(),
                target,
                external,
            },
        ));
        id
    }

    fn xml(&self) -> String {
        let items: String = self
            .entries
            .iter()
            .map(|(id, rel)| {
                let mode = if rel.external {
                    " TargetMode=\"External\""
                } else {
                    ""
                };
                format!(
                    "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{mode}/>",
                    escape_attr(id),
                    escape_attr(&rel.type_),
                    escape_attr(&rel.target),
                )
            })
            .collect();
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"{}\">{items}</Relationships>",
            ns::RELS
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::cells::write_input;
    use crate::model::{DumpCell, DumpCellValue};
    use std::path::Path;

    fn cells_1k() -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/generated/xlsx/cells_1k.xlsx");
        std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    // Catches: saving with no edits rebuilds the zip instead of returning the original bytes.
    #[test]
    fn clean_save_returns_original_bytes() {
        let bytes = cells_1k();
        let host = XlsxHost::open(&bytes).expect("open");
        let saved = save_xlsx(&host, &bytes).expect("save");
        assert_eq!(saved, bytes);
    }

    // Catches: a value edit does not mark formula dependents stale, and a dirty save omits fullCalcOnLoad="1".
    #[test]
    fn value_edit_stales_dependents_and_sets_full_calc() {
        let bytes = cells_1k();
        let mut host = XlsxHost::open(&bytes).expect("open");
        {
            let sheet = host.sheet_mut(0).expect("sheet");
            if let Some(cell) = sheet.cells.iter_mut().find(|c| c.row == 0 && c.col == 2) {
                cell.formula = Some("A1+1".into());
                cell.value = DumpCellValue::Null;
            } else {
                sheet.cells.push(DumpCell {
                    row: 0,
                    col: 2,
                    value: DumpCellValue::Null,
                    type_tag: None,
                    formula: Some("A1+1".into()),
                    style_id: None,
                    rich_text: None,
                    array_range: None,
                    legacy_formula: None,
                    dynamic_array: None,
                    spill_anchor: None,
                    cell_metadata: None,
                    value_metadata: None,
                });
            }
        }
        write_input(&mut host, 0, 0, 0, "99").expect("edit");
        let stale = stale_cells(&host, 0);
        assert!(stale.contains(&(0, 2)), "C1 should be stale: {stale:?}");
        assert!(
            !stale.contains(&(0, 0)),
            "A1 is the precedent, not a formula"
        );
        let saved = save_xlsx(&host, &bytes).expect("save");
        let mut pkg = Package::open(&saved).expect("reopen");
        let xml = pkg
            .text("xl/workbook.xml")
            .expect("read")
            .expect("workbook");
        assert!(xml.contains("fullCalcOnLoad=\"1\""));
        let omitted = save_xlsx_with_options(
            &host,
            &bytes,
            &XlsxSaveOptions {
                full_calc_on_load: false,
            },
        )
        .expect("omit");
        let mut pkg = Package::open(&omitted).expect("reopen omit");
        let xml = pkg
            .text("xl/workbook.xml")
            .expect("read")
            .expect("workbook");
        assert!(!xml.contains("fullCalcOnLoad"));
    }
}
