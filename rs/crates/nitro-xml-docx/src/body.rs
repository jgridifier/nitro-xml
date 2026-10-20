// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/parse.ts, block-parser.ts and
// document-parts.ts; changes listed in PROVENANCE.md.

use std::collections::{BTreeSet, HashMap, HashSet};

use indexmap::IndexMap;
use serde_json::{json, Value};

use nitro_xml_opc::opc::parse_content_types;
use nitro_xml_opc::opc::{AllowedFormat, OpcError, Package, Relationship};
use nitro_xml_opc::xml::{parse_xml_str, Element};

use crate::breaks::{classify_break, BreakKind};
use crate::chart_host::resolve_loaded_charts;
use crate::comments::parse_comments;
use crate::drawing::DrawingContext;
use crate::equation::parse_equation;
use crate::field_runs::{field_placeholder_text, story_field_results, FieldTracker};
use crate::hyperlink::{parse_simple_hyperlink_field, resolve_hyperlink};
use crate::notes::{parse_notes_part, NoteKind};
use crate::numbering::parse_numbering_catalog;
use crate::revisions::{
    collect_paragraph_runs, paragraph_format_revision, paragraph_mark_revision, OpenMoves,
    RunCollector,
};
use crate::run::parse_run;
use crate::sections::{parse_raw_sections, RawHeaderFooterRef};
use crate::settings::parse_track_review;
use crate::simple_types::on_off_element;
use crate::styles::{
    default_style_catalogs as default_cats, parse_paragraph_style_catalog, parse_run_style_catalog,
};
use crate::table::parse_table;
use crate::table_styles::parse_table_style_catalog;
use crate::theme::{parse_color_scheme_mapping, parse_theme};
use crate::warnings::{
    diagram_warnings, for_each_paragraph, image_and_bookmark_warnings, paragraph_bookmark_names,
    warnings_for,
};
use crate::xml_util::{by_name_w, children_w, first_w, get_w, named_w, ElementExt};

const PX_PER_TWIP: f64 = 96.0 / 1440.0;

pub struct DocumentModel {
    pub format: String,
    pub blocks: Vec<Value>,
    pub page: Value,
    pub warnings: Vec<String>,
    pub paragraph_styles: Option<Value>,
    pub character_styles: Option<Value>,
    pub table_styles: Option<Value>,
    pub numbering_catalog: Option<Value>,
    pub theme: Option<Value>,
    pub sections: Option<Vec<Value>>,
    pub footnotes: Option<Vec<Value>>,
    pub endnotes: Option<Vec<Value>>,
    pub footnote_num_fmt: Option<String>,
    pub endnote_num_fmt: Option<String>,
    pub even_and_odd_headers: bool,
    pub auto_hyphenation: bool,
    pub comments: Option<Vec<Value>>,
    pub track_changes: Option<bool>,
    pub track_formatting: Option<bool>,
    pub track_moves: Option<bool>,
    pub media_part_names: Vec<String>,
}

pub struct LoadedDocument<'a> {
    pub model: DocumentModel,
    pub(crate) package: Package<'a>,
    media_parts: BTreeSet<String>,
    media_cache: HashMap<String, Vec<u8>>,
}

/// How far into `w:body` a first-view open may parse.
#[derive(Debug, Clone, Copy, Default)]
pub struct BodyViewOpts {
    /// Stop after this many hard page-break paragraphs (`w:br w:type="page"`).
    /// `None` parses the full body (same as [`read_package`]).
    pub max_hard_page_breaks: Option<usize>,
}

/// Counters for O(visible) body opens (mirrors xlsx `rows_visited`).
#[derive(Debug, Clone, Copy, Default)]
pub struct BodyParseStats {
    pub blocks_parsed: usize,
    pub hard_page_breaks_seen: usize,
    pub body_truncated: bool,
}

pub fn read_package(bytes: &[u8]) -> Result<LoadedDocument<'_>, OpcError> {
    Ok(read_package_with_opts(bytes, BodyViewOpts::default())?.0)
}

/// Like [`read_package`], but may stop after the first N hard page breaks.
pub fn read_package_first_view(
    bytes: &[u8],
    max_hard_page_breaks: usize,
) -> Result<(LoadedDocument<'_>, BodyParseStats), OpcError> {
    read_package_with_opts(
        bytes,
        BodyViewOpts {
            max_hard_page_breaks: Some(max_hard_page_breaks),
        },
    )
}

pub fn read_package_with_opts(
    bytes: &[u8],
    opts: BodyViewOpts,
) -> Result<(LoadedDocument<'_>, BodyParseStats), OpcError> {
    let mut pkg = Package::open(bytes)?;
    let allowed = pkg.format();
    if !matches!(
        allowed,
        AllowedFormat::Docx | AllowedFormat::Docm | AllowedFormat::Dotx | AllowedFormat::Dotm
    ) {
        return Err(OpcError::UnsupportedFormat {
            detected: allowed.as_str().to_string(),
        });
    }
    let format = match allowed {
        AllowedFormat::Docx => "docx",
        AllowedFormat::Docm => "docm",
        AllowedFormat::Dotx => "dotx",
        AllowedFormat::Dotm => "dotm",
        other => {
            return Err(OpcError::UnsupportedFormat {
                detected: other.as_str().to_string(),
            })
        }
    }
    .to_string();
    let doc_xml = pkg
        .text("word/document.xml")?
        .ok_or_else(|| OpcError::MissingPart {
            name: "word/document.xml".into(),
        })?;
    let document = parse_xml_str(doc_xml)?;
    let body =
        crate::xml_util::first_body(&document.root).ok_or_else(|| OpcError::MissingPart {
            name: "w:body".into(),
        })?;

    let media_parts: HashSet<String> = pkg.part_names().map(str::to_owned).collect();
    let ct_xml = pkg.text("[Content_Types].xml")?;
    let content_types = parse_content_types(ct_xml)?;
    let doc_rels = pkg.rels("word/document.xml")?.clone();

    let drawings = DrawingContext {
        rels: doc_rels,
        content_types,
        media_parts,
    };

    let (blocks, parse_stats) =
        parse_blocks_from_container_limited(body, "", Some(&drawings), opts.max_hard_page_breaks);

    let sect_children = children_w(body, "sectPr");
    let section = sect_children.last().copied();
    let size = first_w(section, "pgSz");
    let margins = first_w(section, "pgMar");
    let page = json!({
        "width": px(get_w(size, "w"), 816.0),
        "height": px(get_w(size, "h"), 1056.0),
        "marginTop": px(get_w(margins, "top"), 96.0),
        "marginRight": px(get_w(margins, "right"), 96.0),
        "marginBottom": px(get_w(margins, "bottom"), 96.0),
        "marginLeft": px(get_w(margins, "left"), 96.0),
    });

    let mut warnings = warnings_for(&document);

    let (default_p, default_c, default_t) = default_cats();
    let (paragraph_styles, character_styles, table_styles) = if pkg
        .text("word/styles.xml")?
        .is_some()
    {
        let styles_xml = pkg.text("word/styles.xml")?.unwrap();
        warnings.push(
                "Paragraph style, character style and docDefaults inheritance resolve for rendering, including toggle-property XOR semantics and basedOn chains. Linked styles beyond a basedOn chain, numbering-derived formatting and font metric substitution are not modeled; display can still differ from Word.".to_string(),
            );
        let table = parse_table_style_catalog(styles_xml);
        if let Some(extra) = table.get("warnings").and_then(|v| v.as_array()) {
            for w in extra {
                if let Some(s) = w.as_str() {
                    warnings.push(s.to_string());
                }
            }
        }
        (
            Some(parse_paragraph_style_catalog(styles_xml)),
            Some(parse_run_style_catalog(styles_xml)),
            Some(table),
        )
    } else {
        (Some(default_p), Some(default_c), Some(default_t))
    };

    let mut theme = None;
    if pkg.text("word/theme/theme1.xml")?.is_some() {
        let theme_xml = pkg.text("word/theme/theme1.xml")?.unwrap();
        theme = Some(parse_theme(theme_xml));
        if let Some(settings_xml) = pkg.text("word/settings.xml")? {
            if let Some(t) = theme.as_mut() {
                if let Some(obj) = t.as_object_mut() {
                    obj.insert(
                        "colorMapping".into(),
                        parse_color_scheme_mapping(settings_xml),
                    );
                }
            }
        }
        warnings.push(
            "Theme colors and fonts resolve for rendering through a separate layer; direct theme references are preserved and never flattened onto runs.".to_string(),
        );
    }

    let mut numbering_catalog = None;
    if pkg.text("word/numbering.xml")?.is_some() {
        let num_xml = pkg.text("word/numbering.xml")?.unwrap();
        let catalog = parse_numbering_catalog(num_xml);
        numbering_catalog = Some(catalog.clone());
        warnings.push(
            "Numbering is resolved for decimal, Roman numeral, letter, ordinal, cardinal/ordinal text and bullet formats, including multilevel lvlText and legal numbering. Picture bullets, style-linked numbering and other custom formats fall back to Decimal Number and are not rendered as Word would.".to_string(),
        );
        if let Some(extra) = catalog.get("warnings").and_then(|v| v.as_array()) {
            for w in extra {
                if let Some(s) = w.as_str() {
                    warnings.push(s.to_string());
                }
            }
        }
    } else if !by_name_w(&document.root, "numPr").is_empty() {
        warnings.push(
            "Paragraphs reference list numbering, but the package has no word/numbering.xml part; numbering is preserved as paragraph XML but not rendered.".to_string(),
        );
    }

    if blocks
        .iter()
        .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("table"))
    {
        warnings.push(
            "Table grid widths, merges, borders, shading, cell alignment/margins and table style conditional formatting resolve for rendering; row/column fragmentation and full table-style precedence are not modeled.".to_string(),
        );
    }

    let parts = parse_document_parts(&mut pkg, body, &blocks, &drawings)?;
    warnings.extend(parts.warnings);

    if blocks.iter().any(|b| {
        b.get("type").and_then(|v| v.as_str()) == Some("table")
            && b.get("structureEditable").and_then(|v| v.as_bool()) == Some(false)
    }) {
        warnings.push(
            "Merged, nested, or complex tables can be read, but their row and column structure cannot be edited safely.".to_string(),
        );
    }

    if pkg.text("word/comments.xml")?.is_some() {
        warnings.push(
            "Comment ranges are modeled at run granularity (they may span paragraphs); a range edge outside any run, such as around an empty paragraph or a table, moves to the nearest commented text when that paragraph is edited.".to_string(),
        );
    }

    let (track_changes, track_formatting, track_moves) =
        if let Some(settings_xml) = pkg.text("word/settings.xml")? {
            let (tc, tf, tm) = parse_track_review(settings_xml);
            (Some(tc), Some(tf), Some(tm))
        } else {
            (None, None, None)
        };

    if blocks.iter().any(table_has_nested) {
        warnings.push(
            "Nested tables render as a read-only text preview; edit their content from the original document.".to_string(),
        );
    }

    let mut picture_blocks = blocks.clone();
    if let Some(sections) = &parts.sections {
        for section in sections {
            for key in ["headers", "footers"] {
                if let Some(slots) = section.get(key).and_then(|v| v.as_object()) {
                    for (_, content) in slots {
                        if let Some(b) = content.get("blocks").and_then(|v| v.as_array()) {
                            picture_blocks.extend(b.clone());
                        }
                    }
                }
            }
        }
    }
    if let Some(notes) = &parts.footnotes {
        for note in notes {
            if let Some(b) = note.get("blocks").and_then(|v| v.as_array()) {
                picture_blocks.extend(b.clone());
            }
        }
    }
    if let Some(notes) = &parts.endnotes {
        for note in notes {
            if let Some(b) = note.get("blocks").and_then(|v| v.as_array()) {
                picture_blocks.extend(b.clone());
            }
        }
    }

    warnings.extend(image_and_bookmark_warnings(&blocks));
    warnings.extend(diagram_warnings(&picture_blocks));

    let mut image_part_names = BTreeSet::new();
    for_each_paragraph(&picture_blocks, &mut |paragraph| {
        if let Some(runs) = paragraph.get("runs").and_then(|v| v.as_array()) {
            for run in runs {
                if let Some(image) = run.get("image") {
                    if let Some(p) = image.get("partName").and_then(|v| v.as_str()) {
                        if !p.is_empty() {
                            image_part_names.insert(p.to_string());
                        }
                    }
                    if let Some(p) = image.get("svgPartName").and_then(|v| v.as_str()) {
                        image_part_names.insert(p.to_string());
                    }
                }
            }
        }
    });

    let comments = if pkg.text("word/comments.xml")?.is_some() {
        let comments_xml = pkg.text("word/comments.xml")?.unwrap().to_string();
        let extended = pkg
            .text("word/commentsExtended.xml")?
            .map(|s| s.to_string());
        Some(parse_comments(&comments_xml, extended.as_deref()))
    } else {
        None
    };

    let mut model = DocumentModel {
        format,
        blocks,
        page,
        warnings,
        paragraph_styles,
        character_styles,
        table_styles,
        numbering_catalog,
        theme,
        sections: parts.sections,
        footnotes: parts.footnotes,
        endnotes: parts.endnotes,
        footnote_num_fmt: parts.footnote_num_fmt,
        endnote_num_fmt: parts.endnote_num_fmt,
        even_and_odd_headers: parts.even_and_odd_headers,
        auto_hyphenation: parts.auto_hyphenation,
        comments,
        track_changes,
        track_formatting,
        track_moves,
        media_part_names: image_part_names.into_iter().collect(),
    };

    // Layout labels need title / undrawn family; full chartSpace parse is #28.
    {
        let mut read_text =
            |name: &str| -> Option<String> { pkg.text(name).ok().flatten().map(|s| s.to_string()) };
        resolve_loaded_charts(
            &mut model.blocks,
            model.footnotes.as_mut(),
            model.endnotes.as_mut(),
            model.sections.as_mut(),
            &mut read_text,
        );
    }

    let media_set: BTreeSet<String> = model.media_part_names.iter().cloned().collect();
    Ok((
        LoadedDocument {
            model,
            package: pkg,
            media_parts: media_set,
            media_cache: HashMap::new(),
        },
        parse_stats,
    ))
}

pub fn get_media<'a>(doc: &'a mut LoadedDocument<'a>, part_name: &str) -> Option<&'a [u8]> {
    if !doc.media_parts.contains(part_name) {
        return None;
    }
    if !doc.media_cache.contains_key(part_name) {
        if let Ok(Some(bytes)) = doc.package.read(part_name) {
            doc.media_cache.insert(part_name.to_string(), bytes);
        }
    }
    doc.media_cache.get(part_name).map(|v| v.as_slice())
}

pub fn parse_blocks_from_container(
    container: &Element,
    id_prefix: &str,
    context: Option<&DrawingContext>,
) -> Vec<Value> {
    parse_blocks_from_container_limited(container, id_prefix, context, None).0
}

/// Like [`parse_blocks_from_container`], optionally stopping after N hard page breaks.
pub fn parse_blocks_from_container_limited(
    container: &Element,
    id_prefix: &str,
    context: Option<&DrawingContext>,
    max_hard_page_breaks: Option<usize>,
) -> (Vec<Value>, BodyParseStats) {
    let state = ParseState { context };
    state.parse_container_limited(container, id_prefix, max_hard_page_breaks)
}

/// True when a paragraph element contains a hard page break run.
fn paragraph_element_has_page_break(p: &Element) -> bool {
    for el in by_name_w(p, "br") {
        if matches!(classify_break(el), BreakKind::Page) {
            return true;
        }
    }
    false
}

struct ParseState<'a> {
    context: Option<&'a DrawingContext>,
}

impl<'a> ParseState<'a> {
    fn parse_container_limited(
        &self,
        container: &Element,
        id_prefix: &str,
        max_hard_page_breaks: Option<usize>,
    ) -> (Vec<Value>, BodyParseStats) {
        let mut blocks = Vec::new();
        let field_meta = story_field_results(container);
        let mut open_comments = Vec::new();
        let mut open_moves = OpenMoves::default();
        let mut index = 0;
        let mut hard_page_breaks_seen = 0usize;
        let mut truncated = false;
        for node in container.element_children() {
            if named_w(node, "p") {
                let is_page_break = paragraph_element_has_page_break(node);
                blocks.push(self.parse_paragraph(
                    node,
                    format!("{}p{}", id_prefix, index),
                    &field_meta,
                    &mut open_comments,
                    &mut open_moves,
                ));
                index += 1;
                if is_page_break {
                    hard_page_breaks_seen += 1;
                    if let Some(max) = max_hard_page_breaks {
                        if hard_page_breaks_seen >= max {
                            truncated = true;
                            break;
                        }
                    }
                }
            } else if named_w(node, "tbl") {
                let table_id = format!("{}t{}", id_prefix, index);
                blocks.push(self.parse_table(node, &table_id, &field_meta));
                index += 1;
            }
        }
        // If we stopped early, trailing body children (including final sectPr) were not
        // visited as blocks — sectPr is read separately via children_w(body, "sectPr").
        if truncated {
            // Confirm there is more body content beyond the stop point.
            let total_story = container
                .element_children()
                .into_iter()
                .filter(|n| named_w(n, "p") || named_w(n, "tbl"))
                .count();
            truncated = blocks.len() < total_story;
        }
        let stats = BodyParseStats {
            blocks_parsed: blocks.len(),
            hard_page_breaks_seen,
            body_truncated: truncated,
        };
        (blocks, stats)
    }

    fn parse_table(
        &self,
        node: &Element,
        id: &str,
        field_meta: &HashMap<usize, (Value, Option<Value>)>,
    ) -> Value {
        let mut open_comments = Vec::new();
        let mut open_moves = OpenMoves::default();
        let mut parse_para = |p: &Element, pid: &str| {
            self.parse_paragraph(
                p,
                pid.to_string(),
                field_meta,
                &mut open_comments,
                &mut open_moves,
            )
        };
        parse_table(node, id, &mut parse_para)
    }

    fn parse_paragraph(
        &self,
        node: &Element,
        id: String,
        field_meta: &HashMap<usize, (Value, Option<Value>)>,
        open_comments: &mut Vec<String>,
        open_moves: &mut OpenMoves,
    ) -> Value {
        let props = first_w(Some(node), "pPr");
        let mut collector = ParagraphCollector {
            context: self.context,
            field_meta,
            field_tracker: FieldTracker::new(),
            paragraph_id: id.clone(),
            simple_field_index: 0,
        };
        let (runs, _) = collect_paragraph_runs(node, &mut collector, open_comments, open_moves);
        let runs = if runs.is_empty() {
            vec![json!({ "text": "" })]
        } else {
            runs
        };
        let mut paragraph = json!({
            "type": "paragraph",
            "id": id,
            "runs": runs,
        });
        if let Some(obj) = paragraph.as_object_mut() {
            let direct = crate::styles::parse_direct_paragraph_formatting(props);
            if let Some(d) = direct.as_object() {
                for (k, v) in d {
                    obj.insert(k.clone(), v.clone());
                }
            }
            if let Some(style) = get_w(first_w(props, "pStyle"), "val") {
                obj.insert("style".into(), json!(style));
            }
            let bookmarks = paragraph_bookmark_names(node);
            if !bookmarks.is_empty() {
                obj.insert("bookmarks".into(), json!(bookmarks));
            }
            if let Some(mr) = paragraph_mark_revision(props) {
                obj.insert("markRevision".into(), mr);
            }
            if let Some(fr) = paragraph_format_revision(props) {
                obj.insert("formatRevision".into(), fr);
            }
        }
        paragraph
    }
}

struct DocumentPartsResult {
    sections: Option<Vec<Value>>,
    footnotes: Option<Vec<Value>>,
    endnotes: Option<Vec<Value>>,
    footnote_num_fmt: Option<String>,
    endnote_num_fmt: Option<String>,
    even_and_odd_headers: bool,
    auto_hyphenation: bool,
    warnings: Vec<String>,
}

fn parse_document_parts(
    pkg: &mut Package<'_>,
    body: &Element,
    blocks: &[Value],
    drawings: &DrawingContext,
) -> Result<DocumentPartsResult, OpcError> {
    let mut warnings = Vec::new();
    let raw = parse_raw_sections(body, blocks);
    let relationships = pkg.rels("word/document.xml")?.clone();
    let settings = parse_doc_settings(pkg.text("word/settings.xml")?);
    let mut cache: HashMap<String, Vec<Value>> = HashMap::new();
    let mut field_markup = false;
    let mut sections = Vec::new();
    for raw_section in raw.into_iter() {
        let headers = resolve_slots(
            pkg,
            &relationships,
            &raw_section.header_refs,
            &mut cache,
            &mut field_markup,
            drawings,
            &mut warnings,
        )?;
        let footers = resolve_slots(
            pkg,
            &relationships,
            &raw_section.footer_refs,
            &mut cache,
            &mut field_markup,
            drawings,
            &mut warnings,
        )?;
        let mut section = raw_section.props.as_object().cloned().unwrap_or_default();
        if let Some(h) = headers {
            section.insert("headers".into(), h);
        }
        if let Some(f) = footers {
            section.insert("footers".into(), f);
        }
        sections.push(Value::Object(section));
    }
    let footnotes_xml = pkg.text("word/footnotes.xml")?.map(|s| s.to_string());
    let endnotes_xml = pkg.text("word/endnotes.xml")?.map(|s| s.to_string());
    let footnotes = if let Some(xml) = footnotes_xml {
        let ctx = part_context(pkg, "word/footnotes.xml", drawings)?;
        let mut parser = |container: &Element, prefix: &str| {
            parse_blocks_from_container(container, prefix, ctx.as_ref())
        };
        Some(parse_notes_part(&xml, NoteKind::Footnote, &mut parser))
    } else {
        None
    };
    let endnotes = if let Some(xml) = endnotes_xml {
        let ctx = part_context(pkg, "word/endnotes.xml", drawings)?;
        let mut parser = |container: &Element, prefix: &str| {
            parse_blocks_from_container(container, prefix, ctx.as_ref())
        };
        Some(parse_notes_part(&xml, NoteKind::Endnote, &mut parser))
    } else {
        None
    };
    if sections.iter().any(|s| {
        s.get("columns")
            .and_then(|c| c.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            > 1
    }) {
        warnings.push(
            "Columns render in Print Layout and, for single-section documents, on the editing surface; column breaks balance approximately.".to_string(),
        );
    }
    if field_markup {
        warnings.push(
            "PAGE, NUMPAGES, SECTIONPAGES, DATE and TIME fields are recalculated in Print Layout; other fields show the result Word last saved. Field results are editable and field codes are preserved.".to_string(),
        );
    }
    Ok(DocumentPartsResult {
        sections: if sections.is_empty() {
            None
        } else {
            Some(sections)
        },
        footnotes,
        endnotes,
        footnote_num_fmt: settings.footnote_num_fmt,
        endnote_num_fmt: settings.endnote_num_fmt,
        even_and_odd_headers: settings.even_and_odd_headers,
        auto_hyphenation: settings.auto_hyphenation,
        warnings,
    })
}

struct DocSettings {
    even_and_odd_headers: bool,
    auto_hyphenation: bool,
    footnote_num_fmt: Option<String>,
    endnote_num_fmt: Option<String>,
}

fn parse_doc_settings(xml: Option<&str>) -> DocSettings {
    if xml.is_none() {
        return DocSettings {
            even_and_odd_headers: false,
            auto_hyphenation: false,
            footnote_num_fmt: None,
            endnote_num_fmt: None,
        };
    }
    let document = parse_xml_str(xml.unwrap()).expect("settings");
    let root = &document.root;
    DocSettings {
        even_and_odd_headers: on_off_element(first_w(Some(root), "evenAndOddHeaders"))
            == Some(true),
        auto_hyphenation: on_off_element(first_w(Some(root), "autoHyphenation")) == Some(true),
        footnote_num_fmt: get_w(first_w(first_w(Some(root), "footnotePr"), "numFmt"), "val"),
        endnote_num_fmt: get_w(first_w(first_w(Some(root), "endnotePr"), "numFmt"), "val"),
    }
}

fn part_context(
    pkg: &mut Package<'_>,
    path: &str,
    base: &DrawingContext,
) -> Result<Option<DrawingContext>, OpcError> {
    let rels = pkg.rels(path)?.clone();
    Ok(Some(DrawingContext {
        rels,
        content_types: base.content_types.clone(),
        media_parts: base.media_parts.clone(),
    }))
}

fn resolve_slots(
    pkg: &mut Package<'_>,
    relationships: &IndexMap<String, Relationship>,
    refs: &[RawHeaderFooterRef],
    cache: &mut HashMap<String, Vec<Value>>,
    field_markup: &mut bool,
    drawings: &DrawingContext,
    _warnings: &mut Vec<String>,
) -> Result<Option<Value>, OpcError> {
    if refs.is_empty() {
        return Ok(None);
    }
    let mut slots = serde_json::Map::new();
    for ref_item in refs {
        let relationship = relationships.get(&ref_item.r_id);
        if relationship.is_none() {
            continue;
        }
        let path = pkg
            .target("word/document.xml", relationship.unwrap())
            .unwrap_or_default();
        let blocks = if let Some(b) = cache.get(&path) {
            b.clone()
        } else {
            let xml = pkg.text(&path)?;
            if xml.is_none() {
                continue;
            }
            let xml = xml.unwrap();
            if xml.contains("fldSimple") || xml.contains("instrText") || xml.contains("fldChar") {
                *field_markup = true;
            }
            let root = parse_xml_str(xml)?.root;
            let prefix = format!(
                "{}-",
                path.replace(|c: char| !c.is_ascii_alphanumeric(), "")
            );
            let ctx = part_context(pkg, &path, drawings)?;
            let blocks = parse_blocks_from_container(&root, &prefix, ctx.as_ref());
            cache.insert(path.clone(), blocks.clone());
            blocks
        };
        slots.insert(
            ref_item.slot.clone(),
            json!({ "blocks": blocks, "partName": path }),
        );
    }
    if slots.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(slots)))
    }
}

fn px(twips: Option<String>, fallback: f64) -> f64 {
    twips
        .and_then(|t| t.parse::<f64>().ok())
        .map(|v| v * PX_PER_TWIP)
        .unwrap_or(fallback)
}

struct ParagraphCollector<'a> {
    context: Option<&'a DrawingContext>,
    field_meta: &'a HashMap<usize, (Value, Option<Value>)>,
    field_tracker: FieldTracker,
    paragraph_id: String,
    simple_field_index: usize,
}

impl RunCollector for ParagraphCollector<'_> {
    fn parse_run(&mut self, run_el: &Element, revision: Option<Value>) -> Value {
        let mut run = parse_run(run_el, revision, self.context);
        self.field_tracker.track(run_el, &mut run);
        let key = run_el as *const Element as usize;
        if let Some((field, flags)) = self.field_meta.get(&key) {
            if let Some(obj) = run.as_object_mut() {
                obj.insert("field".into(), field.clone());
                if let Some(f) = flags {
                    obj.insert("fieldFlags".into(), f.clone());
                }
            }
        }
        run
    }

    fn parse_other(&mut self, item: &Element) -> Option<Vec<Value>> {
        if let Some(eq) = parse_equation(item) {
            return Some(vec![eq]);
        }
        if !named_w(item, "fldSimple") {
            return None;
        }
        let instr = get_w(Some(item), "instr").unwrap_or_default();
        let link = parse_simple_hyperlink_field(&instr);
        let field = json!({ "instr": instr.trim(), "simple": true });
        let flags = crate::field_runs::parse_field_flags(item);
        let field_instance_id = format!(
            "{}:simple-field-{}",
            self.paragraph_id, self.simple_field_index
        );
        self.simple_field_index += 1;
        let text = field_placeholder_text(Some(&instr));
        let mut run = json!({
            "text": text,
            "field": field,
            "fieldInstanceId": field_instance_id,
        });
        if let Some(f) = flags {
            if let Some(obj) = run.as_object_mut() {
                obj.insert("fieldFlags".into(), f);
            }
        }
        if let Some(l) = link {
            if let Some(obj) = run.as_object_mut() {
                obj.insert("link".into(), l);
            }
        }
        Some(vec![run])
    }

    fn resolve_link(&self, hyperlink: &Element) -> Option<Value> {
        self.context.map(|c| resolve_hyperlink(hyperlink, &c.rels))
    }
}

fn table_has_nested(block: &Value) -> bool {
    if block.get("type").and_then(|v| v.as_str()) != Some("table") {
        return false;
    }
    block
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter().any(|row| {
                row.as_array()
                    .map(|cells| {
                        cells.iter().any(|cell| {
                            cell.get("nestedTables")
                                .and_then(|v| v.as_array())
                                .map(|a| !a.is_empty())
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}
