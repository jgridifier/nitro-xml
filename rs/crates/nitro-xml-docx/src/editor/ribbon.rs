// SPDX-License-Identifier: MIT
//! Word Home ribbon HTML and stable control ids (#83).

use super::commands::DocxCommandSurface;

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn button(id: &str, label: &str, surface: &DocxCommandSurface, large: bool) -> String {
    let enabled = surface.is_enabled(id);
    let cls = if large {
        "dve-ribbon-large"
    } else {
        "dve-ribbon-inline"
    };
    format!(
        "<button type=\"button\" class=\"{cls}\" data-command=\"{id}\" aria-label=\"{lbl}\"{dis}>{lbl}</button>",
        cls = cls,
        id = esc(id),
        lbl = esc(label),
        dis = if enabled { "" } else { " disabled" },
    )
}

/// Home tab ribbon markup (subset of upstream Word Home: Font, Insert, Editing).
/// Paste, cut, and copy are registered stubs with no runner, so they are omitted.
pub fn ribbon_html(surface: &DocxCommandSurface, active_tab: &str) -> String {
    let tab = esc(active_tab);
    let mut html = String::from("<nav class=\"dve-ribbon\" aria-label=\"Ribbon\">");
    html.push_str(&format!(
        "<div class=\"dve-ribbon-tabs\" role=\"tablist\"><button type=\"button\" role=\"tab\" aria-selected=\"{}\" data-tab=\"home\">Home</button></div>",
        active_tab == "home"
    ));
    if active_tab == "home" {
        html.push_str("<div class=\"dve-ribbon-panel\" role=\"tabpanel\" data-tab=\"home\">");
        html.push_str("<section class=\"dve-ribbon-group\" aria-label=\"Font\">");
        html.push_str(&button("bold", "Bold", surface, false));
        html.push_str(&button("italic", "Italic", surface, false));
        html.push_str(&button("underline", "Underline", surface, false));
        html.push_str(&button(
            "double-underline",
            "Double Underline",
            surface,
            false,
        ));
        html.push_str(&button("caps", "All Caps", surface, false));
        html.push_str(&button("hidden", "Hidden", surface, false));
        html.push_str(&button("expand-spacing", "Expand", surface, false));
        html.push_str(&button("condense-spacing", "Condense", surface, false));
        html.push_str(&button("scale-up", "Scale Up", surface, false));
        html.push_str(&button("scale-down", "Scale Down", surface, false));
        html.push_str(&button("strike", "Strikethrough", surface, false));
        html.push_str(&button("highlight", "Highlight", surface, false));
        html.push_str(&button("subscript", "Subscript", surface, false));
        html.push_str(&button("superscript", "Superscript", surface, false));
        html.push_str(&button("font-name", "Font", surface, false));
        html.push_str(&button("align-left", "Align Left", surface, false));
        html.push_str(&button("align-center", "Center", surface, false));
        html.push_str(&button("align-right", "Align Right", surface, false));
        html.push_str(&button("align-justify", "Justify", surface, false));
        html.push_str(&button("bullets", "Bullets", surface, false));
        html.push_str(&button("numbering", "Numbering", surface, false));
        html.push_str(&button("indent", "Increase Indent", surface, false));
        html.push_str(&button("outdent", "Decrease Indent", surface, false));
        html.push_str(&button(
            "indent-right",
            "Increase Right Indent",
            surface,
            false,
        ));
        html.push_str(&button(
            "outdent-right",
            "Decrease Right Indent",
            surface,
            false,
        ));
        html.push_str(&button("line-spacing", "Line Spacing", surface, false));
        html.push_str(&button("shading", "Shade", surface, false));
        html.push_str(&button("border-bottom", "Bottom Border", surface, false));
        html.push_str(&button("space-before", "Space Before", surface, false));
        html.push_str(&button("space-after", "Space After", surface, false));
        html.push_str(&button("hanging-indent", "Hanging", surface, false));
        html.push_str(&button("first-line", "First Line", surface, false));
        html.push_str(&button("font-size", "Font Size", surface, false));
        html.push_str(&button("grow-font", "Grow Font", surface, false));
        html.push_str(&button("shrink-font", "Shrink Font", surface, false));
        html.push_str(&button("font-color", "Font Color", surface, false));
        html.push_str(&button("style-normal", "Normal", surface, false));
        html.push_str(&button("style-title", "Title", surface, false));
        html.push_str(&button("style-heading-1", "Heading 1", surface, false));
        html.push_str(&button("style-heading-2", "Heading 2", surface, false));
        html.push_str(&button("style-heading-3", "Heading 3", surface, false));
        html.push_str("</section>");
        html.push_str("<section class=\"dve-ribbon-group\" aria-label=\"Insert\">");
        html.push_str(&button("insert.table", "Table", surface, false));
        html.push_str(&button("insert.picture", "Picture", surface, false));
        html.push_str(&button("insert.page-break", "Page Break", surface, false));
        html.push_str(&button("table.insert-row", "Insert Row", surface, false));
        html.push_str(&button("table.delete-row", "Delete Row", surface, false));
        html.push_str(&button(
            "table.insert-column",
            "Insert Column",
            surface,
            false,
        ));
        html.push_str(&button(
            "table.delete-column",
            "Delete Column",
            surface,
            false,
        ));
        html.push_str(&button("table.cell-text", "Cell Text", surface, false));
        html.push_str(&button("table.delete", "Delete Table", surface, false));
        html.push_str("</section>");
        html.push_str("<section class=\"dve-ribbon-group\" aria-label=\"Editing\">");
        html.push_str(&button("find-and-replace", "Find", surface, false));
        html.push_str(&button("replace", "Replace", surface, false));
        html.push_str("</section>");
        html.push_str("</div>");
    }
    html.push_str("</nav>");
    let _ = tab;
    html
}

/// Search panel HTML shown under the ribbon when find/replace is open.
pub fn search_panel_html(surface: &DocxCommandSurface) -> String {
    if !surface.search_panel_open {
        return String::new();
    }
    let st = surface.search.status();
    format!(
        r#"<section class="dve-search-panel" role="region" aria-label="Find and replace">
<label>Find<input type="search" name="find" aria-label="Find text" /></label>
<label>Replace<input type="text" name="replace" aria-label="Replace with" /></label>
<span role="status">{count} matches</span>
<button type="button" data-command="search.findPrevious" aria-label="Find previous">Previous</button>
<button type="button" data-command="search.findNext" aria-label="Find next">Next</button>
<button type="button" data-command="search.replaceOne" aria-label="Replace"{dis1}>Replace</button>
<button type="button" data-command="search.replaceAll" aria-label="Replace all"{dis2}>Replace all</button>
</section>"#,
        count = st.count,
        dis1 = if st.read_only { " disabled" } else { "" },
        dis2 = if st.read_only { " disabled" } else { "" },
    )
}
