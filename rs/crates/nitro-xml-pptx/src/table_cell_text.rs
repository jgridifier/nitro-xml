// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/builders/table-cell-runs.ts,
// table-cell-paragraphs.ts, table-cell-text-style-helpers.ts (applyCellTextFormat) and
// src/ui/src/pptx/render/table-cell-paragraphs.ts; changes in PROVENANCE.md.
//! Rich table cell text: runs, paragraph layout and the cell-level text style taken from the first run.

use nitro_xml_opc::xml::{ns, Element};

use crate::inheritance::{attr, child_local, children_local};

const PX_PER_POINT: f64 = 96.0 / 72.0;
const EMU_PER_PX: f64 = 9525.0;
pub(crate) const DEFAULT_LINE_HEIGHT: f64 = 1.2;

#[derive(Debug, Clone, Default)]
pub(crate) struct CellRun {
    pub text: String,
    pub is_line_break: bool,
    pub is_paragraph_break: bool,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub font_size: Option<f64>,
    pub color: Option<String>,
    pub font_family: Option<String>,
    pub baseline: i64,
    pub character_spacing: i64,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CellParagraph {
    pub align: Option<String>,
    pub rtl: Option<bool>,
    pub margin_left: Option<f64>,
    pub indent: Option<f64>,
    pub line_spacing: Option<f64>,
    pub line_spacing_exact_pt: Option<f64>,
    pub space_before: Option<f64>,
    pub space_after: Option<f64>,
    pub end_para_font_size: Option<f64>,
}

impl CellParagraph {
    fn is_empty(&self) -> bool {
        *self == CellParagraph::default()
    }
}

/// The text-derived part of upstream `PptxTableCellStyle` (`applyCellTextFormat` + anchor).
#[derive(Debug, Clone, Default)]
pub(crate) struct CellTextStyle {
    pub align: Option<&'static str>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font_size: Option<f64>,
    pub color: Option<String>,
    pub font_family: Option<String>,
    pub v_align: Option<&'static str>,
    /// `tcMar` as padding in px (left, top, right, bottom), net of half the authored border.
    pub padding: [Option<f64>; 4],
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CellRich {
    pub runs: Option<Vec<CellRun>>,
    pub paragraphs: Option<Vec<CellParagraph>>,
    pub style: CellTextStyle,
}

fn int_attr(el: &Element, name: &str) -> Option<i64> {
    attr(el, name).and_then(|v| v.trim().parse::<i64>().ok())
}

fn typeface(r_pr: &Element) -> Option<String> {
    ["latin", "ea", "cs"].iter().find_map(|k| {
        child_local(r_pr, k, ns::A)
            .and_then(|n| attr(n, "typeface"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
}

fn size_pt(el: Option<&Element>) -> Option<f64> {
    el.and_then(|e| int_attr(e, "sz"))
        .filter(|s| *s > 0)
        .map(|s| s as f64 / 100.0)
}

fn flag(r_pr: &Element, name: &str) -> bool {
    attr(r_pr, name) == Some("1")
}

fn apply_run_props(run: &mut CellRun, r_pr: &Element, color: &dyn Fn(&Element) -> Option<String>) {
    run.bold = flag(r_pr, "b");
    run.italic = flag(r_pr, "i");
    run.underline = attr(r_pr, "u").is_some_and(|u| u != "none");
    run.strikethrough = attr(r_pr, "strike").is_some_and(|s| !s.is_empty() && s != "noStrike");
    run.font_size = size_pt(Some(r_pr));
    if let Some(fill) = child_local(r_pr, "solidFill", ns::A) {
        run.color = color(fill);
    }
    run.font_family = typeface(r_pr);
    run.baseline = int_attr(r_pr, "baseline").unwrap_or(0);
    run.character_spacing = int_attr(r_pr, "spc").unwrap_or(0);
}

fn lang(r_pr: Option<&Element>) -> Option<String> {
    r_pr.and_then(|r| attr(r, "lang"))
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
}

/// Document-ordered `a:r` / `a:br` / `a:fld` children of a paragraph.
fn content_entries(p: &Element) -> Vec<&Element> {
    p.children
        .iter()
        .filter_map(|n| match n {
            nitro_xml_opc::xml::Node::Element(e)
                if matches!(e.local.as_str(), "r" | "br" | "fld")
                    && e.ns.as_deref() == Some(ns::A) =>
            {
                Some(e)
            }
            _ => None,
        })
        .collect()
}

fn run_text(node: &Element) -> String {
    child_local(node, "t", ns::A)
        .map(|t| t.text_content())
        .unwrap_or_default()
}

fn extract_runs(
    body: &Element,
    color: &dyn Fn(&Element) -> Option<String>,
) -> Option<Vec<CellRun>> {
    let paragraphs = children_local(body, "p", ns::A);
    if paragraphs.is_empty() {
        return None;
    }
    let cell_lang = child_local(body, "lstStyle", ns::A)
        .and_then(|l| child_local(l, "lvl1pPr", ns::A))
        .and_then(|l| child_local(l, "defRPr", ns::A))
        .and_then(|d| lang(Some(d)));
    let mut runs = Vec::new();
    let mut content = 0;
    for (index, p) in paragraphs.iter().enumerate() {
        if index > 0 {
            runs.push(CellRun {
                is_paragraph_break: true,
                ..Default::default()
            });
        }
        for node in content_entries(p) {
            if node.local == "br" {
                runs.push(CellRun {
                    is_line_break: true,
                    ..Default::default()
                });
                content += 1;
                continue;
            }
            let mut run = CellRun {
                text: run_text(node),
                ..Default::default()
            };
            let r_pr = child_local(node, "rPr", ns::A);
            if let Some(r_pr) = r_pr {
                apply_run_props(&mut run, r_pr, color);
            }
            run.language = lang(r_pr).or_else(|| cell_lang.clone());
            runs.push(run);
            content += 1;
        }
    }
    (content > 0).then_some(runs)
}

fn spacing_px(node: Option<&Element>, basis_px: Option<f64>) -> Option<f64> {
    let node = node?;
    if let Some(raw) = child_local(node, "spcPts", ns::A).and_then(|n| int_attr(n, "val")) {
        return Some(raw as f64 / 100.0 * PX_PER_POINT);
    }
    let pct = child_local(node, "spcPct", ns::A)
        .and_then(|n| int_attr(n, "val"))
        .map(|v| v as f64 / 100_000.0)?;
    basis_px.filter(|b| *b > 0.0).map(|b| pct * b * 1.2)
}

fn read_paragraph(p: &Element, default_pt: Option<f64>) -> CellParagraph {
    let mut out = CellParagraph::default();
    let first_run = child_local(p, "r", ns::A);
    let is_empty = first_run.is_none()
        && child_local(p, "fld", ns::A).is_none()
        && child_local(p, "br", ns::A).is_none();
    let end_pt = size_pt(child_local(p, "endParaRPr", ns::A));
    if is_empty {
        out.end_para_font_size = end_pt;
    }
    let Some(p_pr) = child_local(p, "pPr", ns::A) else {
        return out;
    };
    out.align = attr(p_pr, "algn").and_then(|a| match a {
        "l" => Some("left".to_string()),
        "ctr" => Some("center".to_string()),
        "r" => Some("right".to_string()),
        "just" | "justify" => Some("justify".to_string()),
        "justLow" | "dist" | "thaiDist" => Some(a.to_string()),
        _ => None,
    });
    out.rtl = match attr(p_pr, "rtl") {
        Some("1") | Some("true") => Some(true),
        Some("0") | Some("false") => Some(false),
        _ => None,
    };
    out.margin_left = int_attr(p_pr, "marL").map(|v| v as f64 / EMU_PER_PX);
    out.indent = int_attr(p_pr, "indent").map(|v| v as f64 / EMU_PER_PX);
    if let Some(ln) = child_local(p_pr, "lnSpc", ns::A) {
        let pct = child_local(ln, "spcPct", ns::A)
            .and_then(|n| int_attr(n, "val"))
            .map(|v| (v as f64 / 100_000.0).clamp(0.1, 5.0));
        let exact = child_local(ln, "spcPts", ns::A)
            .and_then(|n| int_attr(n, "val"))
            .filter(|v| *v > 0)
            .map(|v| v as f64 / 100.0);
        if pct.is_some() {
            out.line_spacing = pct;
        } else {
            out.line_spacing_exact_pt = exact;
        }
    }
    let basis_pt = first_run
        .map(|r| size_pt(child_local(r, "rPr", ns::A)))
        .unwrap_or(end_pt)
        .or(default_pt);
    let basis_px = basis_pt.map(|v| v * PX_PER_POINT);
    out.space_before = spacing_px(child_local(p_pr, "spcBef", ns::A), basis_px);
    out.space_after = spacing_px(child_local(p_pr, "spcAft", ns::A), basis_px);
    out
}

fn text_style(
    cell: &Element,
    body: Option<&Element>,
    color: &dyn Fn(&Element) -> Option<String>,
) -> CellTextStyle {
    let mut style = CellTextStyle::default();
    if let Some(tc_pr) = child_local(cell, "tcPr", ns::A) {
        // cellPaddingCss: a margin is measured from the grid line, so half the border comes off.
        let border = |name: &str| {
            child_local(tc_pr, name, ns::A)
                .and_then(|n| int_attr(n, "w"))
                .filter(|w| *w > 0)
                .map(|w| w as f64 / EMU_PER_PX)
        };
        let mar = child_local(tc_pr, "tcMar", ns::A);
        // `a:tcMar` children win; direct `marL`/`marR`/`marT`/`marB` attributes are the fallback.
        let margin = |child: &str, direct: &str, line: &str| {
            let emu = mar
                .and_then(|m| child_local(m, child, ns::A))
                .and_then(|n| int_attr(n, "w"))
                .filter(|w| *w >= 0)
                .or_else(|| int_attr(tc_pr, direct).filter(|w| *w >= 0))?;
            Some((emu as f64 / EMU_PER_PX - border(line).unwrap_or(0.0) / 2.0).max(0.0))
        };
        style.padding = [
            margin("marL", "marL", "lnL"),
            margin("marT", "marT", "lnT"),
            margin("marR", "marR", "lnR"),
            margin("marB", "marB", "lnB"),
        ];
    }
    let Some(first) = body.and_then(|b| children_local(b, "p", ns::A).into_iter().next()) else {
        return style;
    };
    style.align = child_local(first, "pPr", ns::A)
        .and_then(|p| attr(p, "algn"))
        .and_then(|a| match a {
            "ctr" => Some("center"),
            "r" => Some("right"),
            "just" => Some("justify"),
            _ => None,
        });
    let r_pr = child_local(first, "r", ns::A).and_then(|r| child_local(r, "rPr", ns::A));
    match r_pr {
        None => {
            let has_content = body.is_some_and(|b| {
                children_local(b, "p", ns::A).iter().any(|p| {
                    ["r", "fld", "br"]
                        .iter()
                        .any(|t| child_local(p, t, ns::A).is_some())
                })
            });
            if !has_content {
                style.font_size = size_pt(child_local(first, "endParaRPr", ns::A));
            }
        }
        Some(r_pr) => {
            style.bold = flag(r_pr, "b");
            style.italic = flag(r_pr, "i");
            style.underline = attr(r_pr, "u").is_some_and(|u| !u.is_empty() && u != "none");
            style.font_size = attr(r_pr, "sz")
                .and_then(|s| s.parse::<i64>().ok())
                .map(|s| s as f64 / 100.0);
            if let Some(fill) = child_local(r_pr, "solidFill", ns::A) {
                style.color = color(fill);
            }
            style.font_family = typeface(r_pr);
        }
    }
    style
}

pub(crate) fn parse_cell_rich(
    cell: &Element,
    default_pt: Option<f64>,
    color: &dyn Fn(&Element) -> Option<String>,
) -> CellRich {
    let body = child_local(cell, "txBody", ns::A);
    let runs = body.and_then(|b| extract_runs(b, color));
    let paragraphs = runs.as_ref().and_then(|_| {
        let read: Vec<CellParagraph> = children_local(body?, "p", ns::A)
            .iter()
            .map(|p| read_paragraph(p, default_pt))
            .collect();
        read.iter().any(|p| !p.is_empty()).then_some(read)
    });
    CellRich {
        runs,
        paragraphs,
        style: text_style(cell, body, color),
    }
}

/// One paragraph `<div>` of a cell (`CellParagraphBlock`).
pub(crate) struct CellBlock {
    pub css: Vec<(&'static str, String)>,
    pub runs: Vec<CellRun>,
    /// Exact line spacing: runs must not grow the line (`line-height: 0`).
    pub exact_spacing: bool,
}

struct Layout {
    css: Vec<(&'static str, String)>,
    run_line_height: Option<f64>,
}

fn num(v: f64) -> String {
    format!("{v}")
}

fn paragraph_layout(
    p: Option<&CellParagraph>,
    follows_cell: bool,
    is_first: bool,
    is_last: bool,
) -> Layout {
    let mut css: Vec<(&'static str, String)> = Vec::new();
    let rtl = p.and_then(|p| p.rtl) == Some(true);
    if !follows_cell {
        let align = match p.and_then(|p| p.align.as_deref()) {
            Some("justLow") | Some("dist") | Some("thaiDist") => Some("justify".to_string()),
            Some(a) => Some(a.to_string()),
            None if rtl => Some("right".to_string()),
            None => None,
        };
        css.push(("text-align", align.unwrap_or_else(|| "start".into())));
        if p.and_then(|p| p.align.as_deref()) == Some("dist") {
            css.push(("text-align-last", "justify".into()));
        }
    }
    if let Some(rtl) = p.and_then(|p| p.rtl) {
        css.push(("direction", if rtl { "rtl" } else { "ltr" }.into()));
        css.push(("unicode-bidi", "embed".into()));
    }
    let margin = p.and_then(|p| p.margin_left).filter(|v| *v != 0.0);
    let indent = p.and_then(|p| p.indent).filter(|v| *v != 0.0);
    if let Some(m) = margin {
        css.push(("margin-inline-start", format!("{}px", num(m))));
    }
    if let Some(i) = indent {
        if i > 0.0 || margin.is_some() {
            css.push(("text-indent", format!("{}px", num(i))));
        }
    }
    let before = p
        .and_then(|p| p.space_before)
        .filter(|v| *v > 0.0 && !is_first)
        .unwrap_or(0.0);
    let after = p
        .and_then(|p| p.space_after)
        .filter(|v| *v > 0.0 && !is_last)
        .unwrap_or(0.0);
    if before + after > 0.0 {
        css.push(("margin-block-end", format!("{}px", num(before + after))));
    }
    let mut run_line_height = None;
    if let Some(exact) = p.and_then(|p| p.line_spacing_exact_pt).filter(|v| *v > 0.0) {
        css.push(("line-height", format!("{}px", num(exact * PX_PER_POINT))));
        run_line_height = Some(0.0);
    } else if let Some(m) = p.and_then(|p| p.line_spacing).filter(|v| *v > 0.0) {
        css.push(("line-height", num(m * DEFAULT_LINE_HEIGHT)));
    }
    Layout {
        css,
        run_line_height,
    }
}

fn paragraph_align(p: Option<&CellParagraph>) -> &str {
    p.and_then(|p| p.align.as_deref()).unwrap_or("left")
}

fn strut_font_size(runs: &[CellRun]) -> Option<f64> {
    let mut smallest: Option<f64> = None;
    for run in runs {
        if run.text.is_empty() {
            continue;
        }
        match run.font_size {
            Some(s) if s > 0.0 => smallest = Some(smallest.map_or(s, |m: f64| m.min(s))),
            _ => return None,
        }
    }
    smallest
}

/// `cellParagraphBlocks`: `None` when the cell renders as plain runs.
pub(crate) fn paragraph_blocks(rich: &CellRich, cell_text: &str) -> Option<Vec<CellBlock>> {
    let paragraphs = rich.paragraphs.as_ref().filter(|p| !p.is_empty())?;
    let runs: Vec<CellRun> = match rich.runs.as_ref().filter(|r| !r.is_empty()) {
        Some(r) => r.clone(),
        None => vec![CellRun {
            text: cell_text.to_string(),
            ..Default::default()
        }],
    };
    let mut groups: Vec<Vec<CellRun>> = vec![Vec::new()];
    for run in runs {
        if run.is_paragraph_break {
            groups.push(Vec::new());
        } else if let Some(last) = groups.last_mut() {
            last.push(run);
        }
    }
    let first = paragraphs.first();
    let follows: Vec<bool> = (0..groups.len())
        .map(|i| {
            let p = paragraphs.get(i);
            paragraph_align(p) == paragraph_align(first)
                && (p.and_then(|p| p.rtl) != Some(true) || rich.style.align.is_some())
        })
        .collect();
    let count = groups.len();
    let layouts: Vec<Layout> = (0..count)
        .map(|i| paragraph_layout(paragraphs.get(i), follows[i], i == 0, i == count - 1))
        .collect();
    let has = |l: &Layout, key: &str| l.css.iter().any(|(k, _)| *k == key);
    let needs = (0..count).any(|i| {
        !follows[i]
            || paragraphs.get(i).and_then(|p| p.rtl) == Some(true)
            || has(&layouts[i], "margin-inline-start")
            || has(&layouts[i], "text-indent")
            || has(&layouts[i], "margin-block-end")
            || layouts[i]
                .css
                .iter()
                .any(|(k, v)| *k == "line-height" && *v != num(DEFAULT_LINE_HEIGHT))
    });
    if !needs {
        return None;
    }
    Some(
        groups
            .into_iter()
            .zip(layouts)
            .enumerate()
            .map(|(index, (group, layout))| {
                let mut css = layout.css;
                let has_content = group.iter().any(|r| !r.text.is_empty() || r.is_line_break);
                if !has_content {
                    if let Some(end) = paragraphs.get(index).and_then(|p| p.end_para_font_size) {
                        css.push(("font-size", format!("{}pt", num(end))));
                    }
                    return CellBlock {
                        css,
                        runs: vec![CellRun {
                            is_line_break: true,
                            ..Default::default()
                        }],
                        exact_spacing: false,
                    };
                }
                if let Some(s) = strut_font_size(&group) {
                    css.push(("font-size", format!("{}pt", num(s))));
                }
                CellBlock {
                    css,
                    runs: group,
                    exact_spacing: layout.run_line_height.is_some(),
                }
            })
            .collect(),
    )
}

/// `cellRunStyle`: the CSS of one run's `<span>` (`line-height: 0` for exact paragraph spacing).
pub(crate) fn run_css(run: &CellRun, exact_spacing: bool) -> Vec<(&'static str, String)> {
    let mut css: Vec<(&'static str, String)> = Vec::new();
    if let Some(f) = &run.font_family {
        css.push(("font-family", f.clone()));
    }
    if exact_spacing {
        css.push(("line-height", "0".into()));
    }
    let shift = match run.baseline {
        0 => None,
        b if b > 0 => Some("super"),
        _ => Some("sub"),
    };
    if let Some(size) = run.font_size {
        let scale = if shift.is_some() { 0.65 } else { 1.0 };
        css.push(("font-size", format!("{}pt", num(size * scale))));
    } else if shift.is_some() {
        css.push(("font-size", "65%".into()));
    }
    if let Some(s) = shift {
        css.push(("vertical-align", s.into()));
    }
    if let Some(c) = &run.color {
        css.push(("color", c.clone()));
    }
    if run.bold {
        css.push(("font-weight", "bold".into()));
    }
    if run.italic {
        css.push(("font-style", "italic".into()));
    }
    let mut deco = Vec::new();
    if run.underline {
        deco.push("underline");
    }
    if run.strikethrough {
        deco.push("line-through");
    }
    if !deco.is_empty() {
        css.push(("text-decoration", deco.join(" ")));
    }
    if run.character_spacing != 0 {
        css.push((
            "letter-spacing",
            format!(
                "{}px",
                num(run.character_spacing as f64 / 100.0 * PX_PER_POINT)
            ),
        ));
    }
    if run.language.as_deref().is_some_and(|l| {
        l.len() >= 2
            && l[..2].eq_ignore_ascii_case("ko")
            && (l.len() == 2 || l.as_bytes()[2] == b'-')
    }) {
        css.push(("word-break", "keep-all".into()));
    }
    css
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::xml::parse_xml_str;

    fn cell(xml: &str) -> CellRich {
        let doc = parse_xml_str(xml).expect("xml");
        parse_cell_rich(&doc.root, None, &|_| None)
    }

    // Catches: table cell text flattened to one default-size string, dropping run size/bold and
    // paragraph align so a 10pt right/centre-aligned cell paints as 18pt left text.
    #[test]
    fn cell_keeps_run_size_and_paragraph_alignment_blocks() {
        let rich = cell(
            r#"<a:tc xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:txBody><a:bodyPr/>
<a:p><a:pPr algn="r"/><a:r><a:rPr sz="1000" b="1"/><a:t>A</a:t></a:r></a:p>
<a:p><a:pPr algn="ctr"/><a:r><a:rPr sz="1000"/><a:t>B</a:t></a:r></a:p></a:txBody></a:tc>"#,
        );
        assert_eq!(rich.style.font_size, Some(10.0));
        assert!(rich.style.bold);
        let blocks = paragraph_blocks(&rich, "A\nB").expect("alignment differs per paragraph");
        assert_eq!(blocks.len(), 2);
        // The first paragraph follows the cell's own alignment; the second differs and sets its own.
        assert_eq!(rich.style.align, Some("right"));
        assert!(blocks[1]
            .css
            .contains(&("text-align", "center".to_string())));
        // Negative control: uniform left-aligned single-spaced paragraphs stay plain runs.
        let plain = cell(
            r#"<a:tc xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:txBody><a:bodyPr/>
<a:p><a:r><a:rPr sz="1000"/><a:t>A</a:t></a:r></a:p></a:txBody></a:tc>"#,
        );
        assert!(paragraph_blocks(&plain, "A").is_none());
    }
}
