// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/editor/render/bullet-toggle.ts
// and src/ui/src/pptx/render/text-advanced.ts (autoFitModePatch); changes listed in PROVENANCE.md.

//! Slide text edits: run style, paragraph align, bullets and autofit (#85).

use nitro_xml_opc::drawingml::{parse_drawing_text_body, DrawingColor, DrawingColorKind};
use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry};
use nitro_xml_opc::xml::{ns, Element};

use crate::auto_number::{
    break_auto_number_run, format_auto_number_bullet, next_auto_number, AutoNumberSequence,
};
use crate::inheritance::{attr, child_local, children_local};
use crate::level_style::parse_level_bullet_from_ppr;

use super::host::{EditableElement, PptxHost};
use super::shapes::{
    arg_bool_opt, arg_f64_opt, arg_id, arg_str, arg_u32, arg_u32_opt, host_mut, map_err,
    BulletInfoSnap, TextSegmentSnap, TextStyleSnap,
};

const DEFAULT_BULLET_CHAR: &str = "•";
const DEFAULT_AUTONUM_TYPE: &str = "arabicPeriod";

/// Authored text body used to seed the editor model (runs, bullets, autofit).
#[derive(Debug, Clone, Default)]
pub struct EditorText {
    pub segments: Vec<TextSegmentSnap>,
    pub bold: bool,
    pub italic: bool,
    pub font_size: Option<f64>,
    pub color: Option<String>,
    pub auto_fit: Option<bool>,
    pub auto_fit_mode: Option<String>,
}

/// Parse `p:txBody` into editor segments. Inherited theme sizes stay unset so a
/// shape with no authored `a:rPr` matches the pre-#85 single-segment snapshot.
pub fn parse_editor_text(tx: &Element) -> EditorText {
    let (auto_fit, auto_fit_mode) = read_autofit(tx);
    let parsed = parse_drawing_text_body(Some(tx));
    let paragraphs = children_local(tx, "p", ns::A);
    let mut out = EditorText {
        auto_fit,
        auto_fit_mode,
        ..EditorText::default()
    };
    let mut seeded = false;
    for (pi, p) in paragraphs.iter().enumerate() {
        if pi > 0 {
            out.segments.push(TextSegmentSnap {
                text: "\n".to_owned(),
                is_paragraph_break: true,
                ..TextSegmentSnap::default()
            });
        }
        let p_pr = child_local(p, "pPr", ns::A);
        let bullet = bullet_from_ppr(p_pr);
        let align = p_pr.and_then(|pp| attr(pp, "algn")).and_then(ooxml_align);
        let drawing_para = parsed.as_ref().and_then(|b| b.paragraphs.get(pi));
        let runs = drawing_para.map(|p| p.runs.as_slice()).unwrap_or(&[]);
        if runs.is_empty() {
            out.segments.push(TextSegmentSnap {
                bullet_info: bullet,
                align,
                ..TextSegmentSnap::default()
            });
            continue;
        }
        for (ri, run) in runs.iter().enumerate() {
            let is_break = run.text == "\n" && run.typeface.is_none() && run.size_pt.is_none();
            // `a:br` is parsed as text "\n" with no run properties. Keep it inside
            // the paragraph so bullet splitting does not treat it as a new `a:p`.
            let line_break = is_break;
            let mut seg = TextSegmentSnap {
                text: run.text.clone(),
                bold: run.bold.unwrap_or(false),
                italic: run.italic.unwrap_or(false),
                underline: run.underline.unwrap_or(false),
                font_size: run.size_pt.map(pt_to_px),
                color: run.color.as_ref().and_then(css_srgb),
                is_line_break: line_break,
                ..TextSegmentSnap::default()
            };
            if ri == 0 && !line_break {
                seg.bullet_info = bullet.clone();
                seg.align = align.clone();
                if !seeded {
                    out.bold = seg.bold;
                    out.italic = seg.italic;
                    out.font_size = seg.font_size;
                    out.color = seg.color.clone();
                    seeded = true;
                }
            }
            out.segments.push(seg);
        }
    }
    out
}

fn pt_to_px(pt: f64) -> f64 {
    // Match `points * (96 / 72)` so 20pt is 26.666666666666664, not 26.666666666666668.
    pt * (96.0 / 72.0)
}

fn css_srgb(color: &DrawingColor) -> Option<String> {
    if color.kind != DrawingColorKind::Srgb {
        return None;
    }
    let hex = color.value.trim();
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{hex}"))
}

fn ooxml_align(raw: &str) -> Option<String> {
    let mapped = match raw {
        "l" | "left" => "left",
        "ctr" | "center" => "center",
        "r" | "right" => "right",
        "just" | "justify" => "justify",
        "justLow" => "justLow",
        "dist" => "dist",
        "thaiDist" => "thaiDist",
        _ => return None,
    };
    Some(mapped.to_owned())
}

fn read_autofit(tx: &Element) -> (Option<bool>, Option<String>) {
    let Some(body) = child_local(tx, "bodyPr", ns::A) else {
        return (None, None);
    };
    if child_local(body, "noAutofit", ns::A).is_some() {
        return (Some(false), Some("none".to_owned()));
    }
    if child_local(body, "normAutofit", ns::A).is_some() {
        return (Some(true), Some("normal".to_owned()));
    }
    if child_local(body, "spAutoFit", ns::A).is_some() {
        return (Some(true), Some("shrink".to_owned()));
    }
    (None, None)
}

fn bullet_from_ppr(p_pr: Option<&Element>) -> Option<BulletInfoSnap> {
    let p_pr = p_pr?;
    let parsed = parse_level_bullet_from_ppr(p_pr);
    if parsed.bullet_none {
        return Some(BulletInfoSnap {
            none: true,
            ..BulletInfoSnap::default()
        });
    }
    if let Some(ch) = parsed.bullet_char {
        return Some(BulletInfoSnap {
            char: Some(ch),
            ..BulletInfoSnap::default()
        });
    }
    if let Some(kind) = parsed.bullet_auto_num_type {
        return Some(BulletInfoSnap {
            auto_num_type: Some(kind),
            auto_num_start_at: Some(parsed.bullet_auto_num_start_at),
            paragraph_index: Some(0),
            ..BulletInfoSnap::default()
        });
    }
    None
}

fn has_text_properties(el: &EditableElement) -> bool {
    matches!(el.type_.as_str(), "text" | "shape" | "connector")
}

struct Paragraph {
    segments: Vec<TextSegmentSnap>,
    terminator: Option<TextSegmentSnap>,
}

fn is_paragraph_separator(seg: &TextSegmentSnap) -> bool {
    seg.is_paragraph_break || (seg.text == "\n" && !seg.is_line_break)
}

fn split_paragraphs(segments: &[TextSegmentSnap]) -> Vec<Paragraph> {
    let mut paragraphs = vec![Paragraph {
        segments: Vec::new(),
        terminator: None,
    }];
    for seg in segments {
        if is_paragraph_separator(seg) {
            paragraphs.last_mut().expect("para").terminator = Some(seg.clone());
            paragraphs.push(Paragraph {
                segments: Vec::new(),
                terminator: None,
            });
        } else {
            paragraphs
                .last_mut()
                .expect("para")
                .segments
                .push(seg.clone());
        }
    }
    paragraphs
}

fn marker_text(info: &BulletInfoSnap) -> String {
    if let Some(kind) = &info.auto_num_type {
        let n = (info.auto_num_start_at.unwrap_or(1) + info.paragraph_index.unwrap_or(0)).max(1);
        // `format_auto_number_bullet` appends a space; the TS marker is the bare token (`"1."`).
        let with_space = format_auto_number_bullet(kind, n);
        return with_space.trim_end().to_owned();
    }
    format!("{} ", info.char.as_deref().unwrap_or(DEFAULT_BULLET_CHAR))
}

fn is_bullet_marker(seg: &TextSegmentSnap) -> bool {
    let Some(info) = &seg.bullet_info else {
        return false;
    };
    if info.none || seg.is_line_break {
        return false;
    }
    if info.char.is_none() && info.auto_num_type.is_none() {
        return false;
    }
    if info.auto_num_type.is_some() {
        if info.paragraph_index.is_none() {
            return false;
        }
        let marker = marker_text(info);
        return seg.text == marker || seg.text == format!("{marker} ");
    }
    seg.text == marker_text(info)
}

fn bullet_info_for_kind(kind: &str, ordinal: u32) -> BulletInfoSnap {
    match kind {
        "none" => BulletInfoSnap {
            none: true,
            ..BulletInfoSnap::default()
        },
        "numbered" => BulletInfoSnap {
            auto_num_type: Some(DEFAULT_AUTONUM_TYPE.to_owned()),
            auto_num_start_at: Some(1),
            paragraph_index: Some(ordinal),
            ..BulletInfoSnap::default()
        },
        _ => BulletInfoSnap {
            char: Some(DEFAULT_BULLET_CHAR.to_owned()),
            ..BulletInfoSnap::default()
        },
    }
}

fn info_for_paragraph(
    previous: Option<&BulletInfoSnap>,
    kind: &str,
    ordinal: Option<u32>,
) -> BulletInfoSnap {
    if kind == "none" {
        let mut info = previous.cloned().unwrap_or_default();
        info.none = true;
        return info;
    }
    let same_kind = previous.is_some_and(|prev| {
        if kind == "numbered" {
            prev.auto_num_type.is_some()
        } else {
            prev.char.is_some() && prev.auto_num_type.is_none()
        }
    });
    let mut info = if same_kind {
        previous.cloned().unwrap_or_default()
    } else {
        bullet_info_for_kind(kind, ordinal.unwrap_or(0))
    };
    info.none = false;
    if kind == "numbered" {
        if let Some(ordinal) = ordinal {
            info.paragraph_index = Some(ordinal);
        }
    }
    info
}

fn paragraph_bullet_kind(paragraph: &[TextSegmentSnap]) -> &'static str {
    let Some(first) = paragraph.first() else {
        return "none";
    };
    let Some(info) = &first.bullet_info else {
        return "none";
    };
    if info.none {
        return "none";
    }
    if info.auto_num_type.is_some() && info.paragraph_index.is_some() {
        return "numbered";
    }
    if info.char.is_some() || info.auto_num_type.is_some() {
        if info.auto_num_type.is_some() && info.paragraph_index.is_none() {
            return "none";
        }
        return "bullet";
    }
    "none"
}

fn toggle_paragraph(
    paragraph: &[TextSegmentSnap],
    kind: &str,
    ordinal: Option<u32>,
) -> Vec<TextSegmentSnap> {
    let Some(source) = paragraph.first() else {
        return Vec::new();
    };
    let content: Vec<TextSegmentSnap> = if is_bullet_marker(source) {
        paragraph[1..].to_vec()
    } else {
        paragraph.to_vec()
    };
    let first = content
        .first()
        .cloned()
        .unwrap_or_else(|| source.copy_run_style());
    let align = source.align.clone().or_else(|| first.align.clone());
    let info = info_for_paragraph(source.bullet_info.as_ref(), kind, ordinal);
    let mut body = first;
    body.align = align.clone();
    body.bullet_info = None;
    let rest: Vec<TextSegmentSnap> = content
        .iter()
        .skip(1)
        .map(|seg| {
            let mut run = seg.clone();
            run.bullet_info = None;
            run
        })
        .collect();
    if kind == "none" {
        body.bullet_info = Some(info);
        let mut out = vec![body];
        out.extend(rest);
        return out;
    }
    let marker_style = if is_bullet_marker(source)
        && paragraph_bullet_kind(std::slice::from_ref(source)) == kind
    {
        source.copy_run_style()
    } else {
        body.copy_run_style()
    };
    let marker = TextSegmentSnap {
        text: marker_text(&info),
        bold: marker_style.bold,
        italic: marker_style.italic,
        underline: marker_style.underline,
        font_size: marker_style.font_size,
        color: marker_style.color,
        align,
        bullet_info: Some(info),
        ..TextSegmentSnap::default()
    };
    let mut out = vec![marker, body];
    out.extend(rest);
    out
}

/// `setElementBullets`. An element without text properties is left unchanged.
pub fn set_element_bullets(
    el: &mut EditableElement,
    kind: &str,
    start: Option<u32>,
    end: Option<u32>,
) -> Result<(), String> {
    if !matches!(kind, "bullet" | "numbered" | "none") {
        return Err(format!("setShapeBullets kind {kind}"));
    }
    if !has_text_properties(el) {
        return Ok(());
    }
    if let (Some(start), Some(end)) = (start, end) {
        if end < start {
            return Ok(());
        }
    }
    let range = match (start, end) {
        (Some(s), Some(e)) => Some((s, e)),
        (None, None) => None,
        _ => return Err("setShapeBullets needs both startParagraph and endParagraph".into()),
    };
    let paragraphs = split_paragraphs(&el.text_segments);
    if let Some((start, _)) = range {
        if start as usize >= paragraphs.len() {
            return Ok(());
        }
    }
    let mut sequence = AutoNumberSequence::default();
    let mut next = Vec::new();
    for (index, paragraph) in paragraphs.iter().enumerate() {
        let selected =
            range.is_none_or(|(start, end)| index >= start as usize && index <= end as usize);
        let mut segments = paragraph.segments.clone();
        if selected {
            if segments.is_empty() {
                segments.push(TextSegmentSnap::default());
            }
            segments = toggle_paragraph(&segments, kind, None);
        }
        let first = segments.first().or(paragraph.terminator.as_ref());
        let info = first.and_then(|s| s.bullet_info.clone());
        if info.as_ref().is_some_and(|i| i.auto_num_type.is_some())
            && paragraph_bullet_kind(&segments) == "numbered"
        {
            let start_at = info.as_ref().and_then(|i| i.auto_num_start_at).unwrap_or(1);
            let kind_name = info
                .as_ref()
                .and_then(|i| i.auto_num_type.clone())
                .unwrap_or_else(|| DEFAULT_AUTONUM_TYPE.to_owned());
            let ordinal = next_auto_number(&mut sequence, 0, &kind_name, start_at) - start_at;
            let current = info.as_ref().and_then(|i| i.paragraph_index);
            if current != Some(ordinal) {
                segments = toggle_paragraph(&segments, "numbered", Some(ordinal));
            }
        } else {
            break_auto_number_run(&mut sequence, 0);
        }
        next.extend(segments);
        if let Some(term) = &paragraph.terminator {
            next.push(term.clone());
        }
    }
    el.text_segments = next;
    Ok(())
}

fn content_runs(segments: &[TextSegmentSnap]) -> Vec<Vec<usize>> {
    let mut paras: Vec<Vec<usize>> = vec![Vec::new()];
    for (i, seg) in segments.iter().enumerate() {
        if is_paragraph_separator(seg) {
            paras.push(Vec::new());
        } else {
            paras.last_mut().expect("para").push(i);
        }
    }
    paras
}

/// Optional run properties for `setShapeRunStyle`. Unset fields stay as they are.
pub struct RunStylePatch<'a> {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub font_size: Option<f64>,
    pub color: Option<&'a str>,
}

pub fn set_run_style(
    el: &mut EditableElement,
    paragraph_index: u32,
    run_index: u32,
    patch: &RunStylePatch<'_>,
) -> Result<(), String> {
    if !has_text_properties(el) {
        return Err("setShapeRunStyle: element has no text".into());
    }
    if el.text_segments.is_empty() {
        el.text_segments.push(TextSegmentSnap::default());
    }
    let paras = content_runs(&el.text_segments);
    let para = paras
        .get(paragraph_index as usize)
        .ok_or_else(|| format!("setShapeRunStyle paragraph {paragraph_index} missing"))?;
    let seg_index = *para
        .get(run_index as usize)
        .ok_or_else(|| format!("setShapeRunStyle run {run_index} missing"))?;
    let seg = &mut el.text_segments[seg_index];
    if let Some(bold) = patch.bold {
        seg.bold = bold;
    }
    if let Some(italic) = patch.italic {
        seg.italic = italic;
    }
    if let Some(underline) = patch.underline {
        seg.underline = underline;
    }
    if let Some(font_size) = patch.font_size {
        seg.font_size = Some(font_size);
    }
    if let Some(color) = patch.color {
        seg.color = Some(color.to_owned());
    }
    // `reconcileDecorationFlags`: the body flag is true only when every content run is on.
    if let Some(bold) = patch.bold {
        reconcile_body_flag(el, true, seg_index, bold);
    }
    if let Some(italic) = patch.italic {
        reconcile_body_flag(el, false, seg_index, italic);
    }
    Ok(())
}

fn is_content_segment(seg: &TextSegmentSnap) -> bool {
    !seg.is_paragraph_break && seg.text != "\n" && !seg.text.is_empty() && !is_bullet_marker(seg)
}

/// Body decoration follows the runs. A flag we just cleared stays off; other
/// runs that were only inheriting a previously-on body flag are written on.
fn reconcile_body_flag(el: &mut EditableElement, bold: bool, touched: usize, written: bool) {
    let body = el
        .text_style
        .as_ref()
        .map(|s| if bold { s.bold } else { s.italic })
        .unwrap_or(false);
    let content: Vec<usize> = el
        .text_segments
        .iter()
        .enumerate()
        .filter(|(_, seg)| is_content_segment(seg))
        .map(|(i, _)| i)
        .collect();
    if content.is_empty() {
        return;
    }
    let all_on = content.iter().all(|&i| {
        let seg = &el.text_segments[i];
        if bold {
            seg.bold
        } else {
            seg.italic
        }
    });
    let style = el.text_style.get_or_insert_with(TextStyleSnap::default);
    if all_on {
        if bold {
            style.bold = true;
        } else {
            style.italic = true;
        }
        return;
    }
    if !body {
        return;
    }
    if bold {
        style.bold = false;
    } else {
        style.italic = false;
    }
    for i in content {
        if i == touched && !written {
            continue;
        }
        let seg = &mut el.text_segments[i];
        let on = if bold { seg.bold } else { seg.italic };
        if !on {
            if bold {
                seg.bold = true;
            } else {
                seg.italic = true;
            }
        }
    }
}

pub fn set_paragraph_align(
    el: &mut EditableElement,
    paragraph_index: u32,
    align: &str,
) -> Result<(), String> {
    if !matches!(
        align,
        "left" | "center" | "right" | "justify" | "justLow" | "dist" | "thaiDist"
    ) {
        return Err(format!("setShapeParagraphAlign align {align}"));
    }
    if !has_text_properties(el) {
        return Err("setShapeParagraphAlign: element has no text".into());
    }
    if el.text_segments.is_empty() {
        el.text_segments.push(TextSegmentSnap::default());
    }
    let paras = content_runs(&el.text_segments);
    let para = paras
        .get(paragraph_index as usize)
        .ok_or_else(|| format!("setShapeParagraphAlign paragraph {paragraph_index} missing"))?;
    let seg_index = *para
        .first()
        .ok_or_else(|| format!("setShapeParagraphAlign paragraph {paragraph_index} missing"))?;
    el.text_segments[seg_index].align = Some(align.to_owned());
    Ok(())
}

/// `autoFitModePatch`: `autoFit` is true for every mode except `none`.
pub fn set_autofit(el: &mut EditableElement, mode: &str) -> Result<(), String> {
    if !matches!(mode, "none" | "normal" | "shrink") {
        return Err(format!("setShapeAutofit mode {mode}"));
    }
    let style = el.text_style.get_or_insert_with(TextStyleSnap::default);
    style.auto_fit_mode = Some(mode.to_owned());
    style.auto_fit = Some(mode != "none");
    Ok(())
}

fn change(kind: ChangeKind, label: &str) -> ChangeSet {
    ChangeSet::new(kind, label)
}

pub fn register_text_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("setShapeRunStyle", "setShapeRunStyle").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let paragraph = arg_u32(args, "paragraphIndex")?;
            let run = arg_u32(args, "runIndex")?;
            let color = args
                .get("color")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            h.set_run_style(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                paragraph,
                run,
                &RunStylePatch {
                    bold: arg_bool_opt(args, "bold"),
                    italic: arg_bool_opt(args, "italic"),
                    underline: arg_bool_opt(args, "underline"),
                    font_size: arg_f64_opt(args, "fontSize"),
                    color: color.as_deref(),
                },
            )
            .map_err(map_err)?;
            Ok(change(ChangeKind::Format, "setShapeRunStyle"))
        }),
    );
    registry.register(
        Command::stub("setShapeParagraphAlign", "setShapeParagraphAlign").with_runner(
            |host, args| {
                let h = host_mut(host)?;
                let slide = arg_u32(args, "slideIndex")?;
                let paragraph = arg_u32(args, "paragraphIndex")?;
                let align = arg_str(args, "align")?.to_owned();
                h.set_paragraph_align(
                    slide,
                    arg_id(args),
                    arg_u32_opt(args, "elementIndex"),
                    paragraph,
                    &align,
                )
                .map_err(map_err)?;
                Ok(change(ChangeKind::Format, "setShapeParagraphAlign"))
            },
        ),
    );
    registry.register(
        Command::stub("setShapeBullets", "setShapeBullets").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let kind = arg_str(args, "kind")?.to_owned();
            h.set_bullets(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                &kind,
                arg_u32_opt(args, "startParagraph"),
                arg_u32_opt(args, "endParagraph"),
            )
            .map_err(map_err)?;
            Ok(change(ChangeKind::Format, "setShapeBullets"))
        }),
    );
    registry.register(
        Command::stub("setShapeAutofit", "setShapeAutofit").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let mode = arg_str(args, "mode")?.to_owned();
            h.set_autofit(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                &mode,
            )
            .map_err(map_err)?;
            Ok(change(ChangeKind::Format, "setShapeAutofit"))
        }),
    );
}

impl PptxHost {
    pub fn set_run_style(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        paragraph_index: u32,
        run_index: u32,
        patch: &RunStylePatch<'_>,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        set_run_style(&mut slide.elements[idx], paragraph_index, run_index, patch)
    }

    pub fn set_paragraph_align(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        paragraph_index: u32,
        align: &str,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        set_paragraph_align(&mut slide.elements[idx], paragraph_index, align)
    }

    pub fn set_bullets(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        kind: &str,
        start: Option<u32>,
        end: Option<u32>,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        set_element_bullets(&mut slide.elements[idx], kind, start, end)
    }

    pub fn set_autofit(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        mode: &str,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        set_autofit(&mut slide.elements[idx], mode)
    }
}
