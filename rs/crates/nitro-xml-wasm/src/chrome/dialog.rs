// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/dialog/dialog.ts and
// src/ui/src/xlsx/dialogs/frame.ts (modal shell + OK/Cancel footer); changes listed in PROVENANCE.md.
//! Modal / non-modal dialog chrome HTML and close reasons (#54).
//!
//! Concrete dialog bodies (paste-special, format-cells, …) stay TODO for later
//! command issues; this module owns the frame, focus trap hooks and restore.

use super::focus::{should_prevent_tab, trap_tab, TrapTab};

/// Why a dialog closed (mirrors `OfficeDialogCloseReason`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogCloseReason {
    Escape,
    CloseButton,
    Backdrop,
    Ok,
    Cancel,
    Api,
}

impl DialogCloseReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Escape => "escape",
            Self::CloseButton => "close-button",
            Self::Backdrop => "backdrop",
            Self::Ok => "ok",
            Self::Cancel => "cancel",
            Self::Api => "api",
        }
    }
}

/// Spec for a chrome dialog frame (heading + body HTML + footer).
#[derive(Clone, Debug)]
pub struct DialogSpec {
    pub name: String,
    pub heading: String,
    pub body_html: String,
    pub ok_label: Option<String>,
    pub cancel_label: String,
    pub wide: bool,
    pub modal: bool,
    pub dismissible: bool,
}

impl DialogSpec {
    pub fn simple(name: &str, heading: &str, body_html: &str) -> Self {
        Self {
            name: name.to_string(),
            heading: heading.to_string(),
            body_html: body_html.to_string(),
            ok_label: Some("OK".into()),
            cancel_label: "Cancel".into(),
            wide: false,
            modal: true,
            dismissible: true,
        }
    }
}

/// Open dialog state held by [`super::EditorChrome`].
#[derive(Clone, Debug)]
pub struct OpenDialog {
    pub spec: DialogSpec,
    /// Index of the initially focused control (0 = first footer/body control).
    pub focus_index: usize,
    pub focusable_count: usize,
}

impl OpenDialog {
    pub fn new(spec: DialogSpec) -> Self {
        // Close + Cancel + optional OK → at least 2 focusables in the frame.
        let focusable_count = 1 + 1 + usize::from(spec.ok_label.is_some());
        Self {
            spec,
            focus_index: 0,
            focusable_count,
        }
    }

    pub fn on_tab(&mut self, shift: bool) -> TrapTab {
        let next = trap_tab(Some(self.focus_index), self.focusable_count, shift);
        if let TrapTab::MoveTo(i) = next {
            self.focus_index = i;
        }
        next
    }

    pub fn should_prevent_tab(&self, shift: bool) -> bool {
        should_prevent_tab(Some(self.focus_index), self.focusable_count, shift)
    }

    /// Whether `reason` may close this dialog.
    pub fn accepts_close(&self, reason: DialogCloseReason) -> bool {
        match reason {
            DialogCloseReason::Escape | DialogCloseReason::Backdrop => self.spec.dismissible,
            _ => true,
        }
    }
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

/// Render the open dialog as trusted HTML for the loader host.
pub fn dialog_html(open: &OpenDialog) -> String {
    let spec = &open.spec;
    let modal = if spec.modal { "true" } else { "false" };
    let wide = if spec.wide {
        " xve-cmd-dialog-wide"
    } else {
        ""
    };
    let dismiss = if spec.dismissible { "true" } else { "false" };
    let mut footer = String::new();
    footer.push_str(&format!(
        "<button type=\"button\" class=\"xve-btn\" data-a=\"dialog-cancel\" data-dialog-focus=\"1\">{}</button>",
        esc(&spec.cancel_label)
    ));
    if let Some(ok) = &spec.ok_label {
        footer.push_str(&format!(
            "<button type=\"button\" class=\"xve-btn xve-btn-primary\" data-a=\"dialog-ok\" data-dialog-focus=\"2\">{}</button>",
            esc(ok)
        ));
    }
    format!(
        "<div class=\"xve-dialog-root\" data-dialog-host=\"1\" data-dialog=\"{name}\">\
           <div class=\"xve-dialog-backdrop\" data-a=\"dialog-backdrop\" aria-hidden=\"true\"></div>\
           <div class=\"xve-cmd-dialog{wide} box\" role=\"dialog\" aria-modal=\"{modal}\" \
                aria-labelledby=\"xve-dialog-title\" tabindex=\"-1\" data-dismissible=\"{dismiss}\" \
                data-dialog-focus=\"0\">\
             <header>\
               <h2 id=\"xve-dialog-title\">{heading}</h2>\
               <button type=\"button\" class=\"close\" data-a=\"dialog-close\" aria-label=\"Close\" data-dialog-focus=\"0\">×</button>\
             </header>\
             <div class=\"xve-dialog-body\">{body}</div>\
             <div class=\"xve-dialog-footer\" slot=\"footer\">{footer}</div>\
           </div>\
         </div>",
        name = esc(&spec.name),
        heading = esc(&spec.heading),
        body = spec.body_html,
        footer = footer,
        wide = wide,
        modal = modal,
        dismiss = dismiss,
    )
}

#[cfg(test)]
mod tests {
    use super::super::focus::{FocusRestore, FocusStack};
    use super::*;

    #[test]
    fn catches_non_dismissible_ignores_escape_and_backdrop() {
        // Catches: dismissible=false must keep the dialog open on Escape/backdrop.
        let mut spec = DialogSpec::simple("opts", "Options", "<p>body</p>");
        spec.dismissible = false;
        let open = OpenDialog::new(spec);
        let mut focus = FocusStack::new();
        focus.push("opener");
        assert!(!open.accepts_close(DialogCloseReason::Escape));
        assert!(!open.accepts_close(DialogCloseReason::Backdrop));
        assert!(open.accepts_close(DialogCloseReason::Ok));
        assert!(!focus.is_empty());
        let _ = focus.pop();
        assert!(focus.is_empty());
        let _ = FocusRestore {
            target_id: "opener".into(),
        };
    }

    #[test]
    fn catches_dialog_html_exposes_aria_modal_and_labelledby() {
        // Catches: dialog frame must expose role=dialog, aria-modal, and labelled heading.
        let open = OpenDialog::new(DialogSpec::simple("confirm", "Confirm?", "<p>Sure?</p>"));
        let html = dialog_html(&open);
        assert!(html.contains("role=\"dialog\""));
        assert!(html.contains("aria-modal=\"true\""));
        assert!(html.contains("aria-labelledby=\"xve-dialog-title\""));
        assert!(html.contains("Confirm?"));
        assert!(html.contains("data-dialog=\"confirm\""));
    }
}
