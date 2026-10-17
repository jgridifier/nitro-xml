// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/ole.ts; changes in PROVENANCE.md.
//! P15 OLE previews — poster image when a blip / VML / metafile preview resolved, else the
//! typed placeholder box (icon + display name), as upstream `buildPlaceholder`.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub, set_style};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    // Loader `scheduleMetafileAttr` rasterizes `nitro-metafile:` img src (bug64693 WMF).
    if el.image_src.is_some() {
        paint_preview(ctx, el, parent);
    } else {
        paint_placeholder(ctx, el, parent);
    }
}

/// Upstream `ResolvedOleType`: `detectOleObjectType` progId/clsid, then `resolveOleType` progId substrings.
#[derive(Clone, Copy, PartialEq, Debug)]
enum OleType {
    Excel,
    Word,
    PowerPoint,
    Pdf,
    Visio,
    MathType,
    Unknown,
}

impl OleType {
    fn color(self) -> &'static str {
        match self {
            Self::Excel => "#217346",
            Self::Word => "#2B579A",
            Self::PowerPoint => "#D24726",
            Self::Pdf => "#D4272E",
            Self::Visio => "#3955A3",
            Self::MathType => "#7B2D8E",
            Self::Unknown => "#666666",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Excel => "Excel Spreadsheet",
            Self::Word => "Word Document",
            Self::PowerPoint => "PowerPoint Presentation",
            Self::Pdf => "PDF Document",
            Self::Visio => "Visio Diagram",
            Self::MathType => "Math Equation",
            Self::Unknown => "Embedded Object",
        }
    }

    fn badge_label(self) -> String {
        match self {
            Self::Unknown => "OLE".to_owned(),
            Self::Excel => "EXCEL".to_owned(),
            Self::Word => "WORD".to_owned(),
            Self::PowerPoint => "POWERPOINT".to_owned(),
            Self::Pdf => "PDF".to_owned(),
            Self::Visio => "VISIO".to_owned(),
            Self::MathType => "MATHTYPE".to_owned(),
        }
    }

    /// Upstream `ole-icon-primitives.ts` `ICONS` (24x24 viewBox, stroked in the brand colour).
    fn icon_shapes(self) -> &'static str {
        match self {
            Self::Excel => "<rect x=\"3\" y=\"3\" width=\"18\" height=\"18\" rx=\"2\" stroke-width=\"1.5\" fill=\"none\"/><line x1=\"3\" y1=\"9\" x2=\"21\" y2=\"9\" stroke-width=\"1\"/><line x1=\"3\" y1=\"15\" x2=\"21\" y2=\"15\" stroke-width=\"1\"/><line x1=\"9\" y1=\"3\" x2=\"9\" y2=\"21\" stroke-width=\"1\"/><line x1=\"15\" y1=\"3\" x2=\"15\" y2=\"21\" stroke-width=\"1\"/>",
            Self::Word => "<rect x=\"4\" y=\"2\" width=\"16\" height=\"20\" rx=\"2\" stroke-width=\"1.5\" fill=\"none\"/><line x1=\"7\" y1=\"7\" x2=\"17\" y2=\"7\" stroke-width=\"1.5\" stroke-linecap=\"round\"/><line x1=\"7\" y1=\"11\" x2=\"17\" y2=\"11\" stroke-width=\"1.5\" stroke-linecap=\"round\"/><line x1=\"7\" y1=\"15\" x2=\"13\" y2=\"15\" stroke-width=\"1.5\" stroke-linecap=\"round\"/>",
            Self::PowerPoint => "<rect x=\"3\" y=\"4\" width=\"18\" height=\"15\" rx=\"2\" stroke-width=\"1.5\" fill=\"none\"/><rect x=\"7\" y=\"8\" width=\"6\" height=\"7\" rx=\"1\" stroke-width=\"1.5\" fill=\"none\"/><line x1=\"15\" y1=\"9\" x2=\"19\" y2=\"9\" stroke-width=\"1.5\"/><line x1=\"15\" y1=\"12\" x2=\"19\" y2=\"12\" stroke-width=\"1.5\"/><line x1=\"9\" y1=\"19\" x2=\"15\" y2=\"19\" stroke-width=\"1.5\" stroke-linecap=\"round\"/>",
            Self::Pdf => "<rect x=\"4\" y=\"2\" width=\"16\" height=\"20\" rx=\"2\" stroke-width=\"1.5\" fill=\"none\"/><text x=\"12\" y=\"14\" text-anchor=\"middle\" font-size=\"7\" font-weight=\"bold\">PDF</text>",
            Self::Visio => "<rect x=\"8\" y=\"2\" width=\"8\" height=\"5\" rx=\"1\" stroke-width=\"1.5\" fill=\"none\"/><line x1=\"12\" y1=\"7\" x2=\"12\" y2=\"10\" stroke-width=\"1.5\"/><line x1=\"6\" y1=\"10\" x2=\"18\" y2=\"10\" stroke-width=\"1.5\"/><line x1=\"6\" y1=\"10\" x2=\"6\" y2=\"13\" stroke-width=\"1.5\"/><line x1=\"18\" y1=\"10\" x2=\"18\" y2=\"13\" stroke-width=\"1.5\"/><rect x=\"2\" y=\"13\" width=\"8\" height=\"5\" rx=\"1\" stroke-width=\"1.5\" fill=\"none\"/><rect x=\"14\" y=\"13\" width=\"8\" height=\"5\" rx=\"1\" stroke-width=\"1.5\" fill=\"none\"/>",
            Self::MathType => "<rect x=\"2\" y=\"4\" width=\"20\" height=\"16\" rx=\"2\" stroke-width=\"1.5\" fill=\"none\"/><text x=\"12\" y=\"15\" text-anchor=\"middle\" font-size=\"9\" font-weight=\"bold\" font-style=\"italic\">f(x)</text>",
            Self::Unknown => "<rect x=\"2\" y=\"5\" width=\"9\" height=\"7\" rx=\"1.5\" stroke-width=\"1.5\" fill=\"none\"/><rect x=\"13\" y=\"12\" width=\"9\" height=\"7\" rx=\"1.5\" stroke-width=\"1.5\" fill=\"none\"/><line x1=\"11\" y1=\"8.5\" x2=\"13\" y2=\"15.5\" stroke-width=\"1.5\" stroke-linecap=\"round\"/>",
        }
    }
}

fn starts_with_ci(value: &str, prefix: &str) -> bool {
    value.len() >= prefix.len() && value[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn resolve_ole_type(el: &DumpElement) -> OleType {
    // `detectOleObjectType`: progId prefixes, then known clsids. `package` / `unknown` fall
    // through to `resolveOleType`'s progId substring match.
    const PROG_PREFIXES: [(&str, OleType); 8] = [
        ("Excel.", OleType::Excel),
        ("Word.", OleType::Word),
        ("PowerPoint.", OleType::PowerPoint),
        ("Visio.", OleType::Visio),
        ("Equation.", OleType::MathType),
        ("MathType", OleType::MathType),
        ("AcroExch.", OleType::Pdf),
        ("Acrobat.", OleType::Pdf),
    ];
    let prog = el.ole_prog_id.as_deref().unwrap_or("");
    if let Some((_, t)) = PROG_PREFIXES.iter().find(|(p, _)| starts_with_ci(prog, p)) {
        return *t;
    }
    if let Some(cls) = el.ole_cls_id.as_deref() {
        let norm: String = cls
            .chars()
            .filter(|c| *c != '{' && *c != '}')
            .collect::<String>()
            .trim()
            .to_ascii_uppercase();
        match norm.as_str() {
            "00020820-0000-0000-C000-000000000046" | "00020830-0000-0000-C000-000000000046" => {
                return OleType::Excel
            }
            "00020906-0000-0000-C000-000000000046" | "F4754C9B-64F5-4B40-8AF4-679732AC0607" => {
                return OleType::Word
            }
            "B801CA65-A1FC-11D0-85AD-444553540000" => return OleType::Pdf,
            "00021A20-0000-0000-C000-000000000046" => return OleType::Visio,
            "0002CE02-0000-0000-C000-000000000046" => return OleType::MathType,
            _ => {}
        }
    }
    let lower = prog.to_ascii_lowercase();
    if lower.contains("excel") {
        OleType::Excel
    } else if lower.contains("word") {
        OleType::Word
    } else if lower.contains("powerpoint") {
        OleType::PowerPoint
    } else if lower.contains("acroexch") || lower.contains("acrobat") || lower.contains("pdf") {
        OleType::Pdf
    } else if lower.contains("visio") {
        OleType::Visio
    } else if lower.contains("equation") || lower.contains("mathtype") {
        OleType::MathType
    } else {
        OleType::Unknown
    }
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Upstream `buildPlaceholder`: brand-tinted box, 36px icon, display name (`oleName`, else type label).
fn paint_placeholder<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    el: &DumpElement,
    parent: u32,
) {
    let ty = resolve_ole_type(el);
    let color = ty.color();
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "ole");
    let boxed = create_child(ctx, id, "div");
    set_attr(ctx, boxed, "class", "pptxv-ole-placeholder");
    set_style(ctx, boxed, "border", &format!("2px solid {color}33"));
    set_style(ctx, boxed, "border-radius", "6px");
    set_style(ctx, boxed, "background-color", &format!("{color}0d"));
    set_style(ctx, boxed, "width", "100%");
    set_style(ctx, boxed, "height", "100%");
    set_style(ctx, boxed, "display", "flex");
    set_style(ctx, boxed, "flex-direction", "column");
    set_style(ctx, boxed, "align-items", "center");
    set_style(ctx, boxed, "justify-content", "center");
    set_style(ctx, boxed, "pointer-events", "none");
    set_style(ctx, boxed, "box-sizing", "border-box");
    let name = el.ole_name.as_deref().unwrap_or(ty.label());
    let markup = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"36\" height=\"36\" viewBox=\"0 0 24 24\" fill=\"none\" aria-hidden=\"true\" stroke=\"{color}\" style=\"color:{color}\">{icon}</svg>\
<span class=\"pptxv-ole-name\" style=\"margin-top:8px;font-size:12px;font-weight:500;max-width:90%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:{color}\">{name}</span>",
        icon = ty.icon_shapes().replace("<text ", &format!("<text fill=\"{color}\" stroke=\"none\" ")),
        name = escape_html(name),
    );
    set_inner_markup_stub(ctx, boxed, &markup);
    ctx.view_ir.push_painted("ole", el, None);
}

fn paint_preview<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    // Outer wrapper stays `position:absolute` from `position_box` (upstream
    // `getContainerStyle`). Nested preview is the relative host for the badge.
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, el.height);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "ole");

    let preview = create_child(ctx, id, "div");
    set_attr(ctx, preview, "class", "pptxv-ole-preview");
    set_style(ctx, preview, "position", "relative");
    set_style(ctx, preview, "width", "100%");
    set_style(ctx, preview, "height", "100%");

    let img = create_child(ctx, preview, "img");
    set_attr(ctx, img, "alt", el.name.as_deref().unwrap_or(""));
    set_style(ctx, img, "width", "100%");
    set_style(ctx, img, "height", "100%");
    set_style(ctx, img, "object-fit", "contain");
    set_style(ctx, img, "display", "block");
    set_style(ctx, img, "pointer-events", "none");
    if let Some(src) = el.image_src.as_deref() {
        // Prefer <img src> so the loader can resolve nitro-metafile: → PNG.
        set_attr(ctx, img, "src", src);
        if !src.starts_with("nitro-metafile:") {
            let url = format!("url({src})");
            set_style(ctx, preview, "background-image", &url);
            set_style(ctx, preview, "background-size", "contain");
            set_style(ctx, preview, "background-repeat", "no-repeat");
            set_style(ctx, preview, "background-position", "center");
        }
    }
    // Upstream `buildPreview` type badge (bottom-right).
    let badge = create_child(ctx, preview, "div");
    set_attr(ctx, badge, "aria-hidden", "true");
    set_style(ctx, badge, "position", "absolute");
    set_style(ctx, badge, "bottom", "4px");
    set_style(ctx, badge, "right", "4px");
    set_style(ctx, badge, "z-index", "10");
    set_style(ctx, badge, "pointer-events", "none");
    let ty = resolve_ole_type(el);
    let badge_label = ty.badge_label();
    let font_size = if badge_label.len() > 4 { 6 } else { 10 };
    set_inner_markup_stub(
        ctx,
        badge,
        &format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" class=\"pptxv-ole-badge\">\
<rect x=\"2\" y=\"2\" width=\"20\" height=\"20\" rx=\"3\" fill=\"{}\"/>\
<text x=\"12\" y=\"16\" text-anchor=\"middle\" fill=\"white\" font-size=\"{font_size}\" font-weight=\"bold\" font-family=\"Liberation Sans, Arial, sans-serif\">{badge_label}</text>\
</svg>",
            ty.color()
        ),
    );
    ctx.view_ir.push_painted("ole", el, None);
}
