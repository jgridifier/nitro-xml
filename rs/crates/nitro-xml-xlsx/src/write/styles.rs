// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/styles.ts; changes listed in PROVENANCE.md.
//! `styles.xml` from the resolved cell-style table. Source numFmt ids and dxf/tableStyles markup are kept.

use std::collections::HashMap;

use nitro_xml_opc::xml::ns;
use nitro_xml_opc::xml::parse_xml_str;

use crate::model::{Border, CellStyle, Fill, NamedStyle};
use crate::numfmt::builtin_format_id;
use crate::workbook::default_cell_style;
use crate::xml_util::{att, num_attr, x_children, x_first};

use super::fragments::direct_child_xml;
use super::style_xml::{
    alignment_xml, border_xml, fill_xml, font_xml, num_fmt_xml, protection_xml,
};
use super::xml_out::{attr_f64, attr_str, attr_u32, el, XML_HEADER};

struct Pool {
    items: Vec<String>,
    index: HashMap<String, usize>,
}

impl Pool {
    fn id(&mut self, xml: String) -> usize {
        if let Some(&id) = self.index.get(&xml) {
            return id;
        }
        let id = self.items.len();
        self.index.insert(xml.clone(), id);
        self.items.push(xml);
        id
    }
}

struct SourceTail {
    num_fmts: HashMap<String, u32>,
    next_num_fmt: u32,
    dxfs: Option<String>,
    table_styles: Option<String>,
    colors: Option<String>,
    ext_lst: Option<String>,
}

fn read_source(xml: Option<&str>) -> SourceTail {
    let mut tail = SourceTail {
        num_fmts: HashMap::new(),
        next_num_fmt: 164,
        dxfs: None,
        table_styles: None,
        colors: None,
        ext_lst: None,
    };
    let Some(xml) = xml else {
        return tail;
    };
    if let Ok(doc) = parse_xml_str(xml) {
        let parent = x_first(Some(&doc.root), "numFmts").unwrap_or(&doc.root);
        for node in x_children(parent, "numFmt") {
            if let (Some(id), Some(code)) = (
                num_attr(Some(node), "numFmtId"),
                att(Some(node), "formatCode"),
            ) {
                if id >= 0.0 && !tail.num_fmts.contains_key(code) {
                    let id = id as u32;
                    tail.num_fmts.insert(code.to_owned(), id);
                    tail.next_num_fmt = tail.next_num_fmt.max(id + 1);
                }
            }
        }
    }
    for (name, raw) in direct_child_xml(xml) {
        match name.as_str() {
            "dxfs" => tail.dxfs = Some(raw),
            "tableStyles" => tail.table_styles = Some(raw),
            "colors" => tail.colors = Some(raw),
            "extLst" => tail.ext_lst = Some(raw),
            _ => {}
        }
    }
    tail
}

/// Builds `styles.xml` (cellXfs index = style id).
pub struct StyleWriter<'a> {
    styles: &'a [CellStyle],
    named: &'a [NamedStyle],
    fonts: Pool,
    fills: Pool,
    borders: Pool,
    num_fmts: HashMap<String, u32>,
    next_num_fmt: u32,
    dxfs: Option<String>,
    table_styles: Option<String>,
    colors: Option<String>,
    ext_lst: Option<String>,
}

impl<'a> StyleWriter<'a> {
    pub fn new(styles: &'a [CellStyle], named: &'a [NamedStyle], source_xml: Option<&str>) -> Self {
        let source = read_source(source_xml);
        let mut fills = Pool {
            items: Vec::new(),
            index: HashMap::new(),
        };
        fills.id(fill_xml(
            &Fill::Pattern {
                pattern: "none".into(),
                fg_color: None,
                bg_color: None,
            },
            false,
        ));
        fills.id(fill_xml(
            &Fill::Pattern {
                pattern: "gray125".into(),
                fg_color: None,
                bg_color: None,
            },
            false,
        ));
        Self {
            styles,
            named,
            fonts: Pool {
                items: Vec::new(),
                index: HashMap::new(),
            },
            fills,
            borders: Pool {
                items: Vec::new(),
                index: HashMap::new(),
            },
            num_fmts: source.num_fmts,
            next_num_fmt: source.next_num_fmt,
            dxfs: source.dxfs,
            table_styles: source.table_styles,
            colors: source.colors,
            ext_lst: source.ext_lst,
        }
    }

    fn num_fmt_id(&mut self, code: &str) -> u32 {
        if let Some(id) = builtin_format_id(code) {
            return u32::from(id);
        }
        if let Some(&id) = self.num_fmts.get(code) {
            return id;
        }
        let id = self.next_num_fmt;
        self.next_num_fmt += 1;
        self.num_fmts.insert(code.to_owned(), id);
        id
    }

    fn xf(&mut self, style: &CellStyle, xf_id: Option<u32>) -> String {
        let num_fmt_id = self.num_fmt_id(&style.num_fmt);
        let font_id = self.fonts.id(font_xml(&style.font, "font", false)) as u32;
        let fill_id = self.fills.id(fill_xml(&style.fill, false)) as u32;
        let border_id = self.borders.id(border_xml(&style.border)) as u32;
        let inner = format!(
            "{}{}",
            alignment_xml(style.alignment.as_ref()),
            protection_xml(style.protection.as_ref())
        );
        let apply = |name: &str, on: bool| -> String {
            if xf_id.is_some() && on {
                attr_bool_true(name)
            } else {
                String::new()
            }
        };
        let attrs = format!(
            "{}{}{}{}{}{}{}{}{}{}{}{}{}",
            attr_u32("numFmtId", Some(num_fmt_id)),
            attr_u32("fontId", Some(font_id)),
            attr_u32("fillId", Some(fill_id)),
            attr_u32("borderId", Some(border_id)),
            attr_u32("xfId", xf_id),
            apply("applyNumberFormat", num_fmt_id != 0),
            apply("applyFont", font_id != 0),
            apply("applyFill", fill_id != 0),
            apply("applyBorder", border_id != 0),
            apply("applyAlignment", style.alignment.is_some()),
            apply("applyProtection", style.protection.is_some()),
            attr_bool_true_opt("quotePrefix", style.quote_prefix),
            attr_bool_true_opt("pivotButton", style.pivot_button),
        );
        el("xf", &attrs, &inner)
    }

    pub fn xml(&mut self) -> String {
        let mut named: Vec<&NamedStyle> = self.named.iter().collect();
        if let Some(at) = named
            .iter()
            .position(|entry| entry.name == "Normal" || entry.builtin_id == Some(0.0))
        {
            if at > 0 {
                let normal = named.remove(at);
                named.insert(0, normal);
            }
        } else {
            // Owned fallback stored beside the slice via a side path: use styles[0].
            // Handled below when `named` is empty of Normal — we push a synthetic via flags.
        }
        let synthetic = if !named.iter().any(|entry| entry.name == "Normal") {
            let base = self
                .styles
                .first()
                .cloned()
                .unwrap_or_else(default_cell_style);
            Some(NamedStyle {
                name: "Normal".into(),
                style: base,
                builtin_id: Some(0.0),
            })
        } else {
            None
        };
        let named_refs: Vec<&NamedStyle> = if let Some(extra) = synthetic.as_ref() {
            let mut all = vec![extra];
            all.extend(named);
            all
        } else {
            named
        };
        let mut seen = std::collections::HashSet::new();
        let unique: Vec<&NamedStyle> = named_refs
            .into_iter()
            .filter(|entry| seen.insert(entry.name.clone()))
            .collect();
        let xf_id_of: HashMap<&str, u32> = unique
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.name.as_str(), index as u32))
            .collect();
        let base = self
            .styles
            .first()
            .cloned()
            .unwrap_or_else(default_cell_style);
        self.fonts.id(font_xml(&base.font, "font", false));
        self.borders.id(border_xml(&Border::default()));
        let mut style_xfs = Vec::new();
        for entry in &unique {
            style_xfs.push(self.xf(&entry.style, None));
        }
        let styles: Vec<CellStyle> = if self.styles.is_empty() {
            vec![base]
        } else {
            self.styles.to_vec()
        };
        let mut cell_xfs = Vec::new();
        for style in &styles {
            let xf_id = xf_id_of
                .get(style.cell_style_name.as_deref().unwrap_or("Normal"))
                .copied()
                .unwrap_or(0);
            cell_xfs.push(self.xf(style, Some(xf_id)));
        }
        let mut cell_styles = Vec::new();
        for (index, entry) in unique.iter().enumerate() {
            let attrs = format!(
                "{}{}{}",
                attr_str("name", Some(&entry.name)),
                attr_u32("xfId", Some(index as u32)),
                attr_f64("builtinId", entry.builtin_id),
            );
            cell_styles.push(el("cellStyle", &attrs, ""));
        }
        let mut custom: Vec<(u32, String)> = self
            .num_fmts
            .iter()
            .filter(|(code, _)| builtin_format_id(code).is_none())
            .map(|(code, id)| (*id, code.clone()))
            .collect();
        custom.sort_by_key(|(id, _)| *id);
        let mut out = format!(
            "{XML_HEADER}<styleSheet xmlns=\"{}\" xmlns:mc=\"{}\" mc:Ignorable=\"x14ac\" xmlns:x14ac=\"{}\">",
            ns::X,
            ns::MC,
            ns::X14AC
        );
        if !custom.is_empty() {
            let body: String = custom
                .iter()
                .map(|(id, code)| num_fmt_xml(*id, code))
                .collect();
            out.push_str(&format!(
                "<numFmts count=\"{}\">{body}</numFmts>",
                custom.len()
            ));
        }
        out.push_str(&format!(
            "<fonts count=\"{}\">{}</fonts>",
            self.fonts.items.len(),
            self.fonts.items.join("")
        ));
        out.push_str(&format!(
            "<fills count=\"{}\">{}</fills>",
            self.fills.items.len(),
            self.fills.items.join("")
        ));
        out.push_str(&format!(
            "<borders count=\"{}\">{}</borders>",
            self.borders.items.len(),
            self.borders.items.join("")
        ));
        out.push_str(&format!(
            "<cellStyleXfs count=\"{}\">{}</cellStyleXfs>",
            style_xfs.len(),
            style_xfs.join("")
        ));
        out.push_str(&format!(
            "<cellXfs count=\"{}\">{}</cellXfs>",
            cell_xfs.len(),
            cell_xfs.join("")
        ));
        out.push_str(&format!(
            "<cellStyles count=\"{}\">{}</cellStyles>",
            cell_styles.len(),
            cell_styles.join("")
        ));
        if let Some(dxfs) = &self.dxfs {
            out.push_str(dxfs);
        } else {
            out.push_str("<dxfs count=\"0\"></dxfs>");
        }
        out.push_str(
            self.table_styles.as_deref().unwrap_or(
                "<tableStyles count=\"0\" defaultTableStyle=\"TableStyleMedium2\" defaultPivotStyle=\"PivotStyleLight16\"/>",
            ),
        );
        if let Some(colors) = &self.colors {
            out.push_str(colors);
        }
        if let Some(ext) = &self.ext_lst {
            out.push_str(ext);
        }
        out.push_str("</styleSheet>");
        out
    }
}

fn attr_bool_true(name: &str) -> String {
    format!(" {name}=\"1\"")
}

fn attr_bool_true_opt(name: &str, value: Option<bool>) -> String {
    if value == Some(true) {
        attr_bool_true(name)
    } else {
        String::new()
    }
}
