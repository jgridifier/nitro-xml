// SPDX-License-Identifier: MIT
//! PowerPoint Home / Insert ribbon HTML and stable control ids (#88).

use super::commands::PptxCommandSurface;
use super::selection::SelectionState;

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

fn button(id: &str, label: &str, surface: &PptxCommandSurface, large: bool) -> String {
    let enabled = surface.is_enabled(id);
    let cls = if large {
        "pve-ribbon-large"
    } else {
        "pve-ribbon-inline"
    };
    format!(
        "<button type=\"button\" class=\"{cls}\" data-command=\"{id}\" aria-label=\"{lbl}\"{dis}>{lbl}</button>",
        cls = cls,
        id = esc(id),
        lbl = esc(label),
        dis = if enabled { "" } else { " disabled" },
    )
}

/// Home + Insert tab ribbon markup (subset of upstream PowerPoint chrome).
pub fn ribbon_html(
    surface: &PptxCommandSurface,
    selection: &SelectionState,
    active_tab: &str,
) -> String {
    let _ = selection;
    let mut html = String::from("<nav class=\"pve-ribbon\" aria-label=\"Ribbon\">");
    html.push_str(&format!(
        "<div class=\"pve-ribbon-tabs\" role=\"tablist\">\
<button type=\"button\" role=\"tab\" aria-selected=\"{home}\" data-tab=\"home\">Home</button>\
<button type=\"button\" role=\"tab\" aria-selected=\"{insert}\" data-tab=\"insert\">Insert</button>\
</div>",
        home = active_tab == "home",
        insert = active_tab == "insert",
    ));
    if active_tab == "home" {
        html.push_str("<div class=\"pve-ribbon-panel\" role=\"tabpanel\" data-tab=\"home\">");
        html.push_str("<section class=\"pve-ribbon-group\" aria-label=\"Clipboard\">");
        html.push_str(&button("file.save", "Save", surface, true));
        html.push_str(&button("edit.undo", "Undo", surface, false));
        html.push_str(&button("edit.redo", "Redo", surface, false));
        html.push_str("</section>");
        html.push_str("<section class=\"pve-ribbon-group\" aria-label=\"Font\">");
        html.push_str(&button("home.bold", "Bold", surface, false));
        html.push_str(&button("home.italic", "Italic", surface, false));
        html.push_str(&button("home.underline", "Underline", surface, false));
        html.push_str(&button("home.align-left", "Align Left", surface, false));
        html.push_str(&button("home.align-center", "Center", surface, false));
        html.push_str(&button("home.align-right", "Align Right", surface, false));
        html.push_str("</section>");
        html.push_str("<section class=\"pve-ribbon-group\" aria-label=\"Drawing\">");
        html.push_str(&button("home.duplicate", "Duplicate", surface, false));
        html.push_str(&button("home.delete", "Delete", surface, false));
        html.push_str(&button(
            "home.bring-forward",
            "Bring Forward",
            surface,
            false,
        ));
        html.push_str(&button(
            "home.send-backward",
            "Send Backward",
            surface,
            false,
        ));
        html.push_str(&button("home.group", "Group", surface, false));
        html.push_str(&button("home.ungroup", "Ungroup", surface, false));
        html.push_str("</section>");
        html.push_str("</div>");
    } else if active_tab == "insert" {
        html.push_str("<div class=\"pve-ribbon-panel\" role=\"tabpanel\" data-tab=\"insert\">");
        html.push_str("<section class=\"pve-ribbon-group\" aria-label=\"Shapes\">");
        html.push_str(&button("insert.shape", "Shape", surface, true));
        html.push_str(&button("insert.text-box", "Text Box", surface, false));
        html.push_str(&button("insert.table", "Table", surface, false));
        html.push_str("</section>");
        html.push_str("<section class=\"pve-ribbon-group\" aria-label=\"Slides\">");
        html.push_str(&button("insert.slide", "New Slide", surface, false));
        html.push_str(&button("home.delete-slide", "Delete Slide", surface, false));
        html.push_str("</section>");
        html.push_str("</div>");
    }
    html.push_str("</nav>");
    html
}

/// Slide navigator strip under the ribbon (active slide highlighted).
pub fn slide_nav_html(slide_count: usize, selection: &SelectionState) -> String {
    let mut html =
        String::from("<nav class=\"pve-slide-nav\" aria-label=\"Slides\" role=\"list\">");
    for i in 0..slide_count {
        let selected = selection.slide_index as usize == i;
        html.push_str(&format!(
            "<button type=\"button\" role=\"listitem\" class=\"pve-slide-thumb\" data-slide=\"{i}\" aria-label=\"Slide {n}\" aria-current=\"{cur}\">{n}</button>",
            n = i + 1,
            cur = if selected { "true" } else { "false" },
        ));
    }
    html.push_str("</nav>");
    html
}
