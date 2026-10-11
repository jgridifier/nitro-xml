// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/PptxHandlerRuntimeLoadSession.ts,
// PptxHandlerRuntimeLoadPipeline.ts and PptxHandlerRuntimeSlideMasters.ts; changes in PROVENANCE.md.

use std::collections::HashMap;

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
    emu_to_px_helper, extract_text_body, parse_sp_tree, slim_element_for_dump,
    text_body_has_content, unwrap_alternate_content_deep, DumpElement, ParseCtx, SpTreeOptions,
    EMU_PER_PX,
};
use crate::theme::{is_presentation_theme_path, parse_theme_part, Theme};

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

fn missing_part(name: impl Into<String>) -> Error {
    OpcError::MissingPart { name: name.into() }
}

pub struct Slide {
    pub path: String,
    pub layout_path: Option<String>,
    pub master_path: Option<String>,
    pub show_master_shapes: Option<bool>,
    pub has_notes: bool,
    pub elements: Vec<DumpElement>,
    pub other_kinds: Vec<String>,
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
        };

        pres.presentation_level_styles = Self::load_presentation_default_text_style(&pres_doc.root);
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
            if !slide.other_kinds.is_empty() {
                o.insert("otherKinds".to_owned(), json!(slide.other_kinds));
            }
            let elements = slide
                .elements
                .iter()
                .map(slim_element_for_dump)
                .collect::<Vec<_>>();
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
        let ctx = ParseCtx {
            slide_path: &path,
            layout_sp_tree,
            master_sp_tree,
            presentation_level_styles: Some(&self.presentation_level_styles),
            layout_placeholder_levels: layout_ph,
            master_placeholder_levels: master_ph,
            master_tx_styles: master_tx,
        };
        let id_prefix = format!("{}-", path);
        let slide_options = SpTreeOptions::default();
        let (slide_elements, slide_other) =
            parse_sp_tree(&slide_sp_tree, Some(&raw), &id_prefix, &ctx, &slide_options);
        let (elements, other_kinds) = if show_master_shapes == Some(false) {
            (slide_elements, slide_other)
        } else if let Some(ref lp) = layout_path {
            let (layout_elements, layout_other) = self.decorative_elements_for_layout(lp)?;
            let mut elements = layout_elements;
            elements.extend(slide_elements);
            let mut other_kinds = layout_other;
            other_kinds.extend(slide_other);
            (elements, other_kinds)
        } else {
            (slide_elements, slide_other)
        };
        let has_notes = self.slide_has_notes(&path)?;
        let slide = Slide {
            path,
            layout_path,
            master_path,
            show_master_shapes,
            has_notes,
            elements,
            other_kinds,
        };
        self.slides[index] = Some(slide);
        Ok(())
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
    ) -> Result<(Vec<DumpElement>, Vec<String>), Error> {
        let doc = self
            .master_xml
            .get(master_path)
            .ok_or_else(|| missing_part(master_path))?;
        let master_raw = self
            .package
            .text(master_path)?
            .ok_or_else(|| missing_part(master_path))?
            .to_owned();
        let sp_tree = Self::sp_tree_from_part_root("sldMaster", doc)
            .ok_or_else(|| missing_part("p:spTree"))?;
        let master_ph = self.master_placeholder_levels.get(master_path);
        let master_tx = self.master_tx_styles.get(master_path);
        let ctx = ParseCtx {
            slide_path: master_path,
            layout_sp_tree: None,
            master_sp_tree: Some(&sp_tree),
            presentation_level_styles: Some(&self.presentation_level_styles),
            layout_placeholder_levels: None,
            master_placeholder_levels: master_ph,
            master_tx_styles: master_tx,
        };
        let token = Self::part_token(master_path);
        let options = SpTreeOptions {
            skip_placeholders: true,
            template_scope: Some("master".to_owned()),
            template_token: Some(token),
        };
        Ok(parse_sp_tree(
            &sp_tree,
            Some(&master_raw),
            "",
            &ctx,
            &options,
        ))
    }

    fn decorative_elements_for_layout(
        &mut self,
        layout_path: &str,
    ) -> Result<(Vec<DumpElement>, Vec<String>), Error> {
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
        let master_sp_tree = master_path.as_ref().and_then(|mp| {
            self.master_xml
                .get(mp)
                .and_then(|d| Self::sp_tree_from_part_root("sldMaster", d))
        });
        let mut elements = Vec::new();
        let mut other_kinds = Vec::new();
        if show_master {
            if let Some(ref mp) = master_path {
                let (m_el, m_ok) = self.decorative_elements_for_master(mp)?;
                elements.extend(m_el);
                other_kinds.extend(m_ok);
            }
        }
        let sp_tree = Self::sp_tree_from_part_root("sldLayout", &doc)
            .ok_or_else(|| missing_part("p:spTree"))?;
        let layout_ph = self.layout_placeholder_levels.get(layout_path);
        let master_ph = master_path
            .as_ref()
            .and_then(|mp| self.master_placeholder_levels.get(mp));
        let master_tx = master_path
            .as_ref()
            .and_then(|mp| self.master_tx_styles.get(mp));
        let ctx = ParseCtx {
            slide_path: layout_path,
            layout_sp_tree: Some(&sp_tree),
            master_sp_tree: master_sp_tree.as_ref(),
            presentation_level_styles: Some(&self.presentation_level_styles),
            layout_placeholder_levels: layout_ph,
            master_placeholder_levels: master_ph,
            master_tx_styles: master_tx,
        };
        let token = Self::part_token(layout_path);
        let options = SpTreeOptions {
            skip_placeholders: true,
            template_scope: Some("layout".to_owned()),
            template_token: Some(token),
        };
        let (lay_el, lay_ok) = parse_sp_tree(&sp_tree, Some(&layout_raw), "", &ctx, &options);
        elements.extend(lay_el);
        other_kinds.extend(lay_ok);
        Ok((elements, other_kinds))
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
