// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/ribbon/icon-paths.ts and commands/icons.ts; changes listed in PROVENANCE.md.

//! Ribbon stroke icons (24×24 grid) for Home tab parity (#150).

fn esc_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;")
}

pub fn icon_path(name: &str) -> Option<&'static str> {
    match name {
        "paste" => Some("M8 4h8v3H8z M6 5.5H5v15h14v-15h-1 M9 12h6 M9 15.5h6"),
        "cut" => Some("M6 3l9 12 M18 3l-9 12 M7 15.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z M17 15.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z"),
        "copy" => Some("M9 8h11v13H9z M6 16H4V3h11v2"),
        "formatPainter" => Some("M5 4h11v5H5z M16 6.5h3v5h-8v3 M9.5 14.5h3V21h-3z"),
        "bold" => Some("M7 4h6a3.5 3.5 0 0 1 0 7H7z M7 11h7a3.5 3.5 0 0 1 0 7H7z"),
        "italic" => Some("M10 4h8 M6 20h8 M15 4l-6 16"),
        "underline" => Some("M7 4v7a5 5 0 0 0 10 0V4 M5 21h14"),
        "strike" => Some("M4 12h16 M16.5 7.5C16 5.5 14 4.5 12 4.5c-2.5 0-4.2 1.3-4.2 3.2 0 1.6 1.2 2.4 4.2 3 M8 16.5c.5 2 2.3 3 4.2 3 2.6 0 4.3-1.3 4.3-3.3 0-1.2-.6-2-2-2.5"),
        "fontGrow" => Some("M3 19 8.5 5 14 19 M5 14h7 M17 9V3 M14 6h6"),
        "fontShrink" => Some("M3 19 8.5 5 14 19 M5 14h7 M14 6h6"),
        "fontColor" => Some("M6 17 12 4l6 13 M8.3 12h7.4"),
        "fillColor" => Some("M5 11l6-6 8 8-6 6z M11 5l-3-2 M19 16c1 1.5 1.5 2.5 1.5 3.3a1.5 1.5 0 0 1-3 0c0-.8.5-1.8 1.5-3.3z"),
        "borders" => Some("M4 4h16v16H4z M4 12h16 M12 4v16"),
        "alignTop" => Some("M4 6h16 M4 10h10 M4 14h16 M4 18h10"),
        "alignMiddle" => Some("M4 6h16 M7 10h10 M4 14h16 M7 18h10"),
        "alignBottom" => Some("M4 6h16 M10 10h10 M4 14h16 M10 18h10"),
        "alignLeft" => Some("M4 6h16 M4 10h10 M4 14h16 M4 18h10"),
        "alignCenter" => Some("M4 6h16 M7 10h10 M4 14h16 M7 18h10"),
        "alignRight" => Some("M4 6h16 M10 10h10 M4 14h16 M10 18h10"),
        "outdent" => Some("M8 6h12 M12 11h8 M12 16h8 M8 21h12 M7 9l-4 3 4 3z"),
        "indent" => Some("M8 6h12 M12 11h8 M12 16h8 M8 21h12 M3 9l4 3-4 3z"),
        "sum" => Some("M17 5H6l6 7-6 7h11"),
        "fill" => Some("M12 4v14 M7 13l5 5 5-5 M5 21h14"),
        "sortFilter" => Some("M4 5h16l-6 7v6l-4 2v-8z"),
        "find" => Some("M10 4a6 6 0 1 0 0 12 6 6 0 0 0 0-12z M14.5 14.5 20 20"),
        "caret" => Some("M7 10l5 5 5-5"),
        "launcher" => Some("M14 4h6v6 M20 4l-7 7 M10 6H5v13h13v-5"),
        "conditional" => Some("M4 4h16v16H4z M4 8h16 M4 13h16 M8 3v14 M10 10h5 M10 15h3"),
        "formatTable" => Some("M4 4h16v16H4z M4 7h16 M4 11h16 M4 15h16 M8 3v14"),
        "cellStyles" => Some("M3 5h14v10H3zM6 9h4M6 12h8M13 7v.1"),
        "insertCells" => Some("M3 3h10v10H3zM3 8h10M8 3v10M15 12v6M12 15h6"),
        "deleteCells" => Some("M3 3h10v10H3zM3 8h10M8 3v10M13 13l4.5 4.5M17.5 13 13 17.5"),
        "formatCells" => Some("M3 3h10v10H3zM3 8h10M8 3v10M12 17l1-3 4-4 2 2-4 4z"),
        "orientation" => Some("M4 16 9 4l3 7M5.5 12h5M13 16l4-4M17 16v-4h-4"),
        "wrapText" => Some("M3 4h14M3 9h11a2.5 2.5 0 0 1 0 5h-3M12.5 12 11 14l1.5 2M3 14h5"),
        "mergeCenter" => Some("M2 4h16v12H2zM5 10h10M7 8l-2 2 2 2M13 8l2 2-2 2"),
        "percent" => Some("M15 5 5 15M6.5 4.5a2 2 0 1 0 0 4 2 2 0 0 0 0-4zM13.5 11.5a2 2 0 1 0 0 4 2 2 0 0 0 0-4z"),
        "comma" => Some("M9 13.5h1.5v1.5c0 1.5-.8 2.5-2 3"),
        "decimalIncrease" => Some("M3 14h.1M6 11a2 2 0 1 1 4 0v3a2 2 0 1 1-4 0zM12 12h5M15 10l2 2-2 2"),
        "decimalDecrease" => Some("M3 14h.1M6 11a2 2 0 1 1 4 0v3a2 2 0 1 1-4 0zM12 12h5M14 10l-2 2 2 2"),
        "accounting" => Some("M3 5h14v10H3zM10 7v6M11.8 8.3c-.3-.5-1-.8-1.8-.8-1 0-1.7.5-1.7 1.2 0 1.6 3.5.8 3.5 2.5 0 .7-.8 1.3-1.8 1.3-.8 0-1.5-.3-1.9-.9"),
        "numberFormat" => Some("M3 4h14v12H3zM6 8h1v4M10 8h3l-2 4M15 8h.1M15 12h.1"),
        "clear" => Some("M4 20l4-1 11-11-3-3L5 16z M13 7l3 3 M14 20h7"),
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
        "home.borders" | "home.borders-more" => icon_path("borders"),
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
    format!(
        "<svg class=\"ribbon-icon\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" data-icon=\"{name}\"><path d=\"{d}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.7\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>",
        size = size,
        name = esc_attr(icon_name),
        d = path,
    )
}
