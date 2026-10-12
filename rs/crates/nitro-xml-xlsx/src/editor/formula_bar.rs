// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/formula-bar/index.ts,
// formula-input.ts and styles.ts; changes listed in PROVENANCE.md.
//! Formula bar HTML: name box, cancel / enter / fx, and a Rust-owned text surface.
//!
//! The field is a `role="textbox"` div, not a `<textarea>`. Caret and text live
//! on [`super::CellEditor`]; the wasm host feeds keys through `text::apply_key`.

/// Snapshot the chrome fragment is rendered from.
#[derive(Debug, Clone)]
pub struct FormulaBarView {
    pub read_only: bool,
    pub name: String,
    pub text: String,
    pub caret: usize,
    pub editing: bool,
    pub expanded: bool,
}

/// Name box + actions + formula field. Empty in read-only mode (negative control).
pub fn formula_bar_html(view: &FormulaBarView) -> String {
    if view.read_only {
        return String::new();
    }
    let editing = if view.editing { " xfb-editing" } else { "" };
    let expanded = if view.expanded { " xfb-expanded" } else { "" };
    let (before, after) = split_caret(&view.text, view.caret);
    let caret_hidden = if view.editing { "" } else { " hidden" };
    format!(
        r##"<div class="xfb{editing}{expanded}" part="formula-bar" role="group" aria-label="Formula Bar"><style>
.xfb{{position:relative;display:flex;align-items:stretch;gap:0;min-height:26px;padding:3px 6px;box-sizing:border-box;background:#fff;border-bottom:1px solid #e1dfdd;font:12px/1.4 system-ui,sans-serif;color:#252423}}
.xfb-name{{display:flex;align-items:center;width:110px;flex:none;border:1px solid #c8c6c4;border-radius:2px;padding:0 6px;height:22px;box-sizing:border-box;overflow:hidden;white-space:nowrap}}
.xfb-actions{{display:flex;align-items:center;flex:none;padding:0 4px;margin-left:4px;border-left:1px solid #e1dfdd}}
.xfb-btn{{border:0;background:transparent;color:inherit;width:24px;height:22px;padding:0;cursor:pointer}}
.xfb-cancel,.xfb-enter{{display:none}}
.xfb-editing .xfb-cancel,.xfb-editing .xfb-enter{{display:inline-flex;align-items:center;justify-content:center}}
.xfb-field{{flex:1;min-width:0;height:22px;border:1px solid transparent;border-radius:2px;padding:1px 4px;font:13px/18px Calibri,Carlito,'Segoe UI',sans-serif;white-space:pre;overflow:hidden;box-sizing:border-box}}
.xfb-field:focus{{border-color:#c8c6c4;outline:none}}
.xfb-expanded .xfb-field{{height:76px;white-space:pre-wrap}}
.xfb-caret{{display:inline-block;width:1px;height:1em;background:#252423;vertical-align:text-bottom}}
.xfb-caret[hidden]{{display:none}}
.xfb-save{{margin-left:4px;font:inherit;padding:0 6px}}
</style>
<div class="xfb-name" part="name-box" aria-label="Name Box">{name}</div>
<div class="xfb-actions">
<button type="button" class="xfb-btn xfb-cancel" data-a="formula-cancel" aria-label="Cancel" title="Cancel">✕</button>
<button type="button" class="xfb-btn xfb-enter" data-a="formula-commit" aria-label="Enter" title="Enter">✓</button>
<button type="button" class="xfb-btn xfb-fx" data-a="formula-fx" aria-label="Insert Function" title="Insert Function"><i>fx</i></button>
</div>
<div class="xfb-field" part="formula-input" role="textbox" tabindex="0" data-a="formula-focus" aria-label="Formula Bar" aria-multiline="{multiline}">{before}<span class="xfb-caret"{caret_hidden}></span>{after}</div>
<button type="button" class="xfb-btn xfb-expand" data-a="formula-expand" aria-label="{expand_label}" aria-expanded="{expanded_attr}">▾</button>
<button type="button" class="xfb-save" data-a="save" aria-label="Save">Save</button>
</div>"##,
        editing = editing,
        expanded = expanded,
        name = esc(&view.name),
        before = esc(before),
        after = esc(after),
        caret_hidden = caret_hidden,
        multiline = if view.expanded { "true" } else { "false" },
        expand_label = if view.expanded {
            "Collapse Formula Bar"
        } else {
            "Expand Formula Bar"
        },
        expanded_attr = if view.expanded { "true" } else { "false" },
    )
}

fn split_caret(text: &str, caret: usize) -> (&str, &str) {
    let mut c = caret.min(text.len());
    while c > 0 && !text.is_char_boundary(c) {
        c -= 1;
    }
    text.split_at(c)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_formula_bar_html_is_empty() {
        // Catches: readonly viewers must not mount formula-bar chrome.
        let view = FormulaBarView {
            read_only: true,
            name: "A1".into(),
            text: "=A1".into(),
            caret: 3,
            editing: true,
            expanded: false,
        };
        let html = formula_bar_html(&view);
        assert!(html.is_empty(), "readonly formula bar must be empty");
        assert!(!html.contains("xfb"));
        assert!(!html.contains("textarea"));
    }

    #[test]
    fn formula_bar_is_a_text_surface_not_a_textarea() {
        // Catches: shipped formula bar must not mount a DOM textarea.
        let view = FormulaBarView {
            read_only: false,
            name: "A1".into(),
            text: "=SUM(A1)".into(),
            caret: 4,
            editing: true,
            expanded: false,
        };
        let html = formula_bar_html(&view);
        assert!(html.contains("part=\"formula-bar\""));
        assert!(html.contains("role=\"textbox\""));
        assert!(html.contains("data-a=\"formula-commit\""));
        assert!(html.contains("data-a=\"save\""));
        assert!(!html.to_ascii_lowercase().contains("<textarea"));
        assert!(!html.to_ascii_lowercase().contains("<input"));
    }
}
