// SPDX-License-Identifier: MIT
//! PPTX package writer (#87): identity shortcut for clean hosts; dirty saves
//! rewrite only the slide parts marked by shape commands.

mod pictures;
mod slide_xml;
mod structure;

use std::collections::{BTreeSet, HashMap};

use indexmap::IndexMap;

use nitro_xml_opc::opc::rel_types::SLIDE_LAYOUT;
use nitro_xml_opc::opc::{relationships_part_for, Package, Relationship};
use nitro_xml_opc::write::PackageWriter;

use crate::edit::host::PptxHost;

use self::pictures::{attach_pictures, plan_pictures, rels_xml, retarget_slide_layout};
use self::slide_xml::{max_numeric_id, rewrite_slide_with_pics};

/// Options for [`save_pptx_with_options`].
#[derive(Debug, Clone, Default)]
pub struct PptxSaveOptions {
    /// Negative control (#87): skip rewriting dirty `ppt/slides/slideN.xml` parts.
    pub skip_slide_rewrite: bool,
}

/// Save `host` over `original` package bytes.
pub fn save_pptx(host: &PptxHost, original: &[u8]) -> Result<Vec<u8>, String> {
    save_pptx_with_options(host, original, &PptxSaveOptions::default())
}

/// [`save_pptx`] with explicit options (see [`PptxSaveOptions::skip_slide_rewrite`]).
///
/// A clean host, or `skip_slide_rewrite`, returns the original bytes through
/// [`PackageWriter`]'s identity shortcut. A slide-list edit rebuilds
/// `p:sldIdLst`, relationships and slide parts. Otherwise only
/// `host.dirty_parts` slide XML is replaced. A layout override rewrites only
/// that slide's `slideLayout` relationship, leaving other relationships in
/// place. Notes parts are TODO(#86); rich
/// runs and autofit are TODO(#85) / TODO(#86). List markers (`a:buChar`,
/// `a:buAutoNum`, `a:buNone`), authored tables (`a:tbl`), and inserted
/// pictures (`p:pic` plus `ppt/media`) are written inside the dirty slide.
pub fn save_pptx_with_options(
    host: &PptxHost,
    original: &[u8],
    options: &PptxSaveOptions,
) -> Result<Vec<u8>, String> {
    if !host.is_dirty() || options.skip_slide_rewrite {
        return PackageWriter::open(original)
            .map_err(|e| e.to_string())?
            .finish()
            .map_err(|e| e.to_string());
    }
    if host.structure_dirty {
        return structure::save_with_structure(host, original);
    }

    let mut pkg = Package::open(original).map_err(|e| e.to_string())?;
    let mut used_parts: BTreeSet<String> = pkg.part_names().map(str::to_owned).collect();
    let mut max_id = 0i64;
    let mut sources: HashMap<String, String> = HashMap::new();
    let mut pic_plans = HashMap::new();
    let mut layout_rels: HashMap<String, IndexMap<String, Relationship>> = HashMap::new();
    for slide in &host.slides {
        let xml = {
            let text = pkg.text(&slide.path).map_err(|e| e.to_string())?;
            text.map(str::to_owned)
        };
        let content_dirty = host.dirty_parts.contains(&slide.path);
        if let Some(xml) = xml.as_ref() {
            max_id = max_id.max(max_numeric_id(xml)?);
        }
        if !content_dirty && slide.layout_target.is_none() {
            continue;
        }
        let Some(xml) = xml else {
            if content_dirty {
                return Err(format!("missing part {}", slide.path));
            }
            continue;
        };
        let rels = pkg.rels(&slide.path).map_err(|e| e.to_string())?.clone();
        if content_dirty {
            let (map, planned) = plan_pictures(slide, &rels, &mut used_parts);
            pic_plans.insert(slide.path.clone(), (rels.clone(), map, planned));
            sources.insert(slide.path.clone(), xml);
        }
        if let Some(target) = slide.layout_target.as_deref() {
            if let Some(next) = retarget_slide_layout(&rels, target) {
                layout_rels.insert(slide.path.clone(), next);
            }
        }
    }

    // Upstream `PptxElementXmlBuilder.nextId` is one past the max `@_id` across
    // every slide part, then incremented as new shapes are serialized.
    let mut next_id = max_id + 1;
    let mut writer = PackageWriter::open(original).map_err(|e| e.to_string())?;
    for slide in &host.slides {
        let content_dirty = host.dirty_parts.contains(&slide.path);
        if content_dirty {
            let src = sources
                .get(&slide.path)
                .ok_or_else(|| format!("missing dirty part {}", slide.path))?;
            let (rels, map, planned) = pic_plans
                .get(&slide.path)
                .ok_or_else(|| format!("missing picture plan {}", slide.path))?;
            let rewritten = rewrite_slide_with_pics(src, slide, &mut next_id, map)?;
            // `replace` keeps content types. Picture saves also add media and a slide rel.
            writer.replace(&slide.path, rewritten.into_bytes());
            let rels_out = layout_rels.get(&slide.path).unwrap_or(rels);
            attach_pictures(&mut writer, &slide.path, rels_out, planned)?;
            if layout_rels.contains_key(&slide.path) && planned.is_empty() {
                writer.add_text(
                    relationships_part_for(&slide.path),
                    rels_xml(rels_out),
                    None,
                );
            }
        } else if let Some(next) = layout_rels.get(&slide.path) {
            writer.add_text(relationships_part_for(&slide.path), rels_xml(next), None);
        }
    }
    writer.finish().map_err(|e| e.to_string())
}

/// Next layout override for `slide_index`.
///
/// `Ok(None)` means the click changes nothing (one layout, or the next target
/// is already current). `Ok(Some(None))` clears an override that cycled back to
/// the package target. `Ok(Some(Some(target)))` stores a relative relationship
/// target such as `../slideLayouts/slideLayout2.xml`.
pub(crate) fn next_layout_override(
    package: &[u8],
    host: &PptxHost,
    slide_index: u32,
) -> Result<Option<Option<String>>, String> {
    let slide = host.slide(slide_index)?;
    let mut pkg = Package::open(package).map_err(|e| e.to_string())?;
    let mut layouts: Vec<(u32, String)> = pkg
        .part_names()
        .filter_map(|name| {
            let name = name.trim_start_matches('/');
            let file = name.rsplit('/').next()?;
            if !name.starts_with("ppt/slideLayouts/slideLayout")
                || !file.ends_with(".xml")
                || name.contains("_rels")
            {
                return None;
            }
            let num = file
                .strip_prefix("slideLayout")?
                .strip_suffix(".xml")?
                .parse::<u32>()
                .ok()?;
            Some((num, file.to_owned()))
        })
        .collect();
    layouts.sort_by_key(|(n, _)| *n);
    layouts.dedup_by(|a, b| a.1 == b.1);
    if layouts.len() < 2 {
        return Ok(None);
    }
    let rels = match pkg.rels(&slide.path) {
        Ok(rels) => rels.clone(),
        Err(_) => return Ok(None),
    };
    let authored = rels
        .values()
        .find(|rel| rel.type_ == SLIDE_LAYOUT)
        .map(|rel| rel.target.clone());
    let Some(authored) = authored else {
        return Ok(None);
    };
    let current = slide
        .layout_target
        .clone()
        .unwrap_or_else(|| authored.clone());
    let current_file = layout_file(&current);
    let pos = layouts
        .iter()
        .position(|(_, file)| file == current_file)
        .unwrap_or(0);
    let next_file = &layouts[(pos + 1) % layouts.len()].1;
    if next_file == current_file {
        return Ok(None);
    }
    let authored_file = layout_file(&authored);
    if next_file == authored_file {
        if slide.layout_target.is_none() {
            return Ok(None);
        }
        return Ok(Some(None));
    }
    Ok(Some(Some(format!("../slideLayouts/{next_file}"))))
}

fn layout_file(target: &str) -> &str {
    target.rsplit(['/', '\\']).next().unwrap_or(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::host::{GeometryPatch, PptxHost};

    // Catches: clean-host save taking the PackageWriter identity shortcut without touching slide XML.
    #[test]
    fn clean_host_returns_original_bytes() {
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let host = PptxHost::open(bytes.as_slice()).expect("open");
        assert!(!host.is_dirty());
        let out = save_pptx(&host, bytes.as_slice()).expect("save");
        assert_eq!(out.as_slice(), bytes.as_slice());
    }

    // Catches: --skip-slide-rewrite still rewriting a dirty slide.
    #[test]
    fn skip_slide_rewrite_leaves_original_bytes() {
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_shape_text(0, None, Some(0), "Rewritten title")
            .expect("text");
        assert!(host.is_dirty());
        let skipped = save_pptx_with_options(
            &host,
            bytes.as_slice(),
            &PptxSaveOptions {
                skip_slide_rewrite: true,
            },
        )
        .expect("skip");
        assert_eq!(skipped.as_slice(), bytes.as_slice());
        let dirty = save_pptx(&host, bytes.as_slice()).expect("dirty");
        assert_ne!(dirty.as_slice(), bytes.as_slice());
        let xml = slide_xml(&dirty, "ppt/slides/slide1.xml");
        assert!(xml.contains("Rewritten title"), "{xml}");
        assert!(!slide_xml(&dirty, "ppt/slides/slide2.xml").contains("Rewritten title"));
    }

    #[test]
    fn catches_underline_and_align_dropped_on_slide_save() {
        // Catches: a slide save that drops a:rPr/@u and a:pPr/@algn, so reopen loses underline and center.
        use crate::edit::host::TextStylePatch;
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                underline: Some(true),
                ..TextStylePatch::default()
            },
        )
        .expect("underline");
        host.set_paragraph_align(0, None, Some(0), 0, "center")
            .expect("align");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let again = PptxHost::open(&saved).expect("reopen");
        let el = &again.slides[0].elements[0];
        assert!(
            el.text_segments.iter().any(|s| s.underline),
            "reopened shape lost underline"
        );
        assert!(
            el.text_segments
                .iter()
                .any(|s| s.align.as_deref() == Some("center")),
            "reopened shape lost center align: {:?}",
            el.text_segments
        );
    }

    #[test]
    fn catches_strikethrough_dropped_on_slide_save() {
        // Catches: a slide save that omits a:rPr/@strike, so reopen loses strikethrough.
        use crate::edit::host::TextStylePatch;
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                strike: Some(true),
                ..TextStylePatch::default()
            },
        )
        .expect("strike");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let again = PptxHost::open(&saved).expect("reopen");
        let el = &again.slides[0].elements[0];
        assert!(
            el.text_segments.iter().any(|s| s.strike),
            "reopened shape lost strikethrough"
        );
    }

    #[test]
    fn catches_double_strike_dropped_on_slide_save() {
        // Catches: a slide save that omits a:rPr/@strike="dblStrike", so reopen draws one line.
        use crate::edit::host::TextStylePatch;
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                double_strike: Some(true),
                ..TextStylePatch::default()
            },
        )
        .expect("double strike");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(
            xml.contains("strike=\"dblStrike\""),
            "saved slide dropped dblStrike: {xml}"
        );
        let again = PptxHost::open(&saved).expect("reopen");
        let el = &again.slides[0].elements[0];
        assert!(
            el.text_segments.iter().any(|s| s.double_strike),
            "reopened shape lost double strikethrough"
        );
    }

    #[test]
    fn catches_double_strike_and_single_strike_both_written_on_one_attr() {
        // Catches: double strikethrough and single strikethrough both surviving on one a:rPr/@strike.
        use crate::edit::host::TextStylePatch;
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                double_strike: Some(true),
                strike: Some(false),
                ..TextStylePatch::default()
            },
        )
        .expect("double");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(
            xml.contains("strike=\"dblStrike\""),
            "missing dblStrike: {xml}"
        );
        assert!(
            !xml.contains("strike=\"sngStrike\""),
            "kept sngStrike: {xml}"
        );

        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                double_strike: Some(false),
                strike: Some(true),
                ..TextStylePatch::default()
            },
        )
        .expect("single");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save single");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(
            xml.contains("strike=\"sngStrike\""),
            "missing sngStrike: {xml}"
        );
        assert!(
            !xml.contains("strike=\"dblStrike\""),
            "kept dblStrike: {xml}"
        );
    }

    #[test]
    fn catches_small_caps_dropped_on_slide_save() {
        // Catches: a slide save that omits a:rPr/@cap="small", so reopen draws the run at full cap height.
        use crate::edit::host::TextStylePatch;
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                small_caps: Some(true),
                ..TextStylePatch::default()
            },
        )
        .expect("small caps");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(
            xml.contains("cap=\"small\""),
            "saved slide dropped cap=small: {xml}"
        );
        let again = PptxHost::open(&saved).expect("reopen");
        let el = &again.slides[0].elements[0];
        assert!(
            el.text_segments.iter().any(|s| s.small_caps),
            "reopened shape lost small caps"
        );
    }

    #[test]
    fn catches_all_caps_dropped_on_slide_save() {
        // Catches: a slide save that omits a:rPr/@cap="all", so reopen leaves the run in sentence case.
        use crate::edit::host::TextStylePatch;
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                all_caps: Some(true),
                ..TextStylePatch::default()
            },
        )
        .expect("all caps");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(
            xml.contains("cap=\"all\""),
            "saved slide dropped cap=all: {xml}"
        );
        let again = PptxHost::open(&saved).expect("reopen");
        let el = &again.slides[0].elements[0];
        assert!(
            el.text_segments.iter().any(|s| s.all_caps),
            "reopened shape lost all caps"
        );
    }

    #[test]
    fn catches_all_caps_and_small_caps_both_written_on_one_cap_attr() {
        // Catches: all caps and small caps both surviving on one a:rPr/@cap, or cap="small" winning over cap="all".
        use crate::edit::host::TextStylePatch;
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                all_caps: Some(true),
                small_caps: Some(false),
                ..TextStylePatch::default()
            },
        )
        .expect("all caps");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(xml.contains("cap=\"all\""), "missing cap=all: {xml}");
        assert!(!xml.contains("cap=\"small\""), "kept cap=small: {xml}");

        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                all_caps: Some(false),
                small_caps: Some(true),
                ..TextStylePatch::default()
            },
        )
        .expect("small caps");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save small");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(xml.contains("cap=\"small\""), "missing cap=small: {xml}");
        assert!(!xml.contains("cap=\"all\""), "kept cap=all: {xml}");
    }

    #[test]
    fn catches_indent_spacing_baseline_and_typeface_dropped_on_slide_save() {
        // Catches: a slide save that omits a:pPr/@marL, a:lnSpc/a:spcPct, a:rPr/@baseline, or a:latin/@typeface.
        use crate::edit::host::TextStylePatch;
        use crate::edit::text::{INDENT_STEP_EMU, LINE_ONE_HALF_PCT};
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        assert!(host
            .adjust_indent(0, None, Some(0), INDENT_STEP_EMU)
            .expect("indent"));
        host.cycle_line_spacing(0, None, Some(0)).expect("spacing");
        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                baseline: Some(30_000),
                typeface: Some("Georgia"),
                ..TextStylePatch::default()
            },
        )
        .expect("style");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let again = PptxHost::open(&saved).expect("reopen");
        let el = &again.slides[0].elements[0];
        let seg = el
            .text_segments
            .iter()
            .find(|s| s.typeface.as_deref() == Some("Georgia"))
            .expect("typeface missing after reopen");
        assert_eq!(
            seg.indent_left_emu,
            Some(INDENT_STEP_EMU),
            "{:?}",
            el.text_segments
        );
        assert_eq!(seg.line_spacing_pct, Some(LINE_ONE_HALF_PCT));
        assert_eq!(seg.baseline, Some(30_000));

        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                baseline: Some(-25_000),
                ..TextStylePatch::default()
            },
        )
        .expect("sub");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save sub");
        let again = PptxHost::open(&saved).expect("reopen sub");
        assert!(
            again.slides[0].elements[0]
                .text_segments
                .iter()
                .any(|s| s.baseline == Some(-25_000)),
            "subscript baseline missing"
        );
    }

    #[test]
    fn catches_justify_and_list_markers_dropped_on_slide_save() {
        // Catches: a slide save that omits a:pPr/@algn="just", a:buChar, or a:buAutoNum.
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_paragraph_align(0, None, Some(0), 0, "justify")
            .expect("justify");
        host.set_bullets(0, None, Some(0), "bullet", None, None)
            .expect("bullets");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save bullets");
        let again = PptxHost::open(&saved).expect("reopen bullets");
        let el = &again.slides[0].elements[0];
        let info = el
            .text_segments
            .iter()
            .find_map(|s| s.bullet_info.as_ref())
            .expect("bullet marker missing after reopen");
        assert_eq!(info.char.as_deref(), Some("•"));
        assert!(
            el.text_segments
                .iter()
                .any(|s| s.align.as_deref() == Some("justify")),
            "justify missing after reopen: {:?}",
            el.text_segments
        );

        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_bullets(0, None, Some(0), "numbered", None, None)
            .expect("numbering");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save numbering");
        let again = PptxHost::open(&saved).expect("reopen numbering");
        let info = again.slides[0].elements[0]
            .text_segments
            .iter()
            .find_map(|s| s.bullet_info.as_ref())
            .expect("numbering missing after reopen");
        assert_eq!(info.auto_num_type.as_deref(), Some("arabicPeriod"));
    }

    // Catches: new, duplicated and deleted slides never landing in the package, and a text box saved without txBox.
    #[test]
    fn catches_slide_structure_and_text_box_missing_after_reopen() {
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let opened = PptxHost::open(bytes.as_slice()).expect("open");
        let before = opened.slides.len();
        let original_text = slide_plain(&opened.slides[0]);
        assert!(
            !original_text.is_empty(),
            "fixture slide has no text to duplicate"
        );

        let mut host = opened.clone();
        host.add_slide(None).expect("add");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save add");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let again = PptxHost::open(&saved).expect("reopen add");
        assert_eq!(
            again.slides.len(),
            before + 1,
            "new slide missing after reopen"
        );

        let mut host = opened.clone();
        host.duplicate_slide(0).expect("dup");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save dup");
        let again = PptxHost::open(&saved).expect("reopen dup");
        assert_eq!(again.slides.len(), before + 1);
        assert_eq!(
            slide_plain(&again.slides[1]),
            original_text,
            "duplicated slide text diverged"
        );

        let mut host = opened.clone();
        let removed = host.slides[before - 1].path.clone();
        host.delete_slide((before - 1) as u32).expect("delete");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save delete");
        let again = PptxHost::open(&saved).expect("reopen delete");
        assert_eq!(
            again.slides.len(),
            before - 1,
            "deleted slide still in the package"
        );
        assert!(
            Package::open(saved.as_slice())
                .expect("pkg")
                .text(&removed)
                .expect("read")
                .is_none(),
            "deleted slide part {removed} was left in the package"
        );

        let mut host = opened;
        let count = host.slides[0].elements.len();
        host.add_text_box(0, "text-box-0", 80.0, 80.0, 240.0, 80.0)
            .expect("text box");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save text box");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(
            xml.contains("txBox=\"1\""),
            "text box was not written: {xml}"
        );
        let again = PptxHost::open(&saved).expect("reopen text box");
        assert!(
            again.slides[0].elements.len() > count,
            "text box missing after reopen"
        );
    }

    // Catches: saving a moved shape re-quantizes an untouched non-multiple EMU axis.
    #[test]
    fn untouched_emu_axis_survives_partial_move() {
        let bytes = emu_package();
        let mut host = PptxHost::open(&bytes).expect("open");
        let before = host.slide(0).expect("slide").elements[0].x_emu;
        assert_eq!(before, Some(10_000), "parser dropped the stored x EMU");
        host.patch_geometry(
            0,
            None,
            Some(0),
            GeometryPatch {
                y: Some(40.0),
                ..GeometryPatch::default()
            },
        )
        .expect("move");
        let saved = save_pptx(&host, &bytes).expect("save");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(
            xml.contains("x=\"10000\""),
            "untouched axis was re-quantized: {xml}"
        );
        assert!(
            xml.contains("y=\"381000\""),
            "moved axis was not re-quantized: {xml}"
        );
    }

    #[test]
    fn catches_reordered_slides_restored_to_the_open_order() {
        // Catches: reorder writing the slide list in the original order, so reopen shows slide 1 first.
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        let first = host.slides[0].path.clone();
        let second = host.slides[1].path.clone();
        host.reorder_slide(0, 1).expect("reorder");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let again = PptxHost::open(&saved).expect("reopen");
        assert_eq!(again.slides[0].path, second);
        assert_eq!(again.slides[1].path, first);
    }

    #[test]
    fn catches_inserted_slide_table_missing_rows_after_reopen() {
        // Catches: insert table / row / column edits not writing a:tbl, so reopen has no extra grid.
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.add_table(0, "table-0", 80.0, 80.0, 360.0, 120.0)
            .expect("table");
        host.insert_slide_row(0, Some("table-0"), None, 1, "below")
            .expect("row");
        host.insert_slide_column(0, Some("table-0"), None, 1, "right")
            .expect("col");
        host.set_slide_cell_text(0, Some("table-0"), None, 2, 2, "tail")
            .expect("cell");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let again = PptxHost::open(&saved).expect("reopen");
        let table = again.slides[0]
            .elements
            .iter()
            .find(|el| el.type_ == "table")
            .expect("table missing after reopen");
        let model = table.table.as_ref().expect("table model");
        assert_eq!(model.rows.len(), 3, "inserted row missing");
        assert_eq!(model.column_widths.len(), 3, "inserted column missing");
        assert_eq!(model.rows[2].cells[2].text, "tail");
        host.delete_slide_row(0, Some("table-0"), None, 2)
            .expect("delete row");
        host.delete_slide_column(0, Some("table-0"), None, 2)
            .expect("delete col");
        let trimmed = save_pptx(&host, bytes.as_slice()).expect("save trim");
        let trimmed_host = PptxHost::open(&trimmed).expect("reopen trim");
        let trimmed_table = trimmed_host.slides[0]
            .elements
            .iter()
            .find(|el| el.type_ == "table")
            .expect("table missing after delete");
        let trimmed_model = trimmed_table.table.as_ref().expect("model");
        assert_eq!(trimmed_model.rows.len(), 2);
        assert_eq!(trimmed_model.column_widths.len(), 2);
    }

    #[test]
    fn catches_pasted_shape_overwriting_the_original() {
        // Catches: Paste of a shape appending its text, or moving the original onto the copy.
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        let original = host.slides[0].elements[0].clone();
        let text = original.text.clone().unwrap_or_else(|| {
            original
                .text_segments
                .iter()
                .map(|seg| seg.text.clone())
                .collect()
        });
        assert!(!text.is_empty(), "fixture shape has no text");
        let before = host.slides[0].elements.len();
        host.paste_shape(0, &original).expect("paste");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let again = PptxHost::open(&saved).expect("reopen");
        assert_eq!(again.slides[0].elements.len(), before + 1);
        let label = |el: &crate::edit::host::EditableElement| {
            el.text.clone().unwrap_or_else(|| {
                el.text_segments
                    .iter()
                    .map(|seg| seg.text.clone())
                    .collect()
            })
        };
        let kept = again.slides[0].elements.iter().any(|el| {
            label(el).contains(&text)
                && (el.x - original.x).abs() < 1.0
                && (el.y - original.y).abs() < 1.0
        });
        assert!(kept, "paste moved the original shape");
        let copied = again.slides[0].elements.iter().any(|el| {
            label(el).contains(&text)
                && (el.x - (original.x + 20.0)).abs() < 1.0
                && (el.y - (original.y + 20.0)).abs() < 1.0
        });
        assert!(
            copied,
            "paste did not insert the shape 20 px down and right from {},{} text {:?}: {:?}",
            original.x,
            original.y,
            text,
            again.slides[0]
                .elements
                .iter()
                .map(|el| (el.type_.clone(), el.x, el.y, label(el)))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn catches_inserted_picture_missing_from_slide_and_media() {
        // Catches: a picture save that omits p:pic or the PNG part, so reopen has no picture.
        use crate::edit::picture::INSERTED_PNG;
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        let before = host.slides[0].elements.len();
        host.add_picture(0, "picture-test", INSERTED_PNG)
            .expect("picture");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let again = PptxHost::open(&saved).expect("reopen");
        let pictures = again.slides[0]
            .elements
            .iter()
            .filter(|el| el.type_ == "picture")
            .count();
        assert_eq!(pictures, 1, "reopen dropped the picture");
        assert_eq!(again.slides[0].elements.len(), before + 1);
        let mut pkg = Package::open(&saved).expect("pkg");
        let media = pkg
            .part_names()
            .find(|n| n.starts_with("ppt/media/nitro-") && n.ends_with(".png"))
            .expect("media part")
            .to_owned();
        let png = pkg.read(&media).expect("read").expect("bytes");
        assert_eq!(png, INSERTED_PNG);
    }

    // Catches: shape fill and outline staying off the saved slide, or a second click
    // rewriting the package, or undo saving the painted bytes.
    #[test]
    fn shape_fill_and_outline_round_trip_and_undo() {
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut editor = crate::editor::PptxEditor::open(bytes.as_slice()).expect("open");
        let id = editor.session.host.slides[0].elements[0].id.clone();
        editor.select_elements(std::iter::once(id));
        editor
            .dispatch_command("home.shape-fill", &serde_json::Value::Null)
            .expect("fill");
        let filled = save_pptx(&editor.session.host, bytes.as_slice()).expect("save fill");
        assert_ne!(filled.as_slice(), bytes.as_slice());
        let fill_xml = part_text(&filled, "ppt/slides/slide1.xml");
        assert!(
            fill_xml.contains("val=\"5B2C6F\""),
            "reopen lost the shape fill: {fill_xml}"
        );
        assert!(
            fill_xml.contains("Slide 1"),
            "fill rewrite dropped the title"
        );
        let reopened = PptxHost::open(&filled).expect("reopen fill");
        assert!(!reopened.is_dirty());
        assert_eq!(
            save_pptx(&reopened, &filled).expect("identity").as_slice(),
            filled.as_slice()
        );
        editor
            .dispatch_command("home.shape-fill", &serde_json::Value::Null)
            .expect("fill again");
        assert_eq!(
            save_pptx(&editor.session.host, bytes.as_slice())
                .expect("second fill")
                .as_slice(),
            filled.as_slice()
        );
        editor
            .dispatch_command("edit.undo", &serde_json::Value::Null)
            .expect("undo fill");
        assert_eq!(
            save_pptx(&editor.session.host, bytes.as_slice())
                .expect("undone fill")
                .as_slice(),
            bytes.as_slice()
        );

        editor
            .dispatch_command("home.shape-outline", &serde_json::Value::Null)
            .expect("outline");
        let outlined = save_pptx(&editor.session.host, bytes.as_slice()).expect("save outline");
        assert_ne!(outlined.as_slice(), bytes.as_slice());
        let line_xml = part_text(&outlined, "ppt/slides/slide1.xml");
        assert!(
            line_xml.contains("val=\"00B050\""),
            "reopen lost the outline: {line_xml}"
        );
        editor
            .dispatch_command("home.shape-outline", &serde_json::Value::Null)
            .expect("outline again");
        assert_eq!(
            save_pptx(&editor.session.host, bytes.as_slice())
                .expect("second outline")
                .as_slice(),
            outlined.as_slice()
        );
        editor
            .dispatch_command("edit.undo", &serde_json::Value::Null)
            .expect("undo outline");
        assert_eq!(
            save_pptx(&editor.session.host, bytes.as_slice())
                .expect("undone outline")
                .as_slice(),
            bytes.as_slice()
        );
    }

    #[test]
    fn catches_dashed_outline_painting_a_solid_line() {
        // Catches: Dashed outline leaving a:prstDash off the saved slide, so reopen paints a solid stroke.
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut editor = crate::editor::PptxEditor::open(bytes.as_slice()).expect("open");
        let id = editor.session.host.slides[0].elements[0].id.clone();
        editor.select_elements(std::iter::once(id));
        editor
            .dispatch_command("home.shape-dash", &serde_json::Value::Null)
            .expect("dash");
        let dashed = save_pptx(&editor.session.host, bytes.as_slice()).expect("save dash");
        assert_ne!(dashed.as_slice(), bytes.as_slice());
        let dash_xml = part_text(&dashed, "ppt/slides/slide1.xml");
        assert!(
            dash_xml.contains("<a:prstDash") && dash_xml.contains("val=\"dash\""),
            "saved slide has no dashed outline: {dash_xml}"
        );
        assert_eq!(reopened_dash(&dashed).as_deref(), Some("dash"));
        editor
            .dispatch_command("home.shape-dash", &serde_json::Value::Null)
            .expect("solid");
        let solid = save_pptx(&editor.session.host, bytes.as_slice()).expect("save solid");
        let solid_xml = part_text(&solid, "ppt/slides/slide1.xml");
        assert!(
            !solid_xml.contains("prstDash"),
            "toggling dashed outline left the dash on the slide: {solid_xml}"
        );
        assert!(
            solid_xml.contains("val=\"00B050\""),
            "solid toggle dropped the outline: {solid_xml}"
        );
        assert_eq!(reopened_dash(&solid), None);
        editor
            .dispatch_command("edit.undo", &serde_json::Value::Null)
            .expect("undo solid");
        assert_eq!(
            save_pptx(&editor.session.host, bytes.as_slice())
                .expect("dashed again")
                .as_slice(),
            dashed.as_slice()
        );
        editor
            .dispatch_command("home.shape-outline", &serde_json::Value::Null)
            .expect("outline clears dash");
        let outlined = save_pptx(&editor.session.host, bytes.as_slice()).expect("outline");
        let outlined_xml = part_text(&outlined, "ppt/slides/slide1.xml");
        assert!(
            !outlined_xml.contains("prstDash") && outlined_xml.contains("val=\"00B050\""),
            "outline left the dash in place: {outlined_xml}"
        );
        editor
            .dispatch_command("edit.undo", &serde_json::Value::Null)
            .expect("undo outline");
        editor
            .dispatch_command("edit.undo", &serde_json::Value::Null)
            .expect("undo dash");
        assert_eq!(
            save_pptx(&editor.session.host, bytes.as_slice())
                .expect("undone")
                .as_slice(),
            bytes.as_slice()
        );
    }

    fn reopened_dash(bytes: &[u8]) -> Option<String> {
        let mut pres = crate::Presentation::open(bytes).expect("reopen");
        let slide = pres.slide(0).expect("slide");
        let path = slide.path.clone();
        slide
            .elements
            .iter()
            .find(|el| el.id.starts_with(&path))
            .and_then(|el| el.stroke_dash.clone())
    }

    // Catches: Layout dirtying a one-layout deck, or a multi-layout change dropping
    // the other slide relationships, or undo failing to restore the original bytes.
    #[test]
    fn slide_layout_round_trip_preserves_other_rels_and_undo() {
        let one = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut editor = crate::editor::PptxEditor::open(one.as_slice()).expect("open one");
        editor
            .dispatch_command("home.layout", &serde_json::Value::Null)
            .expect("layout one");
        assert_eq!(
            save_pptx(&editor.session.host, one.as_slice())
                .expect("one save")
                .as_slice(),
            one.as_slice()
        );

        let bytes =
            include_bytes!("../../../../../fixtures/upstream/pptx/51edb06a949d_text-layout.pptx");
        let mut editor = crate::editor::PptxEditor::open(bytes.as_slice()).expect("open");
        editor
            .dispatch_command("home.layout", &serde_json::Value::Null)
            .expect("layout");
        let saved = save_pptx(&editor.session.host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let rels = part_text(&saved, "ppt/slides/_rels/slide1.xml.rels");
        assert!(
            rels.contains("../slideLayouts/slideLayout2.xml"),
            "layout target unchanged: {rels}"
        );
        assert!(
            rels.contains("https://example.com/docs"),
            "layout rewrite dropped the hyperlink: {rels}"
        );
        assert!(rels.contains("TargetMode=\"External\""));
        let mut pres = crate::Presentation::open(&saved).expect("reopen");
        let slide = pres.slide(0).expect("slide");
        assert_eq!(
            slide.layout_path.as_deref(),
            Some("ppt/slideLayouts/slideLayout2.xml")
        );
        let reopened = PptxHost::open(&saved).expect("host");
        assert!(!reopened.is_dirty());
        assert_eq!(
            save_pptx(&reopened, &saved).expect("identity").as_slice(),
            saved.as_slice()
        );
        editor
            .dispatch_command("insert.slide", &serde_json::Value::Null)
            .expect("insert");
        let with_slide = save_pptx(&editor.session.host, bytes.as_slice()).expect("structure");
        let rels = part_text(&with_slide, "ppt/slides/_rels/slide1.xml.rels");
        assert!(
            rels.contains("../slideLayouts/slideLayout2.xml"),
            "insert slide dropped the layout override: {rels}"
        );
        editor
            .dispatch_command("edit.undo", &serde_json::Value::Null)
            .expect("undo insert");
        editor
            .dispatch_command("edit.undo", &serde_json::Value::Null)
            .expect("undo layout");
        assert_eq!(
            save_pptx(&editor.session.host, bytes.as_slice())
                .expect("undone")
                .as_slice(),
            bytes.as_slice()
        );
    }

    #[test]
    fn catches_grouped_shapes_omitted_from_slide_save() {
        // Catches: Group leaving the two shapes out of the saved slide because p:grpSp was skipped.
        use crate::edit::host::NewShape;
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.add_shape(
            0,
            NewShape {
                shape_type: "rect",
                x: 320.0,
                y: 180.0,
                width: 160.0,
                height: 80.0,
                id: Some("rect-b"),
            },
        )
        .expect("shape");
        let before: Vec<(String, f64, f64, String)> = host.slides[0]
            .elements
            .iter()
            .map(|el| {
                let text = el.text.clone().unwrap_or_else(|| {
                    el.text_segments
                        .iter()
                        .map(|seg| seg.text.clone())
                        .collect()
                });
                (el.id.clone(), el.x, el.y, text)
            })
            .collect();
        assert!(before.len() >= 2, "slide has {}", before.len());
        let ids: Vec<String> = before
            .iter()
            .take(2)
            .map(|(id, _, _, _)| id.clone())
            .collect();
        let before: Vec<_> = before.into_iter().take(2).collect();
        assert!(
            before
                .iter()
                .any(|(_, _, _, text)| text.contains("Slide 1")),
            "title text missing: {before:?}"
        );
        host.group_shapes(0, &ids, "group-editor").expect("group");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save group");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        let start = xml.find("<p:grpSp>").expect("saved slide has no p:grpSp");
        let end = xml[start..]
            .find("</p:grpSp>")
            .map(|n| start + n)
            .expect("unclosed p:grpSp");
        let body = &xml[start..end];
        for (_, _, _, text) in &before {
            assert!(body.contains(text), "{text} is not inside p:grpSp");
        }
        let again = PptxHost::open(&saved).expect("reopen");
        let groups: Vec<_> = again.slides[0]
            .elements
            .iter()
            .filter(|el| el.type_ == "group")
            .collect();
        assert_eq!(groups.len(), 1, "reopen did not keep one group");
        assert_eq!(groups[0].children.len(), 2);
        for (i, child) in groups[0].children.iter().enumerate() {
            let (_, x, y, text) = &before[i];
            if !text.is_empty() {
                let child_text = child.text.clone().unwrap_or_else(|| {
                    child
                        .text_segments
                        .iter()
                        .map(|seg| seg.text.clone())
                        .collect()
                });
                assert!(
                    child_text.contains(text),
                    "child {i} lost {text}, has {child_text}"
                );
            }
            let slide_x = groups[0].x + child.x;
            let slide_y = groups[0].y + child.y;
            assert!(
                (slide_x - x).abs() < 1.0 && (slide_y - y).abs() < 1.0,
                "child {i} moved to {slide_x},{slide_y} from {x},{y}"
            );
        }

        host.ungroup_shapes(0, "group-editor", &[])
            .expect("ungroup");
        let flat = save_pptx(&host, bytes.as_slice()).expect("save ungroup");
        let flat_xml = slide_xml(&flat, "ppt/slides/slide1.xml");
        assert!(
            !flat_xml.contains("<p:grpSp>"),
            "ungroup still wrote a group"
        );
        let flat_host = PptxHost::open(&flat).expect("reopen flat");
        assert!(flat_host.slides[0]
            .elements
            .iter()
            .all(|el| el.type_ != "group"));
        for (_, x, y, text) in &before {
            let found = flat_host.slides[0].elements.iter().any(|el| {
                let label = el.text.clone().unwrap_or_else(|| {
                    el.text_segments
                        .iter()
                        .map(|seg| seg.text.clone())
                        .collect()
                });
                (text.is_empty() || label.contains(text))
                    && (el.x - x).abs() < 1.0
                    && (el.y - y).abs() < 1.0
            });
            assert!(found, "ungroup lost {text} at {x},{y}");
        }
    }

    fn part_text(bytes: &[u8], name: &str) -> String {
        let mut pkg = Package::open(bytes).expect("pkg");
        pkg.text(name).expect("text").expect("part").to_owned()
    }

    fn slide_plain(slide: &crate::edit::host::EditableSlide) -> String {
        let mut out = String::new();
        for el in &slide.elements {
            if let Some(text) = &el.text {
                out.push_str(text);
            }
        }
        out
    }

    fn slide_xml(pkg: &[u8], name: &str) -> String {
        let mut package = Package::open(pkg).expect("reopen");
        package
            .text(name)
            .expect("read")
            .unwrap_or_default()
            .to_owned()
    }

    fn emu_package() -> Vec<u8> {
        use std::io::{Cursor, Write};
        let pres = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst>
  <p:sldSz cx="9144000" cy="5143500"/>
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
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
    <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
    <p:grpSpPr/>
    <p:sp>
      <p:nvSpPr><p:cNvPr id="2" name="Box"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
      <p:spPr><a:xfrm><a:off x="10000" y="20000"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
      <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Box</a:t></a:r></a:p></p:txBody>
    </p:sp>
  </p:spTree></p:cSld>
</p:sld>"#;
        let types = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>"#;
        let parts: &[(&str, &[u8])] = &[
            ("[Content_Types].xml", types),
            ("_rels/.rels", root_rels),
            ("ppt/presentation.xml", pres),
            ("ppt/_rels/presentation.xml.rels", pres_rels),
            ("ppt/slides/slide1.xml", slide),
        ];
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
}
