// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/commands/icons.ts (SHEET_ICONS) + ribbon/icon-paths.ts; changes listed in PROVENANCE.md.

//! Ribbon stroke icons: 20×20 sheet glyphs (stroke 1.5) matching upstream `xl-*` (#150 edit-chrome).

fn esc_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;")
}

pub fn icon_path(name: &str) -> Option<&'static str> {
    match name {
        "paste" => Some("M7 3h6v2.5H7zM5.5 4H4v13h12V4h-1.5M7 10h6M7 13h4"),
        "cut" => Some("M5 3l7.5 9.5M15 3l-7.5 9.5M5.5 12.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5zM14.5 12.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z"),
        "copy" => Some("M7 6h10v11H7zM4 14V3h9"),
        "formatPainter" => Some("M4 3h10v4H4zM14 5h2v4H9v3M8 12h2v5H8z"),
        "bold" => Some("M6 3h5a3 3 0 0 1 0 6H6zM6 9h6a3.5 3.5 0 0 1 0 7H6z"),
        "italic" => Some("M9 3h6M5 17h6M12 3 8 17"),
        "underline" => Some("M6 3v6a4 4 0 0 0 8 0V3M4 17h12"),
        "strike" => Some("M3 10h14M13.5 6c-.5-1.6-2-2.5-3.6-2.5-2 0-3.4 1-3.4 2.6M6.5 14c.5 1.6 2 2.5 3.7 2.5 2.1 0 3.5-1.1 3.5-2.7"),
        "fontGrow" => Some("M2 16 6.5 4 11 16M3.6 12h5.8M15 8V3M12.5 5.5h5"),
        "fontShrink" => Some("M2 16 6.5 4 11 16M3.6 12h5.8M12.5 5.5h5"),
        "fontColor" => Some("M5 13 10 3l5 10M7 9h6M3 17h14"),
        "fillColor" => Some("M4 9l5-5 6 6-5 5zM9 4 7 2M15 10l1.5 3a1.5 1.5 0 1 1-3 0zM3 17h14"),
        // Upstream Home ribbon faces `xl-borderBottom` (not All Borders).
        "borders" | "borderBottom" => {
            Some("M3 17h14M3 3h.1M7 3h.1M11 3h.1M15 3h.1M3 7h.1M3 11h.1M17 7h.1M17 11h.1M17 3h.1")
        }
        "borderMore" => Some("M3 3h14v14H3zM3 10h14M10 3v14M13 13l4 4"),
        "alignTop" => Some("M3 3h14M7 6h6M7 9h6M10 9v6"),
        "alignMiddle" => Some("M3 10h14M7 5h6M7 15h6M10 2v4M10 14v4"),
        "alignBottom" => Some("M3 17h14M7 14h6M7 11h6M10 11V5"),
        "alignLeft" => Some("M3 4h14M3 8h9M3 12h14M3 16h9"),
        "alignCenter" => Some("M3 4h14M5.5 8h9M3 12h14M5.5 16h9"),
        "alignRight" => Some("M3 4h14M8 8h9M3 12h14M8 16h9"),
        "outdent" => Some("M8 4h9M8 8h9M8 12h9M8 16h9M6 7.5 3 10l3 2.5"),
        "indent" => Some("M8 4h9M8 8h9M8 12h9M8 16h9M3 7.5 6 10l-3 2.5"),
        "orientation" => Some("M4 16 9 4l3 7M5.5 12h5M13 16l4-4M17 16v-4h-4"),
        "wrapText" => Some("M3 4h14M3 9h11a2.5 2.5 0 0 1 0 5h-3M12.5 12 11 14l1.5 2M3 14h5"),
        "mergeCenter" => Some("M2 4h16v12H2zM5 10h10M7 8l-2 2 2 2M13 8l2 2-2 2"),
        "sum" => Some("M15 4H5l5 6-5 6h10"),
        "fill" => Some("M4 3h12v5H4zM10 10v7M7 14l3 3 3-3"),
        "sortFilter" => Some("M5 4v12M2.5 13.5 5 16l2.5-2.5M10 5h7l-2.8 3.5V13l-1.4 1V8.5z"),
        "find" => Some("M3 8.5a5.5 5.5 0 1 0 11 0 5.5 5.5 0 1 0-11 0M13 13l4 4"),
        "conditional" => Some("M3 3h14v14H3zM3 8h14M3 13h14M8 3v14M10 10h5M10 15h3"),
        "formatTable" => Some("M3 3h14v14H3zM3 7h14M3 11h14M3 15h14M8 3v14"),
        "cellStyles" => Some("M3 5h14v10H3zM6 9h4M6 12h8M13 7v.1"),
        "insertCells" => Some("M3 3h10v10H3zM3 8h10M8 3v10M15 12v6M12 15h6"),
        "deleteCells" => Some("M3 3h10v10H3zM3 8h10M8 3v10M13 13l4.5 4.5M17.5 13 13 17.5"),
        "formatCells" => Some("M3 3h10v10H3zM3 8h10M8 3v10M12 17l1-3 4-4 2 2-4 4z"),
        "percent" => Some("M15 5 5 15M6.5 4.5a2 2 0 1 0 0 4 2 2 0 0 0 0-4zM13.5 11.5a2 2 0 1 0 0 4 2 2 0 0 0 0-4z"),
        "comma" => Some("M9 13.5h1.5v1.5c0 1.5-.8 2.5-2 3"),
        "decimalIncrease" => Some("M3 14h.1M6 11a2 2 0 1 1 4 0v3a2 2 0 1 1-4 0zM12 12h5M15 10l2 2-2 2"),
        "decimalDecrease" => Some("M3 14h.1M6 11a2 2 0 1 1 4 0v3a2 2 0 1 1-4 0zM12 12h5M14 10l-2 2 2 2"),
        "accounting" => Some("M3 5h14v10H3zM10 7v6M11.8 8.3c-.3-.5-1-.8-1.8-.8-1 0-1.7.5-1.7 1.2 0 1.6 3.5.8 3.5 2.5 0 .7-.8 1.3-1.8 1.3-.8 0-1.5-.3-1.9-.9"),
        "numberFormat" => Some("M3 4h14v12H3zM6 8h1v4M10 8h3l-2 4M15 8h.1M15 12h.1"),
        "clear" => Some("M3 16l3-1 8.5-8.5-2-2L4 13zM11 6l2 2M12 17h6"),
        "caret" => Some("M7 10l5 5 5-5"),
        "launcher" => Some("M14 4h6v6 M20 4l-7 7 M10 6H5v13h13v-5"),
        _ => None,
    }
}

pub fn command_icon(command: &str) -> Option<&'static str> {
    match command {
        "home.paste" => icon_path("paste"),
        "home.cut" => icon_path("cut"),
        "home.copy" => icon_path("copy"),
        "home.format-painter" => icon_path("formatPainter"),
        "home.grow-font" => icon_path("fontGrow"),
        "home.shrink-font" => icon_path("fontShrink"),
        "home.bold" => icon_path("bold"),
        "home.italic" => icon_path("italic"),
        "home.underline" | "home.underline-double" => icon_path("underline"),
        "home.strikethrough" => icon_path("strike"),
        "home.borders" => icon_path("borderBottom"),
        "home.borders-more" => icon_path("borderMore"),
        "home.fill-color" => icon_path("fillColor"),
        "home.font-color" => icon_path("fontColor"),
        "home.align-top" => icon_path("alignTop"),
        "home.align-middle" => icon_path("alignMiddle"),
        "home.align-bottom" => icon_path("alignBottom"),
        "home.align-left" => icon_path("alignLeft"),
        "home.align-center" => icon_path("alignCenter"),
        "home.align-right" => icon_path("alignRight"),
        "home.indent-decrease" => icon_path("outdent"),
        "home.indent-increase" => icon_path("indent"),
        "home.merge-center" => icon_path("mergeCenter"),
        "home.wrap-text" => icon_path("wrapText"),
        "home.autosum" => icon_path("sum"),
        "home.percent" => icon_path("percent"),
        "home.comma" => icon_path("comma"),
        "home.decimal-increase" => icon_path("decimalIncrease"),
        "home.decimal-decrease" => icon_path("decimalDecrease"),
        "home.accounting" => icon_path("accounting"),
        "home.number-format" => icon_path("numberFormat"),
        "cells.insert-split" | "cells.insert" => icon_path("insertCells"),
        "cells.delete-split" | "cells.delete" => icon_path("deleteCells"),
        "home.format-as-table" => icon_path("formatTable"),
        "home.cell-styles" => icon_path("cellStyles"),
        "home.find" | "home.replace" | "home.go-to" => icon_path("find"),
        _ => None,
    }
}

pub fn menu_icon(label: &str) -> Option<&'static str> {
    match label {
        "Orientation" => icon_path("orientation"),
        "Conditional Formatting" => icon_path("conditional"),
        "Format" => icon_path("formatCells"),
        "Fill" => icon_path("fill"),
        "Clear" => icon_path("clear"),
        "Sort & Filter" => icon_path("sortFilter"),
        "Find & Select" => icon_path("find"),
        _ => None,
    }
}

pub fn ribbon_icon_svg(path: &str, size: u32, icon_name: &str) -> String {
    // caret/launcher stay on the 24-grid; sheet command glyphs are 20×20 / stroke 1.5.
    let (vb, sw) = if icon_name == "caret" || icon_name == "launcher" {
        ("0 0 24 24", "1.7")
    } else {
        ("0 0 20 20", "1.5")
    };
    format!(
        "<svg class=\"ribbon-icon\" width=\"{size}\" height=\"{size}\" viewBox=\"{vb}\" aria-hidden=\"true\" data-icon=\"{name}\"><path d=\"{d}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"{sw}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>",
        size = size,
        vb = vb,
        sw = sw,
        name = esc_attr(icon_name),
        d = path,
    )
}
