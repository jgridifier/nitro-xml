// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/PptxHandlerRuntimeLoadSession.ts,
// PptxHandlerRuntimeLoadPipeline.ts and PptxHandlerRuntimeSlideMasters.ts; chart/SmartArt part
// resolution for #27; changes in PROVENANCE.md.

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
use crate::theme::{default_scheme_color_map, is_presentation_theme_path, parse_theme_part, Theme};
use nitro_xml_opc::drawingml::ThemeFormatScheme;

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
    /// Kinds not parsed as elements (paint bands). Chart and SmartArt are elements.
    pub other_kinds: Vec<String>,
    /// Document-order `(seq, kind)` for the issue-24 `otherKinds` dump.
    pub other_kind_seq: Vec<(u32, String)>,
    pub background_color: Option<String>,
    pub background_gradient: Option<String>,
    /// Resolved `data:` URL for a blip slide/layout/master background.
    pub background_image: Option<String>,
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
        let theme_colors = self.theme_colors_owned();
        let format_scheme = self.format_scheme_owned();
        let ctx = ParseCtx {
            slide_path: &path,
            layout_sp_tree,
            master_sp_tree,
            presentation_level_styles: Some(&self.presentation_level_styles),
            layout_placeholder_levels: layout_ph,
            master_placeholder_levels: master_ph,
            master_tx_styles: master_tx,
            theme_colors: Some(&theme_colors),
            format_scheme: format_scheme.as_ref(),
        };
        let id_prefix = format!("{}-", path);
        let slide_options = SpTreeOptions::default();
        let mut slide_parsed =
            parse_sp_tree(&slide_sp_tree, Some(&raw), &id_prefix, &ctx, &slide_options);
        self.attach_graphic_payloads(&path, &mut slide_parsed.elements);
        self.attach_image_payloads(&path, &mut slide_parsed.elements);
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
        let (background_color, background_gradient, background_image) =
            self.slide_background(sld, &path, layout_path.as_deref(), master_path.as_deref());
        let slide = Slide {
            path,
            layout_path,
            master_path,
            show_master_shapes,
            has_notes,
            elements: parsed.elements,
            other_kinds: parsed.other_kinds,
            other_kind_seq: parsed.other_kind_seq,
            background_color,
            background_gradient,
            background_image,
        };
        self.slides[index] = Some(slide);
        Ok(())
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
            if let Some(paint) =
                first_paintable_background(&[Some(part)], &theme, format_scheme.as_ref())
            {
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

    fn smartart_drawing(&mut self, owner: &str, dm_r_id: &str) -> (Option<String>, Option<String>) {
        let Some(data_path) = self.resolve_rel_target(owner, dm_r_id).ok().flatten() else {
            return (None, None);
        };
        let drawing_rel = {
            let Ok(rels) = self.package.rels(&data_path) else {
                return (None, None);
            };
            rels.values()
                .find(|r| {
                    r.type_ == rel_types::DIAGRAM_DRAWING || r.type_.contains("diagramDrawing")
                })
                .cloned()
        };
        let Some(rel) = drawing_rel else {
            return (None, None);
        };
        let Some(path) = self.package.target(&data_path, &rel) else {
            return (None, None);
        };
        let xml = self.package.text(&path).ok().flatten().map(str::to_owned);
        (Some(path), xml)
    }

    fn attach_image_payloads(&mut self, owner: &str, elements: &mut [DumpElement]) {
        for el in elements.iter_mut() {
            if el.image_src.is_none() {
                if let Some(rel_id) = el.image_rel_id.clone() {
                    el.image_src = self.image_data_url_for_rel(owner, &rel_id);
                }
            }
            if let Some(children) = el.children.clone() {
                let mut children = children;
                self.attach_image_payloads(owner, &mut children);
                el.children = Some(children);
            }
        }
    }

    fn image_data_url_for_rel(&mut self, owner: &str, rel_id: &str) -> Option<String> {
        let path = self.resolve_rel_target(owner, rel_id).ok().flatten()?;
        let bytes = self.package.read(&path).ok().flatten()?;
        image_data_url(&bytes, &path)
    }

    fn attach_graphic_payloads(&mut self, owner: &str, elements: &mut [DumpElement]) {
        for el in elements.iter_mut() {
            if el.chart_xml.is_none() {
                if let Some(rid) = el.chart_r_id.clone() {
                    el.chart_xml = self.chart_part_xml(owner, &rid);
                }
            }
            if el.smartart_drawing_xml.is_none() {
                if let Some(dm) = el.smartart_dm_r_id.clone() {
                    let (path, xml) = self.smartart_drawing(owner, &dm);
                    if el.smartart_drawing_path.is_none() {
                        el.smartart_drawing_path = path;
                    }
                    el.smartart_drawing_xml = xml;
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

    fn decorative_elements_for_master(&mut self, master_path: &str) -> Result<ParsedSpTree, Error> {
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
            let ctx = ParseCtx {
                slide_path: master_path,
                layout_sp_tree: None,
                master_sp_tree: Some(&sp_tree),
                presentation_level_styles: Some(&self.presentation_level_styles),
                layout_placeholder_levels: None,
                master_placeholder_levels: master_ph,
                master_tx_styles: master_tx,
                theme_colors: Some(&theme_colors),
                format_scheme: format_scheme.as_ref(),
            };
            let token = Self::part_token(master_path);
            let options = SpTreeOptions {
                skip_placeholders: true,
                template_scope: Some("master".to_owned()),
                template_token: Some(token),
            };
            parse_sp_tree(&sp_tree, Some(&master_raw), "", &ctx, &options)
        };
        self.attach_graphic_payloads(master_path, &mut parsed.elements);
        self.attach_image_payloads(master_path, &mut parsed.elements);
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
        let master_sp_tree = master_path.as_ref().and_then(|mp| {
            self.master_xml
                .get(mp)
                .and_then(|d| Self::sp_tree_from_part_root("sldMaster", d))
        });
        let mut parsed = ParsedSpTree {
            elements: Vec::new(),
            other_kinds: Vec::new(),
            other_kind_seq: Vec::new(),
        };
        if show_master {
            if let Some(ref mp) = master_path {
                let master_parsed = self.decorative_elements_for_master(mp)?;
                concat_parsed(&mut parsed, master_parsed);
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
        let theme_colors = self.theme_colors_owned();
        let format_scheme = self.format_scheme_owned();
        let mut layout_parsed = {
            let ctx = ParseCtx {
                slide_path: layout_path,
                layout_sp_tree: Some(&sp_tree),
                master_sp_tree: master_sp_tree.as_ref(),
                presentation_level_styles: Some(&self.presentation_level_styles),
                layout_placeholder_levels: layout_ph,
                master_placeholder_levels: master_ph,
                master_tx_styles: master_tx,
                theme_colors: Some(&theme_colors),
                format_scheme: format_scheme.as_ref(),
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
        self.attach_graphic_payloads(layout_path, &mut layout_parsed.elements);
        self.attach_image_payloads(layout_path, &mut layout_parsed.elements);
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

/// Package image bytes as a browser `data:` URL. EMF/WMF stay unresolved (metafile S3).
pub(crate) fn image_data_url(bytes: &[u8], part_name: &str) -> Option<String> {
    let mime = image_mime(part_name)?;
    Some(format!("data:{mime};base64,{}", base64_encode(bytes)))
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
    use nitro_xml_render::measure::MeasureBatch;
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
        let mut reads = LayoutReadBatch::default();
        paint_slide_frame(
            &mut buffer,
            &mut intern,
            &mut measure,
            &mut reads,
            &mut opened,
            &SlidePaintViewport {
                slide_index: 0,
                scale: 1.0,
                viewport_w: 800,
                viewport_h: 600,
            },
        )
        .unwrap();
        let joined = intern.0.join("\n");
        assert!(joined.contains("background-image"));
        assert!(joined.contains(&format!("url({bg})")));
        assert!(joined.contains("src"));
        assert!(joined.contains(&src));
    }
}
