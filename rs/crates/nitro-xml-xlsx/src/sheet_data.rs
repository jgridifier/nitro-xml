// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/sheet-data.ts; changes listed in PROVENANCE.md.
//! Read `<sheetData>` into the columnar cell store.

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::xml::{parse_xml_str, Element};
use quick_xml::escape::resolve_xml_entity;
use quick_xml::events::{BytesRef, Event};
use quick_xml::reader::Reader;

use crate::address::{parse_address, parse_range};
use crate::cells::{CellStore, CellTypeTag};
use crate::dates::iso_to_serial;
use crate::formula_text::strip_future_prefixes;
use crate::formula_translate::translate_formula;
use crate::model::{CellAddress, CellRange, DumpCell, DumpCellValue, RowInfo, SharedString};
use crate::shared_strings::parse_rich_string;
use crate::xml_util::{att, num_attr, x_children, x_first, x_text};

pub const ERROR_CODES: &[&str] = &[
    "#NULL!",
    "#DIV/0!",
    "#VALUE!",
    "#REF!",
    "#NAME?",
    "#NUM!",
    "#N/A",
    "#GETTING_DATA",
    "#SPILL!",
    "#CALC!",
];

pub fn is_error_code(value: &str) -> bool {
    ERROR_CODES.contains(&value)
}

pub struct CellContext<'a> {
    pub shared_strings: &'a [SharedString],
    pub xf_map: &'a [u32],
    pub date1904: bool,
    pub palette: Option<&'a [String]>,
    pub dynamic_cells: &'a HashSet<u32>,
    pub warnings: &'a mut Vec<String>,
}

impl<'a> CellContext<'a> {
    pub fn warn(&mut self, message: impl AsRef<str>) {
        let message = message.as_ref().to_owned();
        if !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }
}

struct SharedMaster {
    row: u32,
    col: u32,
    formula: String,
}

struct DynamicAnchor {
    row: u32,
    col: u32,
    range: CellRange,
}

fn style_of(ctx: &CellContext<'_>, element: Option<&Element>) -> u32 {
    let s = num_attr(element, "s");
    match s {
        None => 0,
        Some(s) => ctx.xf_map.get(s as usize).copied().unwrap_or(0),
    }
}

/// Stream-parse inner `sheetData` XML (rows and cells only).
///
/// Prefer a pull parse into [`CellStore`] so a full Element tree for every `<c>`/`<v>` is not
/// retained (the HWM driver on million-cell sheets). Falls back to the DOM path when the
/// stream sees `inlineStr` / nested markup that needs the rich-string tree.
///
/// DOM fallback is outlined ([`read_sheet_data_xml_dom`]) so xlsx S first-open TurboFan does
/// not compile wrap+parse into the stream entry (peak_mem compile floor).
pub fn read_sheet_data_xml(
    ctx: &mut CellContext<'_>,
    sheet_data_inner: &str,
    store: &mut CellStore,
    row_info: &mut HashMap<u32, RowInfo>,
) {
    // Stream path first at every size: no wrapper copy, no DOM tree, no per-element name compares.
    // It declines (returns false) for `inlineStr` / rich-string markup; those take the DOM path.
    if read_sheet_data_xml_stream(ctx, sheet_data_inner, store, row_info) {
        return;
    }
    read_sheet_data_xml_dom(ctx, sheet_data_inner, store, row_info);
}

/// DOM fallback for `inlineStr` / rich-string markup. Cold on the gate S fixture stream path.
#[inline(never)]
fn read_sheet_data_xml_dom(
    ctx: &mut CellContext<'_>,
    sheet_data_inner: &str,
    store: &mut CellStore,
    row_info: &mut HashMap<u32, RowInfo>,
) {
    // Lazy viewport rows keep `x14ac:dyDescent` (and similar) without the outer worksheet
    // xmlns declarations, so the wrapper must re-declare x14ac/mc or parsing drops cells.
    let wrapped = format!(
        r#"<sheetData xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">{}</sheetData>"#,
        sheet_data_inner
    );
    let doc = match parse_xml_str(&wrapped) {
        Ok(d) => d,
        Err(_) => return,
    };
    read_sheet_data_element(ctx, &doc.root, store, row_info);
}

/// Pull-parse `sheetData` inner XML. Returns `false` when the caller should use the DOM path.
#[inline(never)]
fn read_sheet_data_xml_stream(
    ctx: &mut CellContext<'_>,
    sheet_data_inner: &str,
    store: &mut CellStore,
    row_info: &mut HashMap<u32, RowInfo>,
) -> bool {
    // Reject the rare cases the stream path does not handle yet.
    if sheet_data_inner.contains("inlineStr") || sheet_data_inner.contains("<is") {
        return false;
    }

    // Inner XML is the `<sheetData>` children only (rows); no extra wrapper copy on the hot path.
    // Plain `Reader`: cells are matched by local name, so namespace resolution is wasted work.
    let mut reader = Reader::from_str(sheet_data_inner);
    reader.config_mut().trim_text(false);

    let mut masters: HashMap<String, SharedMaster> = HashMap::new();
    let mut anchors: Vec<DynamicAnchor> = Vec::new();
    let mut row_index: i64 = -1;
    let mut col_index: i64 = -1;
    let mut in_row = false;
    let mut in_c = false;
    let mut in_v = false;
    let mut in_f = false;
    let mut row_u = 0u32;
    let mut cell_row = 0u32;
    let mut cell_col = 0u32;
    let mut cell_style = 0u32;
    let mut cell_type_buf = Vec::<u8>::with_capacity(8);
    let mut cell_cm: Option<u32> = None;
    let mut cell_vm: Option<u32> = None;
    let mut v_text = String::new();
    let mut f_text = String::new();
    let mut f_kind_buf = Vec::<u8>::with_capacity(8);
    let mut f_si = String::new();
    let mut f_ref: Option<String> = None;
    // Reused so attribute scans do not allocate per cell.
    let mut attr_scratch = Vec::<u8>::with_capacity(32);

    loop {
        let event = match reader.read_event() {
            Ok(e) => e,
            Err(_) => return false,
        };
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let is_empty = matches!(event, Event::Empty(_));
                let local_owned = e.local_name();
                let local = local_owned.as_ref();
                match local {
                    b"row" if !in_c => {
                        in_row = true;
                        let r_attr = attr_u32(e, b"r", &mut attr_scratch);
                        row_index = if let Some(r) = r_attr {
                            r as i64 - 1
                        } else {
                            row_index + 1
                        };
                        row_u = row_index as u32;
                        col_index = -1;
                        if let Some(info) = row_info_from_attrs(ctx, e, &mut attr_scratch) {
                            row_info.insert(row_u, info);
                        }
                        if is_empty {
                            in_row = false;
                        }
                    }
                    b"c" if in_row => {
                        in_c = true;
                        v_text.clear();
                        f_text.clear();
                        f_kind_buf.clear();
                        f_kind_buf.extend_from_slice(b"normal");
                        f_si.clear();
                        f_ref = None;
                        cell_type_buf.clear();
                        // One pass over the attributes (was five scans, each re-checking duplicates).
                        let mut s_attr = None;
                        let mut address = None;
                        let mut has_t = false;
                        cell_cm = None;
                        cell_vm = None;
                        for a in e.attributes().with_checks(false).flatten() {
                            let v = a.value.as_ref();
                            match local_name(a.key.as_ref()) {
                                b"t" if !has_t => {
                                    has_t = true;
                                    cell_type_buf.extend_from_slice(v);
                                }
                                b"s" if s_attr.is_none() => s_attr = parse_u32(v),
                                b"cm" if cell_cm.is_none() => cell_cm = parse_u32(v),
                                b"vm" if cell_vm.is_none() => cell_vm = parse_u32(v),
                                b"r" if address.is_none() => {
                                    address = std::str::from_utf8(v).ok().and_then(parse_address)
                                }
                                _ => {}
                            }
                        }
                        if !has_t {
                            cell_type_buf.extend_from_slice(b"n");
                        }
                        cell_style = ctx
                            .xf_map
                            .get(s_attr.unwrap_or(0) as usize)
                            .copied()
                            .unwrap_or(0);
                        col_index = if let Some(ref a) = address {
                            a.col as i64
                        } else {
                            col_index + 1
                        };
                        cell_row = address.as_ref().map(|a| a.row).unwrap_or(row_u);
                        cell_col = col_index as u32;
                        if is_empty {
                            let cell_type = std::str::from_utf8(&cell_type_buf).unwrap_or("n");
                            let f_kind = std::str::from_utf8(&f_kind_buf).unwrap_or("normal");
                            flush_stream_cell(
                                ctx,
                                store,
                                &mut masters,
                                &mut anchors,
                                cell_row,
                                cell_col,
                                cell_style,
                                cell_type,
                                &v_text,
                                None,
                                f_kind,
                                &f_si,
                                f_ref.as_deref(),
                                cell_cm,
                                cell_vm,
                            );
                            in_c = false;
                        }
                    }
                    b"v" if in_c => {
                        in_v = !is_empty;
                    }
                    b"f" if in_c => {
                        in_f = !is_empty;
                        f_kind_buf.clear();
                        if let Some(t) = attr_bytes(e, b"t", &mut attr_scratch) {
                            f_kind_buf.extend_from_slice(t);
                        } else {
                            f_kind_buf.extend_from_slice(b"normal");
                        }
                        f_si = attr_bytes(e, b"si", &mut attr_scratch)
                            .and_then(|b| std::str::from_utf8(b).ok())
                            .unwrap_or("")
                            .to_owned();
                        f_ref = attr_bytes(e, b"ref", &mut attr_scratch)
                            .and_then(|b| std::str::from_utf8(b).ok())
                            .map(str::to_owned);
                    }
                    _ => {}
                }
            }
            Event::Text(t) => {
                if let Ok(text) = std::str::from_utf8(t.as_ref()) {
                    if in_v {
                        push_line_normalized(&mut v_text, text);
                    } else if in_f {
                        push_line_normalized(&mut f_text, text);
                    }
                }
            }
            // quick-xml emits `&lt;` / `&amp;` / `&#38;` as GeneralRef, not Text — skipping
            // them dropped `<>` / `&` / quotes from formulas (worksheet dump parity).
            Event::GeneralRef(r) => {
                if in_v {
                    append_xml_ref(&mut v_text, &r);
                } else if in_f {
                    append_xml_ref(&mut f_text, &r);
                }
            }
            Event::CData(t) => {
                if let Ok(text) = std::str::from_utf8(t.as_ref()) {
                    if in_v {
                        push_line_normalized(&mut v_text, text);
                    } else if in_f {
                        push_line_normalized(&mut f_text, text);
                    }
                }
            }
            Event::End(e) => {
                let local_owned = e.local_name();
                let local = local_owned.as_ref();
                match local {
                    b"v" => in_v = false,
                    b"f" => in_f = false,
                    b"c" if in_c => {
                        let cell_type = std::str::from_utf8(&cell_type_buf).unwrap_or("n");
                        let f_kind = std::str::from_utf8(&f_kind_buf).unwrap_or("normal");
                        let has_f = !f_text.is_empty() || f_kind != "normal" || !f_si.is_empty();
                        flush_stream_cell(
                            ctx,
                            store,
                            &mut masters,
                            &mut anchors,
                            cell_row,
                            cell_col,
                            cell_style,
                            cell_type,
                            &v_text,
                            if has_f { Some(f_text.as_str()) } else { None },
                            f_kind,
                            &f_si,
                            f_ref.as_deref(),
                            cell_cm,
                            cell_vm,
                        );
                        in_c = false;
                    }
                    b"row" => in_row = false,
                    b"sheetData" => {
                        mark_spills(store, &anchors);
                        return true;
                    }
                    _ => {}
                }
            }
            Event::Eof => {
                mark_spills(store, &anchors);
                return true;
            }
            _ => {}
        }
    }
}

fn local_name(qname: &[u8]) -> &[u8] {
    qname
        .iter()
        .rposition(|&b| b == b':')
        .map(|i| &qname[i + 1..])
        .unwrap_or(qname)
}

/// Append `text` with XML line-end normalisation (`\r\n` and lone `\r` become `\n`), matching the
/// DOM parser. Character references (`&#13;`) arrive as `GeneralRef` and keep their `\r`.
fn push_line_normalized(out: &mut String, text: &str) {
    if !text.contains('\r') {
        out.push_str(text);
        return;
    }
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
}

/// Expand a `GeneralRef` (`&lt;`, `&amp;`, `&#38;`, …) into `out`.
fn append_xml_ref(out: &mut String, r: &BytesRef<'_>) {
    if let Ok(Some(ch)) = r.resolve_char_ref() {
        out.push(ch);
        return;
    }
    if let Ok(name) = r.decode() {
        if let Some(s) = resolve_xml_entity(name.as_ref()) {
            out.push_str(s);
        }
    }
}

/// Copy attribute value into `scratch` (cleared first). Returns a slice of `scratch`.
fn attr_bytes<'a>(
    e: &quick_xml::events::BytesStart<'_>,
    key: &[u8],
    scratch: &'a mut Vec<u8>,
) -> Option<&'a [u8]> {
    for a in e.attributes().with_checks(false).flatten() {
        if local_name(a.key.as_ref()) == key {
            scratch.clear();
            scratch.extend_from_slice(a.value.as_ref());
            return Some(scratch.as_slice());
        }
    }
    None
}

fn parse_u32(bytes: &[u8]) -> Option<u32> {
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

fn attr_u32(
    e: &quick_xml::events::BytesStart<'_>,
    key: &[u8],
    scratch: &mut Vec<u8>,
) -> Option<u32> {
    let bytes = attr_bytes(e, key, scratch)?;
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

fn attr_flag(e: &quick_xml::events::BytesStart<'_>, key: &[u8], scratch: &mut Vec<u8>) -> bool {
    attr_bytes(e, key, scratch).is_some_and(|b| b == b"1" || b.eq_ignore_ascii_case(b"true"))
}

#[inline(never)]
fn row_info_from_attrs(
    ctx: &CellContext<'_>,
    e: &quick_xml::events::BytesStart<'_>,
    scratch: &mut Vec<u8>,
) -> Option<RowInfo> {
    let mut info = RowInfo::default();
    if let Some(h) = attr_bytes(e, b"ht", scratch)
        .and_then(|b| std::str::from_utf8(b).ok())
        .and_then(|s| s.parse().ok())
    {
        info.height = Some(h);
    }
    if attr_flag(e, b"customHeight", scratch) {
        info.custom_height = Some(true);
    }
    if attr_flag(e, b"hidden", scratch) {
        info.hidden = Some(true);
    }
    if attr_flag(e, b"customFormat", scratch) {
        let s = attr_u32(e, b"s", scratch).unwrap_or(0);
        let style_id = ctx.xf_map.get(s as usize).copied().unwrap_or(0);
        if style_id > 0 {
            info.style_id = Some(style_id);
        }
    }
    if let Some(o) = attr_bytes(e, b"outlineLevel", scratch)
        .and_then(|b| std::str::from_utf8(b).ok())
        .and_then(|s| s.parse::<f64>().ok())
    {
        if o > 0.0 {
            info.outline_level = Some(o);
        }
    }
    if attr_flag(e, b"collapsed", scratch) {
        info.collapsed = Some(true);
    }
    if info == RowInfo::default() {
        None
    } else {
        Some(info)
    }
}

/// Per-cell commit from the stream loop. Outlined so V8 TurboFan on xlsx S open
/// does not compile value/formula materialise into [`read_sheet_data_xml_stream`].
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn flush_stream_cell(
    ctx: &mut CellContext<'_>,
    store: &mut CellStore,
    masters: &mut HashMap<String, SharedMaster>,
    anchors: &mut Vec<DynamicAnchor>,
    row: u32,
    col: u32,
    style_id: u32,
    type_: &str,
    raw: &str,
    f_text: Option<&str>,
    f_kind: &str,
    f_si: &str,
    f_ref: Option<&str>,
    cm: Option<u32>,
    vm: Option<u32>,
) {
    let mut cell_idx = None;
    match type_ {
        "s" => {
            let idx = raw.trim().parse::<usize>().ok();
            let entry = idx.and_then(|i| ctx.shared_strings.get(i));
            if let Some(entry) = entry {
                let sst_idx = idx.unwrap() as u32;
                let i = store.push_shared(row, col, style_id, sst_idx);
                if let Some(runs) = &entry.runs {
                    store.set_rich_text(i, runs.clone());
                }
                cell_idx = Some(i);
            } else if !raw.is_empty() {
                ctx.warn("A cell refers to a missing shared string.");
            }
        }
        "str" => {
            // Match DOM `read_value`: `t="str"` always materializes a string cell, including
            // empty `<v/>` (formula `""` cells dump as `t:"s", v:""`, not Null).
            cell_idx = Some(store.push_string(row, col, style_id, raw.to_owned()));
        }
        "b" => {
            if !raw.is_empty() {
                let b = raw.trim() == "1" || raw.trim().eq_ignore_ascii_case("true");
                cell_idx = Some(store.push_bool(row, col, style_id, b));
            }
        }
        "e" => {
            if !raw.is_empty() {
                let code = raw.trim();
                let err = if is_error_code(code) {
                    code.to_owned()
                } else {
                    "#VALUE!".to_owned()
                };
                cell_idx = Some(store.push_error(row, col, style_id, err));
            }
        }
        "d" => {
            if !raw.is_empty() {
                if let Some(serial) = iso_to_serial(raw, ctx.date1904) {
                    cell_idx = Some(store.push_number(row, col, style_id, serial, Some(raw)));
                } else {
                    ctx.warn(format!("Unreadable date cell value \"{raw}\"."));
                }
            }
        }
        _ => {
            if !raw.trim().is_empty() {
                if let Ok(n) = raw.parse::<f64>() {
                    if n.is_finite() {
                        cell_idx = Some(store.push_number(row, col, style_id, n, Some(raw)));
                    } else {
                        cell_idx = Some(store.push_string(row, col, style_id, raw.to_owned()));
                    }
                } else {
                    cell_idx = Some(store.push_string(row, col, style_id, raw.to_owned()));
                }
            }
        }
    }
    if cell_idx.is_none() && style_id > 0 {
        cell_idx = Some(store.push_style_only(row, col, style_id));
    }
    let dynamic = cm.map(|c| ctx.dynamic_cells.contains(&c)).unwrap_or(false);
    if let Some(cm) = cm {
        if !dynamic {
            if let Some(idx) = cell_idx {
                store.set_cell_metadata(idx, cm);
            }
        }
    }
    if let Some(vm) = vm {
        if let Some(idx) = cell_idx {
            store.set_value_metadata(idx, vm);
        }
    }
    if let Some(text) = f_text {
        // Build a minimal Element-like formula apply via the shared logic path.
        apply_stream_formula(
            ctx, store, masters, anchors, row, col, style_id, cell_idx, text, f_kind, f_si, f_ref,
            dynamic,
        );
    } else if let Some(idx) = cell_idx {
        if store.formula(idx).is_none() {
            store.clear_legacy_dynamic(idx);
        }
    }
}

/// Formula materialise — cold on cells_1k (no `<f>`). Keep out of flush TurboFan.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn apply_stream_formula(
    ctx: &mut CellContext<'_>,
    store: &mut CellStore,
    masters: &mut HashMap<String, SharedMaster>,
    anchors: &mut Vec<DynamicAnchor>,
    row: u32,
    col: u32,
    style_id: u32,
    cell_idx: Option<usize>,
    text_raw: &str,
    kind: &str,
    si: &str,
    ref_attr: Option<&str>,
    dynamic: bool,
) {
    let mut cell_idx = cell_idx;
    let text = strip_future_prefixes(text_raw);
    if cell_idx.is_none() && (!text.trim().is_empty() || kind != "normal") {
        cell_idx = Some(store.push_style_only(row, col, style_id));
    }
    if let Some(idx) = cell_idx {
        if dynamic {
            store.set_dynamic_array(idx);
        } else if kind != "array" {
            store.set_legacy_formula(idx);
        }
    }
    if kind == "shared" {
        if !text.trim().is_empty() {
            masters.insert(
                si.to_owned(),
                SharedMaster {
                    row,
                    col,
                    formula: text.clone(),
                },
            );
            if let Some(idx) = cell_idx {
                store.set_formula(idx, text);
            }
            return;
        }
        let Some(master) = masters.get(si) else {
            ctx.warn("A shared formula refers to a missing master cell; its formula was dropped.");
            return;
        };
        let translated = translate_formula(
            &master.formula,
            (row - master.row) as i32,
            (col - master.col) as i32,
        );
        if let Some(idx) = cell_idx {
            store.set_formula(idx, translated);
        }
        return;
    }
    if kind == "dataTable" {
        ctx.warn("What-if data table formulas are not supported; their cached values were kept.");
        if let Some(idx) = cell_idx {
            store.clear_legacy_dynamic(idx);
        }
        return;
    }
    if text.trim().is_empty() {
        return;
    }
    if let Some(idx) = cell_idx {
        store.set_formula(idx, text);
    }
    if kind == "array" {
        let range = ref_attr.and_then(parse_range);
        if dynamic {
            if let Some(range) = range {
                anchors.push(DynamicAnchor { row, col, range });
            }
        } else if let (Some(idx), Some(range)) = (cell_idx, range) {
            store.set_array_range(idx, range);
        }
    }
}

pub fn read_sheet_data_element(
    ctx: &mut CellContext<'_>,
    sheet_data: &Element,
    store: &mut CellStore,
    row_info: &mut HashMap<u32, RowInfo>,
) {
    let mut masters: HashMap<String, SharedMaster> = HashMap::new();
    let mut anchors: Vec<DynamicAnchor> = Vec::new();
    let mut row_index: i64 = -1;
    for row_el in x_children(sheet_data, "row") {
        let r = num_attr(Some(row_el), "r");
        row_index = if let Some(r) = r {
            r as i64 - 1
        } else {
            row_index + 1
        };
        let row_u = row_index as u32;
        if let Some(info) = crate::sheet_props::read_row_info(row_el, ctx.xf_map) {
            row_info.insert(row_u, info);
        }
        let mut col_index: i64 = -1;
        for c in x_children(row_el, "c") {
            let ref_ = att(Some(c), "r");
            let address = ref_.and_then(parse_address);
            col_index = if let Some(ref a) = address {
                a.col as i64
            } else {
                col_index + 1
            };
            let row = address.as_ref().map(|a| a.row).unwrap_or(row_u);
            let col = col_index as u32;
            let style_id = style_of(ctx, Some(c));
            let mut cell_idx = None;
            read_value(ctx, c, store, style_id, row, col, &mut cell_idx);
            let f = x_first(Some(c), "f");
            let cm = num_attr(Some(c), "cm");
            let dynamic = cm
                .map(|c| ctx.dynamic_cells.contains(&(c as u32)))
                .unwrap_or(false);
            if let Some(cm) = cm {
                if !dynamic {
                    if let Some(idx) = cell_idx {
                        store.set_cell_metadata(idx, cm as u32);
                    }
                }
            }
            let vm = num_attr(Some(c), "vm");
            if let Some(vm) = vm {
                if let Some(idx) = cell_idx {
                    store.set_value_metadata(idx, vm as u32);
                }
            }
            if let Some(f_el) = f {
                read_formula(
                    ctx,
                    f_el,
                    row,
                    col,
                    style_id,
                    store,
                    cell_idx,
                    &mut masters,
                    dynamic,
                    &mut anchors,
                );
            }
            if let Some(idx) = cell_idx {
                if store.formula(idx).is_none() {
                    store.clear_legacy_dynamic(idx);
                }
            }
        }
    }
    mark_spills(store, &anchors);
}

fn read_value(
    ctx: &mut CellContext<'_>,
    c: &Element,
    store: &mut CellStore,
    style_id: u32,
    row: u32,
    col: u32,
    cell_idx: &mut Option<usize>,
) {
    let type_ = att(Some(c), "t").unwrap_or("n");
    let v = x_first(Some(c), "v");
    let raw = v.map(|el| el.text_content());
    match type_ {
        "s" => {
            let idx = raw.as_deref().and_then(|r| r.parse::<usize>().ok());
            let entry = idx.and_then(|i| ctx.shared_strings.get(i));
            if entry.is_none() {
                if raw.is_some() {
                    ctx.warn("A cell refers to a missing shared string.");
                }
                return;
            }
            let sst_idx = idx.unwrap() as u32;
            let entry = entry.unwrap();
            let i = store.push_shared(row, col, style_id, sst_idx);
            if let Some(runs) = &entry.runs {
                store.set_rich_text(i, runs.clone());
            }
            *cell_idx = Some(i);
        }
        "inlineStr" => {
            let is = x_first(Some(c), "is");
            if is.is_none() {
                return;
            }
            let entry = parse_rich_string(is.unwrap(), ctx.palette);
            let i = store.push_string(row, col, style_id, entry.text.clone());
            if let Some(runs) = entry.runs {
                store.set_rich_text(i, runs);
            }
            *cell_idx = Some(i);
        }
        "str" => {
            if let Some(v) = v {
                let i = store.push_string(row, col, style_id, x_text(Some(v)));
                *cell_idx = Some(i);
            }
        }
        "b" => {
            if let Some(raw) = raw {
                let b = raw.trim() == "1" || raw.trim().eq_ignore_ascii_case("true");
                let i = store.push_bool(row, col, style_id, b);
                *cell_idx = Some(i);
            }
        }
        "e" => {
            let code = raw.as_deref().unwrap_or("").trim();
            if raw.is_some() {
                let err = if is_error_code(code) {
                    code.to_owned()
                } else {
                    "#VALUE!".to_owned()
                };
                let i = store.push_error(row, col, style_id, err);
                *cell_idx = Some(i);
            }
        }
        "d" => {
            if let Some(raw) = raw {
                if let Some(serial) = iso_to_serial(raw.as_str(), ctx.date1904) {
                    let i = store.push_number(row, col, style_id, serial, Some(raw.as_str()));
                    *cell_idx = Some(i);
                } else {
                    ctx.warn(format!("Unreadable date cell value \"{raw}\"."));
                }
            }
        }
        _ => {
            if let Some(raw) = raw {
                if raw.trim().is_empty() {
                    return;
                }
                if let Ok(n) = raw.parse::<f64>() {
                    if n.is_finite() {
                        let i = store.push_number(row, col, style_id, n, Some(raw.as_str()));
                        *cell_idx = Some(i);
                    } else {
                        let i = store.push_string(row, col, style_id, raw);
                        *cell_idx = Some(i);
                    }
                } else {
                    let i = store.push_string(row, col, style_id, raw);
                    *cell_idx = Some(i);
                }
            }
        }
    }
    if cell_idx.is_none() && style_id > 0 {
        *cell_idx = Some(store.push_style_only(row, col, style_id));
    }
}

#[allow(clippy::too_many_arguments)]
fn read_formula(
    ctx: &mut CellContext<'_>,
    f: &Element,
    row: u32,
    col: u32,
    style_id: u32,
    store: &mut CellStore,
    cell_idx: Option<usize>,
    masters: &mut HashMap<String, SharedMaster>,
    dynamic: bool,
    anchors: &mut Vec<DynamicAnchor>,
) {
    let kind = att(Some(f), "t").unwrap_or("normal");
    let mut cell_idx = cell_idx;
    let text = strip_future_prefixes(&f.text_content());
    if cell_idx.is_none() && (!text.trim().is_empty() || kind != "normal") {
        cell_idx = Some(store.push_style_only(row, col, style_id));
    }
    if let Some(idx) = cell_idx {
        if dynamic {
            store.set_dynamic_array(idx);
        } else if kind != "array" {
            store.set_legacy_formula(idx);
        }
    }
    if kind == "shared" {
        let si = att(Some(f), "si").unwrap_or("");
        if !text.trim().is_empty() {
            masters.insert(
                si.to_owned(),
                SharedMaster {
                    row,
                    col,
                    formula: text.clone(),
                },
            );
            if let Some(idx) = cell_idx {
                store.set_formula(idx, text);
            }
            return;
        }
        let master = masters.get(si);
        if master.is_none() {
            ctx.warn("A shared formula refers to a missing master cell; its formula was dropped.");
            return;
        }
        let master = master.unwrap();
        let translated = translate_formula(
            &master.formula,
            (row - master.row) as i32,
            (col - master.col) as i32,
        );
        if let Some(idx) = cell_idx {
            store.set_formula(idx, translated);
        }
        return;
    }
    if kind == "dataTable" {
        ctx.warn("What-if data table formulas are not supported; their cached values were kept.");
        if let Some(idx) = cell_idx {
            store.clear_legacy_dynamic(idx);
        }
        return;
    }
    if text.trim().is_empty() {
        return;
    }
    if let Some(idx) = cell_idx {
        store.set_formula(idx, text);
    }
    if kind == "array" {
        let range = att(Some(f), "ref").and_then(parse_range);
        if dynamic {
            if let Some(range) = range {
                anchors.push(DynamicAnchor { row, col, range });
            }
        } else if let (Some(idx), Some(range)) = (cell_idx, range) {
            store.set_array_range(idx, range);
        }
    }
}

fn mark_spills(store: &mut CellStore, anchors: &[DynamicAnchor]) {
    for anchor in anchors {
        let start = &anchor.range.start;
        let end = &anchor.range.end;
        for r in start.row..=end.row {
            for c in start.col..=end.col {
                if r == anchor.row && c == anchor.col {
                    continue;
                }
                for i in 0..store.len() {
                    if store.row(i) == r && store.col(i) == c {
                        if store.formula(i).is_some() {
                            continue;
                        }
                        store.set_spill_anchor(
                            i,
                            CellAddress {
                                row: anchor.row,
                                col: anchor.col,
                            },
                        );
                    }
                }
            }
        }
    }
}

pub fn store_to_dump_cells(store: &CellStore) -> Vec<DumpCell> {
    store_to_dump_cells_with_sst(store, &[])
}

/// Materialize dump cells. Shared-string indices resolve through `sst` once (CellStore keeps indices).
impl crate::model::ActiveWorksheet {
    pub fn cell_count(&self) -> usize {
        if !self.cells.is_empty() {
            self.cells.len()
        } else {
            self.cell_store.as_ref().map(CellStore::len).unwrap_or(0)
        }
    }

    pub fn ensure_dump_cells(&mut self, sst: &[SharedString]) {
        if !self.cells.is_empty() {
            self.ensure_cell_pos();
            return;
        }
        if let Some(store) = self.cell_store.take() {
            self.cells = store_to_dump_cells_with_sst(&store, sst);
            self.rebuild_cell_pos();
        }
    }

    pub fn to_json_with_sst(&self, sst: &[SharedString]) -> serde_json::Value {
        use crate::model::{js_num, ColumnInfo};
        use serde_json::{json, Map, Value};

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
            Value::Array(
                self.merges
                    .iter()
                    .map(crate::model::CellRange::to_json)
                    .collect(),
            ),
        );
        let mut row_info = Map::new();
        for (k, v) in &self.row_info {
            row_info.insert(k.to_string(), v.to_json());
        }
        m.insert("rowInfo".into(), Value::Object(row_info));
        let cells_json: Vec<Value> = if !self.cells.is_empty() {
            self.cells
                .iter()
                .map(crate::model::DumpCell::to_json)
                .collect()
        } else if let Some(store) = &self.cell_store {
            store_to_dump_cells_with_sst(store, sst)
                .iter()
                .map(crate::model::DumpCell::to_json)
                .collect()
        } else {
            Vec::new()
        };
        m.insert("cells".into(), Value::Array(cells_json));
        Value::Object(m)
    }
}

pub fn store_to_dump_cells_with_sst(store: &CellStore, sst: &[SharedString]) -> Vec<DumpCell> {
    let mut out = Vec::new();
    for i in store.iter_sorted() {
        let value = match store.cell_type(i) {
            CellTypeTag::Empty => DumpCellValue::Null,
            CellTypeTag::Number => DumpCellValue::Number(store.number(i)),
            CellTypeTag::String => DumpCellValue::String(store.string(i).to_owned()),
            CellTypeTag::Shared => match sst.get(store.shared_index(i) as usize) {
                Some(entry) => DumpCellValue::String(entry.text.clone()),
                None => DumpCellValue::Null,
            },
            CellTypeTag::Bool => DumpCellValue::Bool(store.bool_val(i)),
            CellTypeTag::Error => DumpCellValue::Error(store.error(i).to_owned()),
        };
        let type_tag = match store.cell_type(i) {
            CellTypeTag::String | CellTypeTag::Shared => Some("s"),
            CellTypeTag::Bool => Some("b"),
            CellTypeTag::Error => Some("e"),
            _ => None,
        };
        out.push(DumpCell {
            row: store.row(i),
            col: store.col(i),
            value,
            type_tag,
            formula: store.formula(i).map(str::to_owned),
            style_id: if store.style(i) > 0 {
                Some(store.style(i))
            } else {
                None
            },
            rich_text: store.rich_text(i).map(|r| r.to_vec()),
            array_range: store.array_range(i).cloned(),
            legacy_formula: store.legacy_formula(i).then_some(true),
            dynamic_array: store.dynamic_array(i).then_some(true),
            spill_anchor: store.spill_anchor(i).cloned(),
            cell_metadata: store.cell_metadata(i),
            value_metadata: store.value_metadata(i),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cells::CellStore;
    use std::collections::HashMap;

    // Catches: stream sheetData dropping `&lt;&gt;` / `&amp;` / `&quot;` GeneralRefs from
    // formula text (COUNT(…)<>0 → COUNT(…)0), breaking worksheet dump parity vs DOM.
    // Also catches: `t="str"` + empty `<v/>` dumping Null instead of empty string.
    #[test]
    fn stream_preserves_xml_entities_in_formulas() {
        let inner = concat!(
            r#"<row r="1">"#,
            r#"<c r="A1"><f>IF(COUNT(C8:C39)&lt;&gt;0,1,)</f><v>1</v></c>"#,
            r#"<c r="B1" t="str"><f>B3&amp;" Matching List"</f><v>x</v></c>"#,
            r#"<c r="C1" t="str"><f>HYPERLINK(&quot;http://x&quot;,&quot;Note&quot;)</f><v>Note</v></c>"#,
            r#"<c r="D1" t="str"><f>""</f><v/></c>"#,
            r#"</row>"#,
        );
        let mut store = CellStore::new();
        let mut row_info = HashMap::new();
        let sst: Vec<SharedString> = Vec::new();
        let xf_map = Vec::new();
        let dynamic = HashSet::new();
        let mut warnings = Vec::new();
        let mut ctx = CellContext {
            shared_strings: &sst,
            xf_map: &xf_map,
            date1904: false,
            palette: None,
            dynamic_cells: &dynamic,
            warnings: &mut warnings,
        };
        assert!(
            read_sheet_data_xml_stream(&mut ctx, inner, &mut store, &mut row_info),
            "stream must accept fixture"
        );
        let dumps = store_to_dump_cells_with_sst(&store, &[]);
        let formulas: Vec<_> = dumps.iter().filter_map(|c| c.formula.as_deref()).collect();
        assert!(
            formulas.iter().any(|f| f.contains("<>")),
            "must keep <> from &lt;&gt;: {formulas:?}"
        );
        assert!(
            formulas.iter().any(|f| f.contains('&')),
            "must keep & from &amp;: {formulas:?}"
        );
        assert!(
            formulas.iter().any(|f| f.contains('"')),
            "must keep quotes from &quot;: {formulas:?}"
        );
        let d1 = dumps.iter().find(|c| c.row == 0 && c.col == 3).expect("D1");
        assert_eq!(
            d1.value,
            DumpCellValue::String(String::new()),
            "empty str+v must dump empty string, got {d1:?}"
        );
        assert_eq!(d1.type_tag, Some("s"));
    }
}
