// SPDX-License-Identifier: MIT
//! Accessibility projection for the single-line text surface (#59).
//!
//! Maps [`TextField`] state to textbox ARIA attributes and polite live-region
//! strings. Host paint wiring (formula bar / cell editor DOM) is #69; this
//! module is the Rust source of truth for the edit a11y gate.

use super::selection::TextField;

/// Labels used by upstream `formula-field.ts` / `cell-editor.ts`.
pub const LABEL_FORMULA_BAR: &str = "Formula Bar";
#[allow(dead_code)]
pub const LABEL_CELL_EDITOR: &str = "Cell editor";

/// Live-region announcement kinds (polite `role=status` / `.xg-live`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnounceKind {
    /// Caret moved with a collapsed selection.
    Caret,
    /// Non-empty text selection.
    Selection,
    /// Text buffer changed by an edit operation.
    Edit,
}

/// Host options mirroring upstream formula field (`textarea` + label).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldA11yOpts {
    pub label: String,
    pub readonly: bool,
    pub multiline: bool,
}

impl FieldA11yOpts {
    #[allow(dead_code)]
    pub fn formula_bar(readonly: bool) -> Self {
        Self {
            label: LABEL_FORMULA_BAR.to_string(),
            readonly,
            multiline: true,
        }
    }

    #[allow(dead_code)]
    pub fn cell_editor(readonly: bool) -> Self {
        Self {
            label: LABEL_CELL_EDITOR.to_string(),
            readonly,
            multiline: true,
        }
    }
}

/// Projected accessibility state for one editable field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldA11y {
    pub role: String,
    pub label: String,
    pub value: String,
    pub selection_start: usize,
    pub selection_end: usize,
    pub readonly: bool,
    pub multiline: bool,
    /// When true the host must not expose an editable textbox (viewer / read-only).
    pub hidden: bool,
}

/// Build the accessibility snapshot for `field` and `opts`.
pub fn field_a11y(field: &TextField, opts: &FieldA11yOpts) -> FieldA11y {
    let (selection_start, selection_end) = field.utf16_selection();
    let hidden = opts.readonly;
    FieldA11y {
        role: if hidden {
            String::new()
        } else {
            "textbox".to_string()
        },
        label: opts.label.clone(),
        value: field.text().to_string(),
        selection_start,
        selection_end,
        readonly: opts.readonly,
        multiline: opts.multiline,
        hidden,
    }
}

/// DOM / shadow-host attributes for the focusable textbox (#69).
///
/// Returns an empty map when the field is hidden (read-only viewer).
pub fn textbox_attrs(a11y: &FieldA11y) -> Vec<(String, String)> {
    if a11y.hidden || a11y.role.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    out.push(("role".to_string(), a11y.role.clone()));
    out.push(("aria-label".to_string(), a11y.label.clone()));
    out.push((
        "aria-multiline".to_string(),
        if a11y.multiline { "true" } else { "false" }.to_string(),
    ));
    if a11y.readonly {
        out.push(("aria-readonly".to_string(), "true".to_string()));
    }
    out.push(("data-a11y-value".to_string(), a11y.value.clone()));
    out.push((
        "data-a11y-selection-start".to_string(),
        a11y.selection_start.to_string(),
    ));
    out.push((
        "data-a11y-selection-end".to_string(),
        a11y.selection_end.to_string(),
    ));
    out
}

/// Polite status string for `.xg-live` / `aria-live=polite`. Empty when there is
/// nothing assistive tech should repeat (read-only, or redundant caret ping).
pub fn announce(field: &TextField, kind: AnnounceKind, opts: &FieldA11yOpts) -> String {
    if opts.readonly {
        return String::new();
    }
    let (anchor, focus) = field.utf16_selection();
    match kind {
        AnnounceKind::Caret => {
            if anchor != focus {
                return String::new();
            }
            let pos = focus;
            if field.text().is_empty() && pos == 0 {
                return String::new();
            }
            format!("Caret at character {}", pos + 1)
        }
        AnnounceKind::Selection => {
            if anchor == focus {
                return String::new();
            }
            let start = anchor.min(focus);
            let end = anchor.max(focus);
            let selected = field.selected_text();
            if selected.is_empty() {
                format!("Selected characters {} to {}", start + 1, end)
            } else {
                format!("Selected {}", selected)
            }
        }
        AnnounceKind::Edit => {
            let text = field.text();
            if text.is_empty() {
                return String::new();
            }
            text.to_string()
        }
    }
}

pub fn eval_a11y_script(script_json: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(script_json).map_err(|e| format!("bad script json: {e}"))?;
    let label = v
        .get("label")
        .and_then(|x| x.as_str())
        .unwrap_or(LABEL_FORMULA_BAR);
    let readonly = v.get("readonly").and_then(|x| x.as_bool()).unwrap_or(false);
    let multiline = v.get("multiline").and_then(|x| x.as_bool()).unwrap_or(true);
    let text = v
        .get("text")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let anchor_u16 = v.get("anchor").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let focus_u16 = v.get("focus").and_then(|x| x.as_u64()).unwrap_or(0) as usize;

    let opts = FieldA11yOpts {
        label: label.to_string(),
        readonly,
        multiline,
    };

    let mut field = TextField::from_utf16_selection(&text, anchor_u16, focus_u16);
    let ops = v
        .get("ops")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    let mut live = String::new();
    for op in ops {
        if opts.readonly {
            continue;
        }
        let kind = op.get("op").and_then(|x| x.as_str()).unwrap_or("");
        if kind == "insert" {
            let t = op.get("text").and_then(|x| x.as_str()).unwrap_or("");
            field.insert(t);
            live = announce(&field, AnnounceKind::Edit, &opts);
        } else if kind == "select" {
            let a = op.get("anchor").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
            let f = op.get("focus").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
            field = TextField::from_utf16_selection(field.text(), a, f);
            live = announce(&field, AnnounceKind::Selection, &opts);
        } else if kind == "caret" {
            let f = op.get("focus").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
            field = TextField::from_utf16_selection(field.text(), f, f);
            live = announce(&field, AnnounceKind::Caret, &opts);
        }
    }

    let textbox = field_a11y(&field, &opts);
    let attrs = textbox_attrs(&textbox);
    let out = serde_json::json!({
        "textbox": {
            "role": textbox.role,
            "label": textbox.label,
            "value": textbox.value,
            "selection_start": textbox.selection_start,
            "selection_end": textbox.selection_end,
            "readonly": textbox.readonly,
            "multiline": textbox.multiline,
            "hidden": textbox.hidden,
        },
        "attrs": attrs,
        "announce_caret": announce(&field, AnnounceKind::Caret, &opts),
        "announce_selection": announce(&field, AnnounceKind::Selection, &opts),
        "announce_edit": announce(&field, AnnounceKind::Edit, &opts),
        "live": live,
    });
    Ok(out.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::selection::TextField;

    #[test]
    fn catches_readonly_hides_textbox_and_edit_announce() {
        // Catches: read-only viewer must not expose an edit textbox or edit live strings.
        let field = TextField::new("secret");
        let opts = FieldA11yOpts::formula_bar(true);
        let a11y = field_a11y(&field, &opts);
        assert!(a11y.hidden);
        assert!(a11y.role.is_empty());
        assert!(textbox_attrs(&a11y).is_empty());
        assert_eq!(announce(&field, AnnounceKind::Edit, &opts), "");
    }

    #[test]
    fn catches_insert_edit_announces_value_like_upstream_live() {
        // Catches: after insert the polite live region must carry the full value (formula bar parity).
        let mut field = TextField::new("=");
        let opts = FieldA11yOpts::formula_bar(false);
        field.insert("A1");
        let msg = announce(&field, AnnounceKind::Edit, &opts);
        assert_eq!(msg, "=A1");
        let a11y = field_a11y(&field, &opts);
        assert_eq!(a11y.role, "textbox");
        assert_eq!(a11y.label, LABEL_FORMULA_BAR);
        assert_eq!(a11y.selection_start, a11y.selection_end);
        assert_eq!(a11y.selection_end, "=A1".encode_utf16().count());
    }

    #[test]
    fn catches_selection_announce_non_empty_for_range() {
        // Catches: shift-selection must produce a non-empty selection announcement.
        let field = TextField::from_utf16_selection("abcd", 1, 3);
        let opts = FieldA11yOpts::cell_editor(false);
        let msg = announce(&field, AnnounceKind::Selection, &opts);
        assert_eq!(msg, "Selected bc");
    }
}
