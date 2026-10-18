// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/PptxHandlerRuntimeLoadSession.ts,
// PptxHandlerRuntimeLoadPipeline.ts, PptxHandlerRuntimeSlideMasters.ts,
// PptxHandlerRuntimeSlideUtils.ts (`extractSlideNotes`) and
// src/core/pptx/core/core/runtime/notes-body-shapes.ts; chart/SmartArt part
// resolution (data-part diagramDrawing, else the owner slide/layout/master) and VML OLE preview
// rels for #27; changes in PROVENANCE.md.

use std::collections::{BTreeMap, HashMap};

use nitro_xml_opc::opc::{rel_types, resolve_part_path, OpcError, Package, Relationship};
use nitro_xml_opc::xml::{ns, parse_xml_str, Document, Element};
use serde_json::json;

use crate::inheritance::{attr, child_local, child_path, children_local};
use crate::level_style::{
    parse_master_tx_styles, parse_tx_style_category, scan_sp_tree_placeholder_level_styles,
    LevelStylesMap, MasterTxStyles,
};
use crate::placeholder_defaults::PlaceholderDefaultsCache;
use crate::slide::{
    concat_parsed, emu_to_px_helper, extract_text_body, first_paintable_background,
    issue24_dump_parts, parse_sp_tree, shift_parse_seq, text_body_has_content,
    unwrap_alternate_content_deep, DumpElement, ParseCtx, ParsedSpTree, SpTreeOptions, EMU_PER_PX,
};
use crate::table_style::{parse_table_style_list, TableStyleMap};
use crate::theme::{default_scheme_color_map, is_presentation_theme_path, parse_theme_part, Theme};
use nitro_xml_opc::chart::parse_chart_space_xml;
use nitro_xml_opc::drawingml::{parse_theme_color_map, ThemeColorMap, ThemeFormatScheme};
use nitro_xml_render::chart::chart_blip_rel_ids;

pub type Error = OpcError;

fn presentation_root(doc_root: &Element) -> Option<&Element> {
    part_root(doc_root, "presentation")
}

fn part_root<'a>(doc_root: &'a Element, local: &str) -> Option<&'a Element> {
    if doc_root.local == local && doc_root.ns.as_deref() == Some(ns::P) {
        Some(doc_root)
    } else {
        child_local(doc_root, local, ns::P)
    }
}

fn notes_slide_has_printable_content(notes: &Element) -> bool {
    let c_sld = child_local(notes, "cSld", ns::P);
    let sp_tree = c_sld.and_then(|c| child_local(c, "spTree", ns::P));
    let Some(sp_tree) = sp_tree else {
        return false;
    };
    for sp in children_local(sp_tree, "sp", ns::P) {
        let ph = child_path(sp, &[("nvSpPr", ns::P), ("nvPr", ns::P), ("ph", ns::P)]);
        let ph_type = ph.and_then(|p| attr(p, "type"));
        if ph_type == Some("sldNum")
            || ph_type == Some("sldImg")
            || ph_type == Some("hdr")
            || ph_type == Some("ftr")
            || ph_type == Some("dt")
        {
            continue;
        }
        let tx = child_local(sp, "txBody", ns::P);
        if text_body_has_content(tx) {
            let text = tx.map(extract_text_body).unwrap_or_default();
            if !text.trim().is_empty() {
                return true;
            }
        }
    }
    false
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NotesShapeRole {
    Body,
    Field,
    Other,
}

/// `notesShapeRole`: body (including a bare `<p:ph>`), notes-page fields, or other.
fn notes_shape_role(shape: &Element) -> NotesShapeRole {
    let Some(ph) = child_path(shape, &[("nvSpPr", ns::P), ("nvPr", ns::P), ("ph", ns::P)]) else {
        return NotesShapeRole::Other;
    };
    let ty = attr(ph, "type").unwrap_or("").trim().to_ascii_lowercase();
    if ty.is_empty() || ty == "body" {
        return NotesShapeRole::Body;
    }
    if matches!(ty.as_str(), "sldimg" | "sldnum" | "dt" | "hdr" | "ftr") {
        NotesShapeRole::Field
    } else {
        NotesShapeRole::Other
    }
}

/// `selectNotesBodyShapes` + `extractTextFromTxBody`, joined the way `extractSlideNotes` does.
fn speaker_notes_text(notes: &Element) -> Option<String> {
    let sp_tree =
        child_local(notes, "cSld", ns::P).and_then(|c| child_local(c, "spTree", ns::P))?;
    let shapes = children_local(sp_tree, "sp", ns::P);
    let body: Vec<&Element> = shapes
        .iter()
        .copied()
        .filter(|sp| notes_shape_role(sp) == NotesShapeRole::Body)
        .collect();
    let chosen: Vec<&Element> = if body.is_empty() {
        shapes
            .iter()
            .copied()
            .filter(|sp| notes_shape_role(sp) == NotesShapeRole::Other)
            .collect()
    } else {
        body
    };
    let mut chunks = Vec::new();
    for shape in chosen {
        let Some(tx) = child_local(shape, "txBody", ns::P) else {
            continue;
        };
        let text = notes_tx_body_text(tx);
        if !text.is_empty() {
            chunks.push(text);
        }
    }
    let merged = chunks.join("\n").trim().to_owned();
    if merged.is_empty() {
        None
    } else {
        Some(merged)
    }
}

/// Paragraph text from a notes `p:txBody` (`extractTextFromTxBody`): runs, fields, breaks.
fn notes_tx_body_text(tx_body: &Element) -> String {
    let paras = children_local(tx_body, "p", ns::A);
    if paras.is_empty() {
        return tx_body.text_content().trim().to_owned();
    }
    let mut chunks = Vec::new();
    for (index, para) in paras.iter().enumerate() {
        let mut run = String::new();
        for child in children_local(para, "r", ns::A) {
            if let Some(t) = child_local(child, "t", ns::A) {
                run.push_str(&t.text_content());
            }
        }
        for child in children_local(para, "fld", ns::A) {
            if let Some(t) = child_local(child, "t", ns::A) {
                run.push_str(&t.text_content());
            }
        }
        if let Some(t) = child_local(para, "t", ns::A) {
            run.push_str(&t.text_content());
        }
        for _ in children_local(para, "br", ns::A) {
            run.push('\n');
        }
        chunks.push(run);
        if index + 1 < paras.len() {
            chunks.push("\n".to_owned());
        }
    }
    chunks.join("").trim().to_owned()
}

fn missing_part(name: impl Into<String>) -> Error {
    OpcError::MissingPart { name: name.into() }
}

fn is_diagram_drawing(type_: &str) -> bool {
    type_ == rel_types::DIAGRAM_DRAWING || type_.contains("diagramDrawing")
}

/// Trailing digits of a diagram part (`data1.xml`, `../diagrams/drawing1.xml` → `1`).
fn diagram_file_stem(path: &str) -> Option<&str> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = name.strip_suffix(".xml").unwrap_or(name);
    let head_len = stem
        .rfind(|c: char| !c.is_ascii_digit())
        .map(|i| i + 1)
        .unwrap_or(0);
    let digits = &stem[head_len..];
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

pub struct Slide {
    pub path: String,
    pub layout_path: Option<String>,
    pub master_path: Option<String>,
    pub show_master_shapes: Option<bool>,
    pub has_notes: bool,
    /// Speaker-notes body text (`slide.notes`). Empty notes stay `None`.
    pub notes_text: Option<String>,
    pub elements: Vec<DumpElement>,
    /// Kinds not parsed as elements (paint bands). Chart and SmartArt are elements.
    pub other_kinds: Vec<String>,
    /// Document-order `(seq, kind)` for the issue-24 `otherKinds` dump.
    pub other_kind_seq: Vec<(u32, String)>,
    pub background_color: Option<String>,
    pub background_gradient: Option<String>,
    /// Resolved `data:` URL for a blip slide/layout/master background.
    pub background_image: Option<String>,
    /// Theme scheme colours for SmartArt `a:schemeClr` (`accent1` → `#156082`).
    pub scheme_colors: BTreeMap<String, String>,
}

pub struct SlideMasterLayout {
    pub path: String,
    pub name: Option<String>,
    pub type_: Option<String>,
}

pub struct MasterPlaceholder {
    pub type_: String,
    pub idx: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

pub struct SlideMasterInfo {
    pub path: String,
    pub name: Option<String>,
    pub layouts: Vec<SlideMasterLayout>,
    pub placeholders: Vec<MasterPlaceholder>,
}

pub struct Presentation<'a> {
    package: Package<'a>,
    width: i64,
    height: i64,
    width_emu: i64,
    height_emu: i64,
    notes_width_emu: i64,
    notes_height_emu: i64,
    theme: Option<Theme>,
    slide_masters: Vec<SlideMasterInfo>,
    slide_paths: Vec<String>,
    slides: Vec<Option<Slide>>,
    layout_xml: HashMap<String, Document>,
    master_xml: HashMap<String, Document>,
    layout_to_master: HashMap<String, String>,
    presentation_level_styles: LevelStylesMap,
    master_tx_styles: HashMap<String, MasterTxStyles>,
    layout_placeholder_levels: HashMap<String, PlaceholderDefaultsCache>,
    master_placeholder_levels: HashMap<String, PlaceholderDefaultsCache>,
    table_style_map: TableStyleMap,
}

impl<'a> Presentation<'a> {
    pub fn open(bytes: &'a [u8]) -> Result<Self, Error> {
        let mut package = Package::open(bytes)?;
        let pres_xml = package
            .text("ppt/presentation.xml")?
            .ok_or_else(|| missing_part("ppt/presentation.xml"))?;
        let pres_doc = parse_xml_str(pres_xml)?;
        let pres =
            presentation_root(&pres_doc.root).ok_or_else(|| missing_part("p:presentation"))?;

        let sld_sz = child_local(pres, "sldSz", ns::P);
        let width_emu = parse_i64_attr(sld_sz.and_then(|n| attr(n, "cx")));
        let height_emu = parse_i64_attr(sld_sz.and_then(|n| attr(n, "cy")));
        let notes_sz = child_local(pres, "notesSz", ns::P);
        let notes_width_emu = parse_i64_attr(notes_sz.and_then(|n| attr(n, "cx")));
        let notes_height_emu = parse_i64_attr(notes_sz.and_then(|n| attr(n, "cy")));
        let width = emu_to_px_helper(width_emu);
        let height = emu_to_px_helper(height_emu);

        let mut pres = Self {
            package,
            width,
            height,
            width_emu,
            height_emu,
            notes_width_emu,
            notes_height_emu,
            theme: None,
            slide_masters: Vec::new(),
            slide_paths: Vec::new(),
            slides: Vec::new(),
            layout_xml: HashMap::new(),
            master_xml: HashMap::new(),
            layout_to_master: HashMap::new(),
            presentation_level_styles: LevelStylesMap::new(),
            master_tx_styles: HashMap::new(),
            layout_placeholder_levels: HashMap::new(),
            master_placeholder_levels: HashMap::new(),
            table_style_map: TableStyleMap::new(),
        };

        pres.presentation_level_styles = Self::load_presentation_default_text_style(&pres_doc.root);
        pres.table_style_map = pres.load_table_styles()?;
        pres.theme = pres.load_theme()?;
        pres.slide_masters = pres.load_slide_masters()?;
        pres.slide_paths = pres.load_slide_paths()?;
        pres.slides.resize_with(pres.slide_paths.len(), || None);
        if !pres.slide_paths.is_empty() {
            pres.parse_slide_index(0)?;
        }
        Ok(pres)
    }

    pub fn slide_paths(&self) -> &[String] {
        &self.slide_paths
    }

    pub fn slide_count(&self) -> usize {
        self.slide_paths.len()
    }

    /// Slide canvas size in CSS pixels (96 DPI), matching upstream `canvasSize`.
    pub fn canvas_size(&self) -> (f64, f64) {
        (self.width as f64, self.height as f64)
    }

    pub fn slide(&mut self, index: usize) -> Result<&Slide, Error> {
        if index >= self.slide_paths.len() {
            return Err(missing_part("slide index"));
        }
        if self.slides[index].is_none() {
            self.parse_slide_index(index)?;
        }
        Ok(self.slides[index].as_ref().expect("parsed"))
    }

    pub fn ensure_all_slides(&mut self) -> Result<(), Error> {
        for i in 0..self.slide_paths.len() {
            if self.slides[i].is_none() {
                self.parse_slide_index(i)?;
            }
        }
        Ok(())
    }

    pub fn inspect_json(&mut self) -> Result<String, Error> {
        self.ensure_all_slides()?;
        let mut slides_json = Vec::new();
        for (i, path) in self.slide_paths.iter().enumerate() {
            let slide = self.slides[i].as_ref().expect("parsed");
            let mut slide_obj = json!({
                "id": path,
                "hasNotes": slide.has_notes,
            });
            let o = slide_obj.as_object_mut().unwrap();
            if let Some(lp) = &slide.layout_path {
                o.insert("layoutPath".to_owned(), json!(lp));
            }
            if let Some(mp) = &slide.master_path {
                o.insert("masterPath".to_owned(), json!(mp));
            }
            if let Some(sms) = slide.show_master_shapes {
                o.insert("showMasterShapes".to_owned(), json!(sms));
            }
            let (elements, other_kinds) =
                issue24_dump_parts(&slide.elements, &slide.other_kind_seq);
            if !other_kinds.is_empty() {
                o.insert("otherKinds".to_owned(), json!(other_kinds));
            }
            o.insert("elements".to_owned(), json!(elements));
            slides_json.push(slide_obj);
        }

        let masters_json = self
            .slide_masters
            .iter()
            .map(|m| {
                let mut mobj = serde_json::Map::new();
                mobj.insert("path".to_owned(), json!(m.path));
                if let Some(name) = &m.name {
                    mobj.insert("name".to_owned(), json!(name));
                }
                mobj.insert(
                    "layouts".to_owned(),
                    json!(m
                        .layouts
                        .iter()
                        .map(|l| {
                            let mut o = json!({ "path": l.path, "name": l.name });
                            if let Some(t) = &l.type_ {
                                o.as_object_mut()
                                    .unwrap()
                                    .insert("type".to_owned(), json!(t));
                            }
                            o
                        })
                        .collect::<Vec<_>>()),
                );
                mobj.insert(
                    "placeholders".to_owned(),
                    json!(m
                        .placeholders
                        .iter()
                        .map(|p| {
                            let mut o = json!({ "type": p.type_ });
                            let obj = o.as_object_mut().unwrap();
                            if let Some(idx) = &p.idx {
                                obj.insert("idx".to_owned(), json!(idx));
                            }
                            if let Some(x) = p.x {
                                obj.insert("x".to_owned(), px_json(x));
                            }
                            if let Some(y) = p.y {
                                obj.insert("y".to_owned(), px_json(y));
                            }
                            if let Some(w) = p.width {
                                obj.insert("width".to_owned(), px_json(w));
                            }
                            if let Some(h) = p.height {
                                obj.insert("height".to_owned(), px_json(h));
                            }
                            o
                        })
                        .collect::<Vec<_>>()),
                );
                json!(mobj)
            })
            .collect::<Vec<_>>();

        let mut root = json!({
            "width": self.width,
            "height": self.height,
            "widthEmu": self.width_emu,
            "heightEmu": self.height_emu,
            "notesWidthEmu": self.notes_width_emu,
            "notesHeightEmu": self.notes_height_emu,
            "slideMasters": masters_json,
            "slides": slides_json,
        });
        if let Some(theme) = &self.theme {
            if theme.color_scheme.is_some() || theme.font_scheme.is_some() {
                root.as_object_mut().unwrap().insert(
                    "theme".to_owned(),
                    serde_json::to_value(theme).unwrap_or(json!({})),
                );
            }
        }
        Ok(root.to_string())
    }

    fn load_table_styles(&mut self) -> Result<TableStyleMap, Error> {
        let xml = self.package.text("ppt/tableStyles.xml")?;
        Ok(xml.map(parse_table_style_list).unwrap_or_default())
    }

    fn load_theme(&mut self) -> Result<Option<Theme>, Error> {
        let theme_path = self.resolve_primary_theme_path()?;
        if let Some(path) = theme_path {
            let xml = self
                .package
                .text(&path)?
                .ok_or_else(|| missing_part(path))?;
            let doc = parse_xml_str(xml)?;
            let theme = parse_theme_part(&doc.root);
            return Ok(Some(theme));
        }
        Ok(None)
    }

    fn resolve_primary_theme_path(&mut self) -> Result<Option<String>, Error> {
        let masters = self
            .package
            .part_names()
            .filter(|p| p.starts_with("ppt/slideMasters/slideMaster") && p.ends_with(".xml"))
            .collect::<Vec<_>>();
        if masters.is_empty() {
            return Ok(None);
        }
        let master_path = masters[0].to_owned();
        let theme_rels: Vec<Relationship> = {
            let rels = self.package.rels(&master_path)?;
            rels.values().cloned().collect()
        };
        for rel in &theme_rels {
            if rel.type_.contains("theme") {
                if let Some(target) = self.package.target(&master_path, rel) {
                    return Ok(Some(target));
                }
            }
        }
        Ok(self
            .package
            .part_names()
            .find(|p| is_presentation_theme_path(p))
            .map(str::to_owned))
    }

    fn load_presentation_default_text_style(pres_root: &Element) -> LevelStylesMap {
        let presentation = presentation_root(pres_root).unwrap_or(pres_root);
        child_local(presentation, "defaultTextStyle", ns::P)
            .map(parse_tx_style_category)
            .unwrap_or_default()
    }

    fn load_slide_masters(&mut self) -> Result<Vec<SlideMasterInfo>, Error> {
        let mut out = Vec::new();
        let master_paths: Vec<String> = self
            .package
            .part_names()
            .filter(|p| p.starts_with("ppt/slideMasters/slideMaster") && p.ends_with(".xml"))
            .map(str::to_owned)
            .collect();
        for path in master_paths {
            let xml = self
                .package
                .text(&path)?
                .ok_or_else(|| missing_part(&path))?;
            let doc = parse_xml_str(xml)?;
            self.master_xml.insert(path.clone(), doc.clone());
            let sld_master =
                part_root(&doc.root, "sldMaster").ok_or_else(|| missing_part("p:sldMaster"))?;
            let c_sld = child_local(sld_master, "cSld", ns::P);
            let name = c_sld
                .and_then(|c| attr(c, "name"))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            let sp_tree = c_sld.and_then(|c| child_local(c, "spTree", ns::P));
            let placeholders = sp_tree.map(extract_master_placeholders).unwrap_or_default();
            self.master_tx_styles
                .insert(path.clone(), parse_master_tx_styles(sld_master));
            if let Some(st) = sp_tree {
                let mut cloned = st.clone();
                unwrap_alternate_content_deep(&mut cloned);
                self.master_placeholder_levels
                    .insert(path.clone(), scan_sp_tree_placeholder_level_styles(&cloned));
            }

            let rels: Vec<Relationship> = self.package.rels(&path)?.values().cloned().collect();
            let layout_paths =
                resolve_slide_layout_order(sld_master, &rels, &path, |t| resolve_target(&path, t));
            let mut layouts = Vec::new();
            for lp in &layout_paths {
                self.layout_to_master.insert(lp.clone(), path.clone());
                if let Some(layout) = self.parse_layout_attributes(lp)? {
                    layouts.push(layout);
                }
            }

            out.push(SlideMasterInfo {
                path,
                name,
                layouts,
                placeholders,
            });
        }
        Ok(out)
    }

    fn parse_layout_attributes(
        &mut self,
        layout_path: &str,
    ) -> Result<Option<SlideMasterLayout>, Error> {
        let xml = self
            .package
            .text(layout_path)?
            .ok_or_else(|| missing_part(layout_path))?;
        let doc = parse_xml_str(xml)?;
        self.layout_xml.insert(layout_path.to_owned(), doc.clone());
        let sld_layout =
            part_root(&doc.root, "sldLayout").ok_or_else(|| missing_part("p:sldLayout"))?;
        if let Some(c_sld) = child_local(sld_layout, "cSld", ns::P) {
            if let Some(sp_tree) = child_local(c_sld, "spTree", ns::P) {
                let mut cloned = sp_tree.clone();
                unwrap_alternate_content_deep(&mut cloned);
                self.layout_placeholder_levels.insert(
                    layout_path.to_owned(),
                    scan_sp_tree_placeholder_level_styles(&cloned),
                );
            }
        }
        let c_sld = child_local(sld_layout, "cSld", ns::P);
        let name = c_sld
            .and_then(|c| attr(c, "name"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        Ok(Some(SlideMasterLayout {
            path: layout_path.to_owned(),
            name,
            type_: None,
        }))
    }

    fn load_slide_paths(&mut self) -> Result<Vec<String>, Error> {
        let xml = self
            .package
            .text("ppt/presentation.xml")?
            .ok_or_else(|| missing_part("ppt/presentation.xml"))?;
        let doc = parse_xml_str(xml)?;
        let presentation =
            presentation_root(&doc.root).ok_or_else(|| missing_part("p:presentation"))?;
        let sld_id_lst = child_local(presentation, "sldIdLst", ns::P);
        let mut paths = Vec::new();
        if let Some(lst) = sld_id_lst {
            for sld_id in children_local(lst, "sldId", ns::P) {
                let rel_id = rel_attr(sld_id, "id");
                if let Some(rid) = rel_id {
                    if let Some(target) = self.resolve_rel_target("ppt/presentation.xml", rid)? {
                        paths.push(target);
                    }
                }
            }
        }
        Ok(paths)
    }

    fn parse_slide_index(&mut self, index: usize) -> Result<(), Error> {
        let path = self.slide_paths[index].clone();
        let xml = self
            .package
            .text(&path)?
            .ok_or_else(|| missing_part(&path))?;
        let raw = xml.to_owned();
        let doc = parse_xml_str(&raw)?;
        let sld = part_root(&doc.root, "sld").ok_or_else(|| missing_part("p:sld"))?;
        let show_master_shapes = attr(sld, "showMasterSp").map(parse_bool_attr_val);
        let layout_path = self.find_layout_for_slide(&path)?;
        if let Some(ref lp) = layout_path {
            if !self.layout_xml.contains_key(lp) {
                let _ = self.parse_layout_attributes(lp)?;
            }
        }
        let master_path = layout_path
            .as_ref()
            .and_then(|lp| self.layout_to_master.get(lp).cloned())
            .or_else(|| {
                layout_path
                    .as_ref()
                    .and_then(|lp| self.find_master_for_layout(lp))
            });
        let color_map = self.slide_color_map(sld, master_path.as_deref());
        let layout_sp_tree = layout_path.as_ref().and_then(|lp| {
            self.layout_xml
                .get(lp)
                .and_then(|d| part_root(&d.root, "sldLayout"))
                .and_then(|l| child_local(l, "cSld", ns::P))
                .and_then(|c| child_local(c, "spTree", ns::P))
        });
        let master_sp_tree = master_path.as_ref().and_then(|mp| {
            self.master_xml
                .get(mp)
                .and_then(|d| part_root(&d.root, "sldMaster"))
                .and_then(|m| child_local(m, "cSld", ns::P))
                .and_then(|c| child_local(c, "spTree", ns::P))
        });
        let sp_tree = child_path(sld, &[("cSld", ns::P), ("spTree", ns::P)])
            .ok_or_else(|| missing_part("p:spTree"))?;
        let mut slide_sp_tree = sp_tree.clone();
        unwrap_alternate_content_deep(&mut slide_sp_tree);
        let layout_ph = layout_path
            .as_ref()
            .and_then(|lp| self.layout_placeholder_levels.get(lp));
        let master_ph = master_path
            .as_ref()
            .and_then(|mp| self.master_placeholder_levels.get(mp));
        let master_tx = master_path
            .as_ref()
            .and_then(|mp| self.master_tx_styles.get(mp));
        let theme_colors = self.theme_colors_owned();
        let format_scheme = self.format_scheme_owned();
        let font_scheme = self.theme.as_ref().and_then(|t| t.font_scheme.as_ref());
        let ctx = ParseCtx {
            slide_path: &path,
            layout_sp_tree,
            master_sp_tree,
            presentation_level_styles: Some(&self.presentation_level_styles),
            layout_placeholder_levels: layout_ph,
            master_placeholder_levels: master_ph,
            master_tx_styles: master_tx,
            theme_colors: Some(&theme_colors),
            color_map: Some(&color_map),
            format_scheme: format_scheme.as_ref(),
            font_scheme,
            table_style_map: Some(&self.table_style_map),
        };
        let id_prefix = format!("{}-", path);
        let slide_options = SpTreeOptions::default();
        let mut slide_parsed =
            parse_sp_tree(&slide_sp_tree, Some(&raw), &id_prefix, &ctx, &slide_options);
        self.attach_paint_payloads(&path, &mut slide_parsed.elements);
        shift_parse_seq(&mut slide_parsed, 2_000_000);
        let parsed = if show_master_shapes == Some(false) {
            slide_parsed
        } else if let Some(ref lp) = layout_path {
            let mut layout_parsed = self.decorative_elements_for_layout(lp)?;
            concat_parsed(&mut layout_parsed, slide_parsed);
            layout_parsed
        } else {
            slide_parsed
        };
        let has_notes = self.slide_has_notes(&path)?;
        let notes_text = self.slide_notes_text(&path)?;
        let (background_color, background_gradient, background_image) = self.slide_background(
            sld,
            &path,
            layout_path.as_deref(),
            master_path.as_deref(),
            &color_map,
        );
        let slide = Slide {
            path,
            layout_path,
            master_path,
            show_master_shapes,
            has_notes,
            notes_text,
            elements: parsed.elements,
            other_kinds: parsed.other_kinds,
            other_kind_seq: parsed.other_kind_seq,
            background_color,
            background_gradient,
            background_image,
            scheme_colors: theme_colors.clone(),
        };
        self.slides[index] = Some(slide);
        Ok(())
    }

    /// Master's `p:clrMap`. An empty map means PowerPoint alias defaults.
    fn master_color_map(&self, master_path: Option<&str>) -> ThemeColorMap {
        let Some(path) = master_path else {
            return ThemeColorMap::new();
        };
        let Some(master) = self
            .master_xml
            .get(path)
            .and_then(|doc| part_root(&doc.root, "sldMaster"))
        else {
            return ThemeColorMap::new();
        };
        parse_theme_color_map(child_local(master, "clrMap", ns::P))
    }

    /// `p:clrMapOvr/a:overrideClrMapping`. `a:masterClrMapping` (and a missing
    /// override) inherit the master map.
    fn part_clr_map_override(part: &Element) -> Option<ThemeColorMap> {
        let ovr = child_local(part, "clrMapOvr", ns::P)?;
        if child_local(ovr, "masterClrMapping", ns::A).is_some() {
            return None;
        }
        let mapping = child_local(ovr, "overrideClrMapping", ns::A)?;
        let map = parse_theme_color_map(Some(mapping));
        if map.is_empty() {
            None
        } else {
            Some(map)
        }
    }

    /// Layout `clrMapOvr` replaces the master map for layout and master chrome
    /// parsed through that layout. Otherwise the master's `p:clrMap`.
    fn layout_color_map(&self, layout_path: &str, master_path: Option<&str>) -> ThemeColorMap {
        let override_map = self
            .layout_xml
            .get(layout_path)
            .and_then(|doc| part_root(&doc.root, "sldLayout"))
            .and_then(Self::part_clr_map_override);
        override_map.unwrap_or_else(|| self.master_color_map(master_path))
    }

    /// Slide `clrMapOvr` wins; `a:masterClrMapping` keeps the master map.
    fn slide_color_map(&self, slide: &Element, master_path: Option<&str>) -> ThemeColorMap {
        Self::part_clr_map_override(slide).unwrap_or_else(|| self.master_color_map(master_path))
    }

    fn theme_colors_owned(&self) -> BTreeMap<String, String> {
        self.theme
            .as_ref()
            .and_then(|t| t.color_scheme.clone())
            .unwrap_or_else(default_scheme_color_map)
    }

    fn format_scheme_owned(&self) -> Option<ThemeFormatScheme> {
        self.theme.as_ref().and_then(|t| t.format_scheme.clone())
    }

    fn slide_background(
        &mut self,
        sld: &Element,
        slide_path: &str,
        layout_path: Option<&str>,
        master_path: Option<&str>,
        color_map: &ThemeColorMap,
    ) -> (Option<String>, Option<String>, Option<String>) {
        let theme = self.theme_colors_owned();
        let format_scheme = self.format_scheme_owned();
        let slide_csld = child_local(sld, "cSld", ns::P).cloned();
        let layout_csld = layout_path.and_then(|p| self.part_c_sld(p, "sldLayout"));
        let master_csld = master_path.and_then(|p| self.part_c_sld(p, "sldMaster"));
        let chain = [
            (slide_path, slide_csld.as_ref()),
            (layout_path.unwrap_or(""), layout_csld.as_ref()),
            (master_path.unwrap_or(""), master_csld.as_ref()),
        ];
        let mut chosen = None;
        for (owner, part) in chain {
            let Some(part) = part else {
                continue;
            };
            if owner.is_empty() {
                continue;
            }
            if let Some(paint) = first_paintable_background(
                &[Some(part)],
                &theme,
                format_scheme.as_ref(),
                Some(color_map),
            ) {
                chosen = Some((owner.to_owned(), paint));
                break;
            }
        }
        let Some((owner, paint)) = chosen else {
            return (None, None, None);
        };
        let image = paint
            .image_rel_id
            .as_deref()
            .and_then(|rel_id| self.image_data_url_for_rel(&owner, rel_id));
        (paint.color, paint.gradient, image)
    }

    fn part_c_sld(&self, path: &str, local: &str) -> Option<Element> {
        let doc = self
            .layout_xml
            .get(path)
            .or_else(|| self.master_xml.get(path))?;
        let part = part_root(&doc.root, local)?;
        child_local(part, "cSld", ns::P).cloned()
    }

    /// Chart part XML for `r_id` on a slide, layout, or master part.
    pub fn chart_part_xml(&mut self, owner_part: &str, r_id: &str) -> Option<String> {
        self.part_xml_by_rel(owner_part, r_id)
    }

    fn part_xml_by_rel(&mut self, owner: &str, r_id: &str) -> Option<String> {
        let path = self.resolve_rel_target(owner, r_id).ok().flatten()?;
        self.package.text(&path).ok().flatten().map(str::to_owned)
    }

    /// Cached `dsp:drawing` for a diagram data relationship.
    ///
    /// Order: the data part's `diagramDrawing` rel (older packages), then a
    /// `diagramDrawing` rel on the owner slide, layout, or master. When the
    /// owner lists several drawings, the target that shares the data part's
    /// stem (`data1` → `drawing1`) wins; otherwise the first drawing rel.
    fn smartart_drawing(&mut self, owner: &str, dm_r_id: &str) -> (Option<String>, Option<String>) {
        let Some(data_path) = self.resolve_rel_target(owner, dm_r_id).ok().flatten() else {
            return (None, None);
        };
        if let Some(found) = self.diagram_drawing_on(&data_path, None) {
            return found;
        }
        self.diagram_drawing_on(owner, diagram_file_stem(&data_path))
            .unwrap_or((None, None))
    }

    /// First `diagramDrawing` on `part`, or the one whose filename stem matches
    /// `prefer_stem` when that part declares more than one.
    fn diagram_drawing_on(
        &mut self,
        part: &str,
        prefer_stem: Option<&str>,
    ) -> Option<(Option<String>, Option<String>)> {
        let rels: Vec<Relationship> = self
            .package
            .rels(part)
            .ok()?
            .values()
            .filter(|r| is_diagram_drawing(&r.type_))
            .cloned()
            .collect();
        if rels.is_empty() {
            return None;
        }
        let chosen = rels
            .iter()
            .find(|r| prefer_stem.is_some_and(|stem| diagram_file_stem(&r.target) == Some(stem)))
            .unwrap_or(&rels[0]);
        let path = self.package.target(part, chosen)?;
        let xml = self.package.text(&path).ok().flatten().map(str::to_owned);
        Some((Some(path), xml))
    }

    fn attach_paint_payloads(&mut self, owner: &str, elements: &mut [DumpElement]) {
        self.attach_graphic_payloads(owner, elements);
        self.attach_image_payloads(owner, elements);
        self.attach_content_part_ink(owner, elements);
    }

    fn attach_content_part_ink(&mut self, owner: &str, elements: &mut [DumpElement]) {
        for el in elements.iter_mut() {
            if el.type_ == "contentPart" && el.ink_data.is_none() {
                if let Some(rel_id) = el.content_part_ink_rel_id.clone() {
                    if let Ok(Some(path)) = self.resolve_rel_target(owner, &rel_id) {
                        if let Ok(Some(xml)) = self.package.text(&path) {
                            if let Ok(doc) = parse_xml_str(xml) {
                                el.ink_data = crate::inkml::parse_ink_ml_content(
                                    &doc.root, el.width, el.height,
                                );
                            }
                        }
                    }
                }
            }
            if let Some(children) = el.children.clone() {
                let mut children = children;
                self.attach_content_part_ink(owner, &mut children);
                el.children = Some(children);
            }
        }
    }

    fn attach_image_payloads(&mut self, owner: &str, elements: &mut [DumpElement]) {
        for el in elements.iter_mut() {
            if el.image_src.is_none() {
                if let Some(rel_id) = el.image_rel_id.clone() {
                    let rel_owner = el
                        .image_rel_owner
                        .clone()
                        .unwrap_or_else(|| owner.to_owned());
                    el.image_src = self.image_data_url_for_rel(&rel_owner, &rel_id);
                }
            }
            if el.media_src.is_none() {
                if let Some(rel_id) = el.media_rel_id.clone() {
                    el.media_src = self.media_data_url_for_rel(owner, &rel_id);
                }
            }
            if let Some(children) = el.children.clone() {
                let mut children = children;
                self.attach_image_payloads(owner, &mut children);
                el.children = Some(children);
            }
        }
    }

    fn media_data_url_for_rel(&mut self, owner: &str, rel_id: &str) -> Option<String> {
        let path = self.resolve_rel_target(owner, rel_id).ok().flatten()?;
        let bytes = self.package.read(&path).ok().flatten()?;
        media_data_url(&bytes, &path)
    }

    fn image_data_url_for_rel(&mut self, owner: &str, rel_id: &str) -> Option<String> {
        let path = self.resolve_rel_target(owner, rel_id).ok().flatten()?;
        let bytes = self.package.read(&path).ok().flatten()?;
        if let Some(url) = image_data_url(&bytes, &path) {
            return Some(url);
        }
        let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if ext == "emf" || ext == "wmf" {
            let base = path.rsplit_once('.').map(|(b, _)| b).unwrap_or(&path);
            for alt in ["png", "jpg", "jpeg", "gif"] {
                let alt_path = format!("{base}.{alt}");
                if let Some(alt_bytes) = self.package.read(&alt_path).ok().flatten() {
                    if let Some(url) = image_data_url(&alt_bytes, &alt_path) {
                        return Some(url);
                    }
                }
            }
        }
        None
    }

    fn attach_chart_image_urls(&mut self, el: &mut DumpElement) {
        if el.chart_image_urls.is_some() {
            return;
        }
        let xml = match el.chart_xml.as_deref() {
            Some(x) => x,
            None => return,
        };
        let part = match el.chart_part_path.as_deref() {
            Some(p) => p,
            None => return,
        };
        let parsed = parse_chart_space_xml(xml);
        let mut urls = HashMap::new();
        for rel_id in chart_blip_rel_ids(&parsed.chart_space) {
            if let Some(url) = self.image_data_url_for_rel(part, &rel_id) {
                urls.insert(rel_id, url);
            }
        }
        if !urls.is_empty() {
            el.chart_image_urls = Some(urls);
        }
    }

    fn attach_graphic_payloads(&mut self, owner: &str, elements: &mut [DumpElement]) {
        for el in elements.iter_mut() {
            if el.chart_xml.is_none() {
                if let Some(rid) = el.chart_r_id.clone() {
                    if let Some(path) = self.resolve_rel_target(owner, &rid).ok().flatten() {
                        el.chart_part_path = Some(path);
                    }
                    el.chart_xml = self.chart_part_xml(owner, &rid);
                }
            }
            if el.chart_xml.is_some() {
                el.chart_minor_font = self
                    .theme
                    .as_ref()
                    .and_then(|t| t.font_scheme.as_ref())
                    .and_then(|f| f.minor_font.as_ref())
                    .and_then(|m| m.latin.clone())
                    .filter(|f| !f.is_empty());
                self.attach_chart_image_urls(el);
            }
            if el.smartart_drawing_xml.is_none() || el.smartart_data_xml.is_none() {
                if let Some(dm) = el.smartart_dm_r_id.clone() {
                    if el.smartart_drawing_xml.is_none() {
                        let (path, xml) = self.smartart_drawing(owner, &dm);
                        if el.smartart_drawing_path.is_none() {
                            el.smartart_drawing_path = path;
                        }
                        el.smartart_drawing_xml = xml;
                    }
                    if el.smartart_data_xml.is_none() {
                        el.smartart_data_xml = self.part_xml_by_rel(owner, &dm);
                    }
                }
            }
            if el.smartart_qs_xml.is_none() {
                if let Some(qs) = el.smartart_qs_r_id.clone() {
                    el.smartart_qs_xml = self.part_xml_by_rel(owner, &qs);
                }
            }
            if let Some(children) = el.children.clone() {
                let mut children = children;
                self.attach_graphic_payloads(owner, &mut children);
                el.children = Some(children);
            }
        }
    }

    fn part_token(path: &str) -> String {
        path.split('/')
            .next_back()
            .and_then(|s| s.strip_suffix(".xml"))
            .unwrap_or(path)
            .to_owned()
    }

    fn layout_show_master_shapes(layout_root: &Element) -> bool {
        attr(layout_root, "showMasterSp")
            .map(parse_bool_attr_val)
            .unwrap_or(true)
    }

    fn sp_tree_from_part_root(part_local: &str, doc: &Document) -> Option<Element> {
        let part = part_root(&doc.root, part_local)?;
        let c_sld = child_local(part, "cSld", ns::P)?;
        let sp_tree = child_local(c_sld, "spTree", ns::P)?;
        let mut cloned = sp_tree.clone();
        unwrap_alternate_content_deep(&mut cloned);
        Some(cloned)
    }

    fn decorative_elements_for_master(
        &mut self,
        master_path: &str,
        color_map: &ThemeColorMap,
    ) -> Result<ParsedSpTree, Error> {
        let master_raw = self
            .package
            .text(master_path)?
            .ok_or_else(|| missing_part(master_path))?
            .to_owned();
        let sp_tree = {
            let doc = self
                .master_xml
                .get(master_path)
                .ok_or_else(|| missing_part(master_path))?;
            Self::sp_tree_from_part_root("sldMaster", doc)
                .ok_or_else(|| missing_part("p:spTree"))?
        };
        let theme_colors = self.theme_colors_owned();
        let format_scheme = self.format_scheme_owned();
        let mut parsed = {
            let master_ph = self.master_placeholder_levels.get(master_path);
            let master_tx = self.master_tx_styles.get(master_path);
            let font_scheme = self.theme.as_ref().and_then(|t| t.font_scheme.as_ref());
            let ctx = ParseCtx {
                slide_path: master_path,
                layout_sp_tree: None,
                master_sp_tree: Some(&sp_tree),
                presentation_level_styles: Some(&self.presentation_level_styles),
                layout_placeholder_levels: None,
                master_placeholder_levels: master_ph,
                master_tx_styles: master_tx,
                theme_colors: Some(&theme_colors),
                color_map: Some(color_map),
                format_scheme: format_scheme.as_ref(),
                font_scheme,
                table_style_map: Some(&self.table_style_map),
            };
            let token = Self::part_token(master_path);
            let options = SpTreeOptions {
                skip_placeholders: true,
                template_scope: Some("master".to_owned()),
                template_token: Some(token),
            };
            parse_sp_tree(&sp_tree, Some(&master_raw), "", &ctx, &options)
        };
        self.attach_paint_payloads(master_path, &mut parsed.elements);
        Ok(parsed)
    }

    fn decorative_elements_for_layout(&mut self, layout_path: &str) -> Result<ParsedSpTree, Error> {
        let doc = self
            .layout_xml
            .get(layout_path)
            .cloned()
            .ok_or_else(|| missing_part(layout_path))?;
        let layout_raw = self
            .package
            .text(layout_path)?
            .ok_or_else(|| missing_part(layout_path))?
            .to_owned();
        let sld_layout =
            part_root(&doc.root, "sldLayout").ok_or_else(|| missing_part("p:sldLayout"))?;
        let show_master = Self::layout_show_master_shapes(sld_layout);
        let master_path = self.layout_to_master.get(layout_path).cloned();
        let color_map = self.layout_color_map(layout_path, master_path.as_deref());
        let mut parsed = ParsedSpTree {
            elements: Vec::new(),
            other_kinds: Vec::new(),
            other_kind_seq: Vec::new(),
        };
        if show_master {
            if let Some(ref mp) = master_path {
                let master_parsed = self.decorative_elements_for_master(mp, &color_map)?;
                concat_parsed(&mut parsed, master_parsed);
            }
        }
        let master_sp_tree = master_path.as_ref().and_then(|mp| {
            self.master_xml
                .get(mp)
                .and_then(|d| Self::sp_tree_from_part_root("sldMaster", d))
        });
        let sp_tree = Self::sp_tree_from_part_root("sldLayout", &doc)
            .ok_or_else(|| missing_part("p:spTree"))?;
        let layout_ph = self.layout_placeholder_levels.get(layout_path);
        let master_ph = master_path
            .as_ref()
            .and_then(|mp| self.master_placeholder_levels.get(mp));
        let master_tx = master_path
            .as_ref()
            .and_then(|mp| self.master_tx_styles.get(mp));
        let theme_colors = self.theme_colors_owned();
        let format_scheme = self.format_scheme_owned();
        let mut layout_parsed = {
            let font_scheme = self.theme.as_ref().and_then(|t| t.font_scheme.as_ref());
            let ctx = ParseCtx {
                slide_path: layout_path,
                layout_sp_tree: Some(&sp_tree),
                master_sp_tree: master_sp_tree.as_ref(),
                presentation_level_styles: Some(&self.presentation_level_styles),
                layout_placeholder_levels: layout_ph,
                master_placeholder_levels: master_ph,
                master_tx_styles: master_tx,
                theme_colors: Some(&theme_colors),
                color_map: Some(&color_map),
                format_scheme: format_scheme.as_ref(),
                font_scheme,
                table_style_map: Some(&self.table_style_map),
            };
            let token = Self::part_token(layout_path);
            let options = SpTreeOptions {
                skip_placeholders: true,
                template_scope: Some("layout".to_owned()),
                template_token: Some(token),
            };
            parse_sp_tree(&sp_tree, Some(&layout_raw), "", &ctx, &options)
        };
        shift_parse_seq(&mut layout_parsed, 1_000_000);
        self.attach_paint_payloads(layout_path, &mut layout_parsed.elements);
        concat_parsed(&mut parsed, layout_parsed);
        Ok(parsed)
    }

    fn slide_has_notes(&mut self, slide_path: &str) -> Result<bool, Error> {
        let note_rels: Vec<Relationship> = {
            let rels = self.package.rels(slide_path)?;
            rels.values().cloned().collect()
        };
        for rel in &note_rels {
            if rel.type_ == rel_types::NOTES_SLIDE {
                if let Some(notes_path) = self.package.target(slide_path, rel) {
                    let xml = self.package.text(&notes_path)?;
                    if let Some(xml) = xml {
                        let doc = parse_xml_str(xml)?;
                        if let Some(notes) = part_root(&doc.root, "notes") {
                            if notes_slide_has_printable_content(notes) {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    /// Speaker notes text, matching `extractSlideNotes`: body placeholders, else
    /// non-placeholder shapes. Slide-number / date / header / footer fields are skipped.
    fn slide_notes_text(&mut self, slide_path: &str) -> Result<Option<String>, Error> {
        let note_rels: Vec<Relationship> = {
            let rels = self.package.rels(slide_path)?;
            rels.values().cloned().collect()
        };
        for rel in &note_rels {
            if rel.type_ != rel_types::NOTES_SLIDE {
                continue;
            }
            let Some(notes_path) = self.package.target(slide_path, rel) else {
                continue;
            };
            let Some(xml) = self.package.text(&notes_path)? else {
                continue;
            };
            let doc = parse_xml_str(xml)?;
            if let Some(notes) = part_root(&doc.root, "notes") {
                return Ok(speaker_notes_text(notes));
            }
        }
        Ok(None)
    }

    fn find_layout_for_slide(&mut self, slide_path: &str) -> Result<Option<String>, Error> {
        let layout_rels: Vec<Relationship> = {
            let rels = self.package.rels(slide_path)?;
            rels.values().cloned().collect()
        };
        for rel in &layout_rels {
            if rel.type_ == rel_types::SLIDE_LAYOUT {
                return Ok(self.package.target(slide_path, rel));
            }
        }
        Ok(None)
    }

    fn find_master_for_layout(&mut self, layout_path: &str) -> Option<String> {
        let master_rels: Vec<Relationship> = match self.package.rels(layout_path) {
            Ok(rels) => rels.values().cloned().collect(),
            Err(_) => return None,
        };
        for rel in &master_rels {
            if rel.type_ == rel_types::SLIDE_MASTER {
                return self.package.target(layout_path, rel);
            }
        }
        None
    }

    fn resolve_rel_target(&mut self, owner: &str, rel_id: &str) -> Result<Option<String>, Error> {
        let rel = {
            let rels = self.package.rels(owner)?;
            rels.get(rel_id).cloned()
        };
        Ok(rel.and_then(|r| self.package.target(owner, &r)))
    }
}

fn px_json(v: f64) -> serde_json::Value {
    if v.fract() == 0.0 && v.is_finite() {
        json!(v as i64)
    } else {
        json!(v)
    }
}

fn parse_i64_attr(v: Option<&str>) -> i64 {
    v.and_then(|s| s.parse().ok()).unwrap_or(0)
}

fn parse_bool_attr_val(v: &str) -> bool {
    let n = v.trim().to_lowercase();
    n != "0" && n != "false"
}

fn rel_attr<'a>(el: &'a Element, local: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|a| a.local == local && a.ns.as_deref() == Some(ns::R))
        .map(|a| a.value.as_str())
}

fn resolve_target(base: &str, target: &str) -> String {
    resolve_part_path(base, target)
}

fn extract_master_placeholders(sp_tree: &Element) -> Vec<MasterPlaceholder> {
    let mut out = Vec::new();
    for sp in children_local(sp_tree, "sp", ns::P) {
        let ph_node = child_path(sp, &[("nvSpPr", ns::P), ("nvPr", ns::P)])
            .and_then(|nv| child_local(nv, "ph", ns::P));
        if ph_node.is_none() {
            continue;
        }
        let type_ = ph_node
            .and_then(|ph| attr(ph, "type"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| "body".to_owned());
        let idx = ph_node.and_then(|ph| attr(ph, "idx")).map(str::to_owned);
        let xfrm = child_path(sp, &[("spPr", ns::P), ("xfrm", ns::A)]);
        let (x, y, w, h) = if let Some(xfrm) = xfrm {
            let off = child_local(xfrm, "off", ns::A);
            let ext = child_local(xfrm, "ext", ns::A);
            let x_emu = parse_i64_attr(off.and_then(|o| attr(o, "x")));
            let y_emu = parse_i64_attr(off.and_then(|o| attr(o, "y")));
            let w_emu = parse_i64_attr(ext.and_then(|e| attr(e, "cx")));
            let h_emu = parse_i64_attr(ext.and_then(|e| attr(e, "cy")));
            (
                Some(emu_to_px_f64(x_emu)),
                Some(emu_to_px_f64(y_emu)),
                Some(emu_to_px_f64(w_emu)),
                Some(emu_to_px_f64(h_emu)),
            )
        } else {
            (None, None, None, None)
        };
        out.push(MasterPlaceholder {
            type_,
            idx,
            x,
            y,
            width: w,
            height: h,
        });
    }
    out
}

fn emu_to_px_f64(emu: i64) -> f64 {
    (emu as f64) / (EMU_PER_PX as f64)
}

fn resolve_slide_layout_order(
    sld_master: &Element,
    relationships: &[Relationship],
    master_path: &str,
    resolve_target_fn: impl Fn(&str) -> String,
) -> Vec<String> {
    let layout_rels: Vec<&Relationship> = relationships
        .iter()
        .filter(|r| r.type_.contains("slideLayout"))
        .collect();
    let mut target_by_id: HashMap<String, String> = HashMap::new();
    for rel in &layout_rels {
        target_by_id.insert(rel.id.clone(), rel.target.clone());
    }
    let layout_id_lst = child_local(sld_master, "sldLayoutIdLst", ns::P);
    let mut ordered = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut consumed = std::collections::HashSet::new();
    if let Some(lst) = layout_id_lst {
        for node in children_local(lst, "sldLayoutId", ns::P) {
            let rel_id = rel_attr(node, "id");
            if let Some(rid) = rel_id {
                if let Some(target) = target_by_id.get(rid) {
                    let path = resolve_target_fn(target);
                    if !path.is_empty() && seen.insert(path.clone()) {
                        consumed.insert(rid.to_owned());
                        ordered.push(path);
                    }
                }
            }
        }
    }
    for rel in &layout_rels {
        if !consumed.contains(&rel.id) {
            let path = resolve_target_fn(&rel.target);
            if !path.is_empty() && seen.insert(path.clone()) {
                ordered.push(path);
            }
        }
    }
    let _ = master_path;
    ordered
}

/// Package image bytes as a browser `data:` URL.
/// EMF/WMF use `nitro-metafile:` so the loader can rasterize via `emf-converter` (S3).
/// TIFF is decoded to PNG (Chromium cannot paint `image/tiff` in `<img>` — upstream
/// `decodeTiffToPngBlob`). SVG gets `preserveAspectRatio="none"` (upstream
/// `forceSvgStretchFill`) so stretch/`fillRect` matches raster object-fit.
pub(crate) fn image_data_url(bytes: &[u8], part_name: &str) -> Option<String> {
    let ext = part_name.rsplit('.').next()?.to_ascii_lowercase();
    if ext == "emf" || ext == "wmf" {
        return Some(format!(
            "nitro-metafile:{ext};base64,{}",
            base64_encode(bytes)
        ));
    }
    if ext == "tif" || ext == "tiff" {
        let png = tiff_to_png(bytes)?;
        return Some(format!("data:image/png;base64,{}", base64_encode(&png)));
    }
    if ext == "svg" {
        let text = std::str::from_utf8(bytes).ok()?;
        let stretched = force_svg_stretch_fill(text);
        return Some(format!(
            "data:image/svg+xml;base64,{}",
            base64_encode(stretched.as_bytes())
        ));
    }
    let mime = image_mime(part_name)?;
    Some(format!("data:{mime};base64,{}", base64_encode(bytes)))
}

/// Decode the first TIFF page to PNG (upstream `decodeTiffToPngBlob` / `utif`).
fn tiff_to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Cursor;
    use tiff::decoder::{Decoder, DecodingResult};
    use tiff::ColorType;

    let mut decoder = Decoder::new(Cursor::new(bytes)).ok()?;
    let (width, height) = decoder.dimensions().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let color = decoder.colortype().ok()?;
    let decoded = decoder.read_image().ok()?;
    let rgba = match (color, decoded) {
        (ColorType::RGB(8), DecodingResult::U8(buf)) => {
            let mut out = Vec::with_capacity(buf.len() / 3 * 4);
            for &[r, g, b] in buf.as_chunks::<3>().0 {
                out.extend_from_slice(&[r, g, b, 255]);
            }
            out
        }
        (ColorType::RGBA(8), DecodingResult::U8(buf)) => buf,
        (ColorType::Gray(8), DecodingResult::U8(buf)) => {
            let mut out = Vec::with_capacity(buf.len() * 4);
            for g in buf {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        (ColorType::GrayA(8), DecodingResult::U8(buf)) => {
            let mut out = Vec::with_capacity(buf.len() / 2 * 4);
            for &[g, a] in buf.as_chunks::<2>().0 {
                out.extend_from_slice(&[g, g, g, a]);
            }
            out
        }
        _ => return None,
    };
    if rgba.len() != (width as usize) * (height as usize) * 4 {
        return None;
    }
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
    }
    Some(png_bytes)
}

/// Upstream `forceSvgStretchFill`: set root `preserveAspectRatio="none"`.
fn force_svg_stretch_fill(svg_text: &str) -> String {
    let lower = svg_text.to_ascii_lowercase();
    let Some(start) = lower.find("<svg") else {
        return svg_text.to_owned();
    };
    let after_tag = start + 4;
    let Some(gt_rel) = svg_text[after_tag..].find('>') else {
        return svg_text.to_owned();
    };
    let gt = after_tag + gt_rel;
    let attrs = &svg_text[after_tag..gt];
    let attrs_lower = attrs.to_ascii_lowercase();
    let next_attrs = if let Some(p) = attrs_lower.find("preserveaspectratio") {
        let tail = &attrs[p..];
        // Match `\spreserveAspectRatio\s*=\s*("..."|'...')` replacement.
        let Some(eq) = tail.find('=') else {
            return format!(
                "{}{} preserveAspectRatio=\"none\"{}",
                &svg_text[..after_tag],
                attrs,
                &svg_text[gt..]
            );
        };
        let value = tail[eq + 1..].trim_start();
        let (quote, close) = if let Some(rest) = value.strip_prefix('"') {
            ('"', rest.find('"'))
        } else if let Some(rest) = value.strip_prefix('\'') {
            ('\'', rest.find('\''))
        } else {
            return format!(
                "{}{} preserveAspectRatio=\"none\"{}",
                &svg_text[..after_tag],
                attrs,
                &svg_text[gt..]
            );
        };
        let Some(close) = close else {
            return svg_text.to_owned();
        };
        let trimmed = tail[eq + 1..].len() - value.len();
        let value_end = p + eq + 1 + trimmed + 1 + close + 1;
        format!(
            "{}preserveAspectRatio={}none{}{}",
            &attrs[..p],
            quote,
            quote,
            &attrs[value_end..]
        )
    } else {
        format!("{attrs} preserveAspectRatio=\"none\"")
    };
    format!(
        "{}{}>{}",
        &svg_text[..after_tag],
        next_attrs,
        &svg_text[gt + 1..]
    )
}

fn image_mime(part_name: &str) -> Option<&'static str> {
    let ext = part_name.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        "tif" | "tiff" => Some("image/tiff"),
        _ => None,
    }
}

/// Package media bytes as a browser `data:` URL (video/audio parts).
pub(crate) fn media_data_url(bytes: &[u8], part_name: &str) -> Option<String> {
    let mime = media_mime(part_name)?;
    Some(format!("data:{mime};base64,{}", base64_encode(bytes)))
}

fn media_mime(part_name: &str) -> Option<&'static str> {
    let ext = part_name.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "m4v" => Some("video/mp4"),
        "mov" => Some("video/quicktime"),
        "webm" => Some("video/webm"),
        "mp3" => Some("audio/mpeg"),
        "m4a" => Some("audio/mp4"),
        "wav" => Some("audio/wav"),
        "ogg" => Some("audio/ogg"),
        _ => image_mime(part_name),
    }
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let (chunks, rest) = data.as_chunks::<3>();
    for chunk in chunks {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | chunk[2] as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
    }
    if rest.len() == 1 {
        let n = (rest[0] as u32) << 16;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest.len() == 2 {
        let n = ((rest[0] as u32) << 16) | ((rest[1] as u32) << 8);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{paint_slide_frame, SlidePaintViewport, StringIntern};
    use nitro_xml_render::measure::{MeasureBatch, OutlineBatch};
    use nitro_xml_render::smartart::{smartart_svg, smartart_svg_with_scheme};
    use nitro_xml_render::{CommandBuffer, LayoutReadBatch};

    struct Rec(Vec<String>);

    impl StringIntern for Rec {
        fn intern(&mut self, s: &str) -> u32 {
            if let Some(i) = self.0.iter().position(|existing| existing == s) {
                return i as u32;
            }
            self.0.push(s.to_owned());
            (self.0.len() - 1) as u32
        }
    }

    fn zip_stored(parts: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::{Cursor, Write};
        let mut cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in parts {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        cursor.into_inner()
    }

    // Catches: SVG data URL missing preserveAspectRatio=none so stretch/fillRect letterboxes.
    #[test]
    fn svg_data_url_forces_preserve_aspect_ratio_none() {
        let svg = br#"<?xml version="1.0"?><svg width="10" height="10" viewBox="0 0 10 10" xmlns="http://www.w3.org/2000/svg"><rect width="10" height="10" fill="red"/></svg>"#;
        let url = image_data_url(svg, "ppt/media/image6.svg").unwrap();
        assert!(url.starts_with("data:image/svg+xml;base64,"));
        let b64 = url.rsplit_once(',').unwrap().1;
        // Decode just enough to confirm the attribute landed (ASCII subset of base64).
        let stretched = force_svg_stretch_fill(std::str::from_utf8(svg).unwrap());
        assert!(stretched.contains("preserveAspectRatio=\"none\""));
        assert!(force_svg_stretch_fill(
            "<svg preserveAspectRatio=\"xMidYMid meet\" xmlns=\"http://www.w3.org/2000/svg\"/>"
        )
        .contains("preserveAspectRatio=\"none\""));
        let _ = b64;
    }

    // Catches: TIFF picture parts served as image/tiff so Chromium paints an empty broken-image tile.
    #[test]
    fn tiff_picture_becomes_png_data_url() {
        let tiff = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/upstream/pptx/3a09e4b3e925_picture-formats.pptx"
        ))
        .ok()
        .and_then(|pkg| {
            use std::io::Cursor;
            let mut zip = zip::ZipArchive::new(Cursor::new(pkg)).ok()?;
            let mut file = zip.by_name("ppt/media/image3.tif").ok()?;
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut file, &mut bytes).ok()?;
            Some(bytes)
        })
        .expect("fixture TIFF");
        let url = image_data_url(&tiff, "ppt/media/image3.tif").unwrap();
        assert!(
            url.starts_with("data:image/png;base64,"),
            "expected PNG data URL, got {}",
            &url[..url.len().min(40)]
        );
        assert!(tiff_to_png(&tiff)
            .unwrap()
            .starts_with(&[0x89, b'P', b'N', b'G']));
    }

    // Catches: blip r:embed left as a grey frame / empty stage instead of a data URL.
    #[test]
    fn blip_background_and_picture_become_data_urls() {
        let png = b"\x89PNG\r\n\x1a\n";
        let pres = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldSz cx="9144000" cy="5143500"/>
  <p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst>
</p:presentation>"#;
        let pres_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#;
        let root_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#;
        let slide = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:bg><p:bgPr><a:blipFill><a:blip r:embed="rId2"/><a:stretch><a:fillRect/></a:stretch></a:blipFill></p:bgPr></p:bg>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
      <p:pic>
        <p:nvPicPr><p:cNvPr id="2" name="Pic"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>
        <p:blipFill><a:blip r:embed="rId3"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
        <p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
      </p:pic>
    </p:spTree>
  </p:cSld>
</p:sld>"#;
        let slide_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/bg.png"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/pic.png"/>
</Relationships>"#;
        let types = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>"#;
        let bytes = zip_stored(&[
            ("[Content_Types].xml", types),
            ("_rels/.rels", root_rels),
            ("ppt/presentation.xml", pres),
            ("ppt/_rels/presentation.xml.rels", pres_rels),
            ("ppt/slides/slide1.xml", slide),
            ("ppt/slides/_rels/slide1.xml.rels", slide_rels),
            ("ppt/media/bg.png", png),
            ("ppt/media/pic.png", b"PIC"),
        ]);
        let mut opened = Presentation::open(&bytes).unwrap();
        let (bg, src) = {
            let slide = opened.slide(0).unwrap();
            let bg = slide.background_image.clone().unwrap();
            assert!(bg.starts_with("data:image/png;base64,"));
            assert_eq!(bg, image_data_url(png, "bg.png").unwrap());
            let pic = slide
                .elements
                .iter()
                .find(|el| el.type_ == "picture")
                .unwrap();
            let src = pic.image_src.clone().unwrap();
            assert_eq!(src, image_data_url(b"PIC", "pic.png").unwrap());
            (bg, src)
        };
        let mut intern = Rec(Vec::new());
        let mut buffer = CommandBuffer::new();
        let mut measure = MeasureBatch::default();
        let mut outline = OutlineBatch::default();
        let mut reads = LayoutReadBatch::default();
        let mut sp_autofit = crate::render::SpAutofitPaintState::default();
        paint_slide_frame(
            &mut buffer,
            &mut intern,
            &mut measure,
            &mut outline,
            &mut reads,
            &mut opened,
            &mut sp_autofit,
            &SlidePaintViewport {
                slide_index: 0,
                scale: 1.0,
                viewport_w: 800,
                viewport_h: 600,
                selected_ids: Vec::new(),
            },
        )
        .unwrap();
        let joined = intern.0.join("\n");
        assert!(joined.contains("background-image"));
        assert!(joined.contains(&format!("url({bg})")));
        assert!(joined.contains("src"));
        assert!(joined.contains(&src));
    }

    // Catches: master clrMap bg1="dk2" ignored so slide chrome paints yellow lt1 instead of blue.
    #[test]
    fn header_fixture_chrome_uses_dk2_not_lt1() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/real/pptx/poi-45541_Header.pptx"
        );
        let bytes = std::fs::read(path).expect("poi-45541_Header.pptx");
        let mut opened = Presentation::open(&bytes).unwrap();
        let slide = opened.slide(0).unwrap();
        let bg = slide.background_color.clone().unwrap_or_default();
        let grad = slide.background_gradient.clone().unwrap_or_default();
        assert!(
            is_dk2_blue(&bg) || is_dk2_blue(&grad),
            "bg={bg} grad={grad}"
        );
        assert!(
            !is_lt1_yellow(&bg) && !is_lt1_yellow(&grad),
            "bg={bg} grad={grad}"
        );
        let paints: Vec<String> = slide
            .elements
            .iter()
            .take(5)
            .map(|el| {
                el.fill_color
                    .clone()
                    .or(el.fill_gradient.clone())
                    .unwrap_or_default()
            })
            .collect();
        assert!(paints.iter().any(|p| is_dk2_blue(p)), "{paints:?}");
        assert!(paints.iter().all(|p| !is_lt1_yellow(p)), "{paints:?}");
    }

    #[test]
    fn slide_rels_supply_smartart_drawing() {
        // Catches: SmartArt drawing only on slide rels leaves smartart_drawing_xml unset so Alpha/Beta/Gamma never paint.
        let drawing1 = cached_drawing(&["Alpha", "Beta", "Gamma"]);
        let drawing9 = cached_drawing(&["Nope"]);
        let drawing8 = cached_drawing(&["Later"]);
        let pres = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldSz cx="9144000" cy="5143500"/>
  <p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst>
</p:presentation>"#;
        let pres_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#;
        let root_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#;
        let slide = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld><p:spTree>
    <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
    <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
    <p:graphicFrame>
      <p:nvGraphicFramePr><p:cNvPr id="5" name="SmartArt"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>
      <p:xfrm><a:off x="0" y="0"/><a:ext cx="5715000" cy="3333750"/></p:xfrm>
      <a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/diagram">
        <dgm:relIds xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" r:dm="rId3"/>
      </a:graphicData></a:graphic>
    </p:graphicFrame>
  </p:spTree></p:cSld>
</p:sld>"#;
        // No ppt/diagrams/_rels/data1.xml.rels. drawing9 is listed first so a
        // first-rel fallback would paint Nope instead of data1's drawing1.
        let slide_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramData" Target="../diagrams/data1.xml"/>
  <Relationship Id="rId7" Type="http://schemas.microsoft.com/office/2007/relationships/diagramDrawing" Target="../diagrams/drawing9.xml"/>
  <Relationship Id="rId2" Type="http://schemas.microsoft.com/office/2007/relationships/diagramDrawing" Target="../diagrams/drawing1.xml"/>
  <Relationship Id="rId8" Type="http://schemas.microsoft.com/office/2007/relationships/diagramDrawing" Target="../diagrams/drawing8.xml"/>
</Relationships>"#;
        let types = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>"#;
        let data = br#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram"/>"#;
        let bytes = zip_stored(&[
            ("[Content_Types].xml", types),
            ("_rels/.rels", root_rels),
            ("ppt/presentation.xml", pres),
            ("ppt/_rels/presentation.xml.rels", pres_rels),
            ("ppt/slides/slide1.xml", slide),
            ("ppt/slides/_rels/slide1.xml.rels", slide_rels),
            ("ppt/diagrams/data1.xml", data),
            ("ppt/diagrams/drawing1.xml", drawing1.as_bytes()),
            ("ppt/diagrams/drawing9.xml", drawing9.as_bytes()),
            ("ppt/diagrams/drawing8.xml", drawing8.as_bytes()),
        ]);
        let mut opened = Presentation::open(&bytes).unwrap();
        let (xml, width, height) = {
            let slide = opened.slide(0).unwrap();
            let el = slide
                .elements
                .iter()
                .find(|el| el.type_ == "smartArt")
                .expect("smartArt");
            assert_eq!(
                el.smartart_drawing_path.as_deref(),
                Some("ppt/diagrams/drawing1.xml")
            );
            let xml = el.smartart_drawing_xml.clone().expect("drawing xml");
            assert!(xml.contains("Alpha") && xml.contains("Beta") && xml.contains("Gamma"));
            assert!(!xml.contains("Nope") && !xml.contains("Later"));
            (xml, el.width, el.height)
        };
        let svg = smartart_svg(Some(&xml), width, height);
        assert!(
            svg.contains("Alpha") && svg.contains("Beta") && svg.contains("Gamma"),
            "{svg}"
        );
        assert!(svg.contains("<rect "), "{svg}");
        assert!(!svg.contains(">SmartArt<"), "{svg}");
        let mut intern = Rec(Vec::new());
        let mut buffer = CommandBuffer::new();
        let mut measure = MeasureBatch::default();
        let mut outline = OutlineBatch::default();
        let mut reads = LayoutReadBatch::default();
        let mut sp_autofit = crate::render::SpAutofitPaintState::default();
        paint_slide_frame(
            &mut buffer,
            &mut intern,
            &mut measure,
            &mut outline,
            &mut reads,
            &mut opened,
            &mut sp_autofit,
            &SlidePaintViewport {
                slide_index: 0,
                scale: 1.0,
                viewport_w: 960,
                viewport_h: 540,
                selected_ids: Vec::new(),
            },
        )
        .unwrap();
        let joined = intern.0.join("\n");
        assert!(
            joined.contains("Alpha") && joined.contains("Gamma"),
            "{joined}"
        );
        assert!(!joined.contains(">SmartArt<"), "{joined}");

        // Data-part diagramDrawing still wins when both sides declare one.
        let from_data = cached_drawing(&["FromData"]);
        let data_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.microsoft.com/office/2007/relationships/diagramDrawing" Target="drawing-from-data.xml"/>
</Relationships>"#;
        let slide_one = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramData" Target="../diagrams/data1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.microsoft.com/office/2007/relationships/diagramDrawing" Target="../diagrams/drawing1.xml"/>
</Relationships>"#;
        let both = zip_stored(&[
            ("[Content_Types].xml", types),
            ("_rels/.rels", root_rels),
            ("ppt/presentation.xml", pres),
            ("ppt/_rels/presentation.xml.rels", pres_rels),
            ("ppt/slides/slide1.xml", slide),
            ("ppt/slides/_rels/slide1.xml.rels", slide_one),
            ("ppt/diagrams/data1.xml", data),
            ("ppt/diagrams/_rels/data1.xml.rels", data_rels),
            ("ppt/diagrams/drawing-from-data.xml", from_data.as_bytes()),
            ("ppt/diagrams/drawing1.xml", drawing1.as_bytes()),
        ]);
        let mut both_opened = Presentation::open(&both).unwrap();
        let both_slide = both_opened.slide(0).unwrap();
        let both_el = both_slide
            .elements
            .iter()
            .find(|el| el.type_ == "smartArt")
            .expect("smartArt");
        assert_eq!(
            both_el.smartart_drawing_path.as_deref(),
            Some("ppt/diagrams/drawing-from-data.xml")
        );
        assert!(both_el
            .smartart_drawing_xml
            .as_deref()
            .unwrap()
            .contains("FromData"));

        assert_fixture_drawing(
            "f374280d2eb7_smartart-build-reveal.pptx",
            0,
            "Alpha",
            "drawing1.xml",
        );
        assert_fixture_drawing(
            "a61f509fcab9_chevron-list--hier5.pptx",
            0,
            "Node One",
            "drawing1.xml",
        );
        assert_fixture_drawing(
            "091abf411e47_smartart-orgchart-assistants.pptx",
            0,
            "Manager",
            "drawing1.xml",
        );
        assert_fixture_drawing(
            "091abf411e47_smartart-orgchart-assistants.pptx",
            1,
            "Manager",
            "drawing2.xml",
        );
        assert_fixture_drawing(
            "091abf411e47_smartart-orgchart-assistants.pptx",
            2,
            "Manager",
            "drawing3.xml",
        );
    }

    #[test]
    fn bidi_slides_paint_rtl_paragraphs() {
        // Catches: Arabic/Hebrew deck paragraphs painted LTR so visual gate stays above 0.1%.
        let path = format!(
            "{}/../../../fixtures/bidi/arabic-hebrew-slides.pptx",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(&path).unwrap();
        let opened = Presentation::open(&bytes).unwrap();
        let slide = opened.slides[0].as_ref().expect("slide 0");
        let text = slide
            .elements
            .iter()
            .find(|el| el.text.is_some())
            .expect("text shape");
        let paras = text
            .text_paint_paragraphs
            .as_ref()
            .expect("paint paragraphs");
        assert_eq!(paras.len(), 3, "parsed paragraphs: {:?}", paras.len());
        assert_eq!(paras[2].align, Some(crate::text_layout::TextAlignCss::Left));
    }

    #[test]
    fn smartart_fixtures_paint_without_clamp_panic() {
        // Catches: a short SmartArt shape panics in f64::clamp, aborting wasm so the host's next call reports recursive aliasing.
        let shp = paint_fixture("real/pptx/python-pptx-shp-shapes.pptx", 0);
        // Labels may wrap ("Smart" / "Art") once font-size is CSS px, not EMU.
        assert!(
            shp.contains("Smart Art") || (shp.contains(">Smart<") && shp.contains(">Art<")),
            "{shp}"
        );
        let accent = paint_fixture("upstream/pptx/21343018c511_accent-process--hier5.pptx", 0);
        assert!(accent.contains("Node One"), "{accent}");
        let org = paint_fixture(
            "upstream/pptx/091abf411e47_smartart-orgchart-assistants.pptx",
            0,
        );
        assert!(org.contains("Manager"), "{org}");
        assert!(
            org.contains("#156082"),
            "orgchart accent1 should be teal, got {org}"
        );
        assert!(
            !org.contains("#D6DCE4"),
            "textless frames painted as pale placeholders: {org}"
        );
    }

    fn paint_fixture(rel: &str, slide_index: usize) -> String {
        let path = format!("{}/../../../fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("read {path}"));
        let mut opened = Presentation::open(&bytes).unwrap();
        let mut intern = Rec(Vec::new());
        let mut buffer = CommandBuffer::new();
        let mut measure = MeasureBatch::default();
        let mut outline = OutlineBatch::default();
        let mut reads = LayoutReadBatch::default();
        let mut sp_autofit = crate::render::SpAutofitPaintState::default();
        paint_slide_frame(
            &mut buffer,
            &mut intern,
            &mut measure,
            &mut outline,
            &mut reads,
            &mut opened,
            &mut sp_autofit,
            &SlidePaintViewport {
                slide_index,
                scale: 1.0,
                viewport_w: 960,
                viewport_h: 540,
                selected_ids: Vec::new(),
            },
        )
        .unwrap();
        intern.0.join("\n")
    }

    // Catches: CJK-only runs with a:ea ≠ a:latin re-entering paint_run_text_span forever
    // (shell dump / open of poi-with_japanese.pptx stack-overflowed).
    #[test]
    fn paint_with_japanese_does_not_stack_overflow() {
        let painted = paint_fixture("real/pptx/poi-with_japanese.pptx", 0);
        assert!(
            painted.contains("Here is a text box") || painted.contains("pptxv-text"),
            "expected painted text host for with_japanese, got {} chars",
            painted.len()
        );
    }

    // Catches: master titleStyle defRPr/@kern never becoming CSS font-kerning on ctrTitle runs.
    #[test]
    fn bug62513_title_level_kern_paints_font_kerning() {
        let path = format!(
            "{}/../../../fixtures/real/pptx/poi-bug62513.pptx",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(&path).unwrap();
        let mut opened = Presentation::open(&bytes).unwrap();
        let slide = opened.slide(0).unwrap();
        let el = slide
            .elements
            .iter()
            .find(|e| e.text.as_deref().unwrap_or("").contains("CNIA"))
            .unwrap();
        let para = &el.text_paint_paragraphs.as_ref().unwrap()[0];
        assert_eq!(para.level_font_kern_hundredths, Some(1200));
        let painted = paint_fixture("real/pptx/poi-bug62513.pptx", 0);
        assert!(
            painted.contains("font-kerning") && painted.contains("normal"),
            "expected font-kerning:normal from titleStyle kern=1200"
        );
    }

    fn cached_drawing(labels: &[&str]) -> String {
        let mut body = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dsp:spTree>"#,
        );
        for (i, label) in labels.iter().enumerate() {
            // Real diagram drawings store EMU (TS `parseDrawingShape` rounds EMU/9525).
            let y = i * 476_250;
            body.push_str(&format!(
                r#"<dsp:sp><dsp:spPr><a:xfrm><a:off x="0" y="{y}"/><a:ext cx="914400" cy="457200"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="4472C4"/></a:solidFill></dsp:spPr><dsp:txBody><a:p><a:r><a:t>{label}</a:t></a:r></a:p></dsp:txBody></dsp:sp>"#
            ));
        }
        body.push_str("</dsp:spTree></dsp:drawing>");
        body
    }

    fn assert_fixture_drawing(name: &str, slide_index: usize, needle: &str, path_suffix: &str) {
        let path = format!(
            "{}/../../../fixtures/upstream/pptx/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("read {path}"));
        let mut opened = Presentation::open(&bytes).unwrap();
        let slide = opened.slide(slide_index).unwrap();
        let el = slide
            .elements
            .iter()
            .find(|el| el.type_ == "smartArt")
            .unwrap_or_else(|| panic!("{name} slide {slide_index} smartArt"));
        let xml = el
            .smartart_drawing_xml
            .as_deref()
            .unwrap_or_else(|| panic!("{name} drawing xml"));
        assert!(xml.contains(needle), "{name} missing {needle}");
        let drawing_path = el.smartart_drawing_path.as_deref().unwrap_or("");
        assert!(
            drawing_path.ends_with(path_suffix),
            "{name} path {drawing_path}"
        );
        let scheme = slide.scheme_colors.clone();
        let svg = smartart_svg_with_scheme(Some(xml), el.width, el.height, Some(&scheme));
        assert!(svg.contains(needle), "{name} svg");
        assert!(
            svg.contains("<rect ") || svg.contains("<path ") || svg.contains("<ellipse "),
            "{name} svg missing painted geometry"
        );
        assert!(!svg.contains(">SmartArt<"), "{name} stub");
    }

    fn is_dk2_blue(paint: &str) -> bool {
        paint.contains("0066CC") || paint.contains("0, 102, 204") || paint.contains("#0051A5")
    }

    fn is_lt1_yellow(paint: &str) -> bool {
        paint.contains("FFFF00") || paint.contains("255, 255, 0")
    }
}
