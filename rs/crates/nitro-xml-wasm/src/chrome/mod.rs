// SPDX-License-Identifier: MIT
//! Xlsx editor chrome: command registry, ribbon, shortcuts (#53), dialogs/menus/focus (#54).

#[allow(dead_code)]
mod commands;
mod dialog;
mod focus;
mod menu;
mod ribbon;
mod ribbon_icons;
mod shortcuts;

pub use commands::ChromeCommands;
pub use dialog::{dialog_html, DialogCloseReason, DialogSpec, OpenDialog};
pub use menu::{menu_html, stub_cell_context_items, MenuCloseReason, MenuItem, OpenMenu};
pub use shortcuts::{find_shortcut, KeyEventLike, SHORTCUTS};

/// Viewport + restore target for opening a menu / context menu.
pub struct MenuOpenOpts<'a> {
    pub label: &'a str,
    pub x: f32,
    pub y: f32,
    pub restore_focus_id: &'a str,
    pub viewport_w: f32,
    pub viewport_h: f32,
}

use wasm_bindgen::prelude::*;

/// Open overlay on top of the ribbon / shell.
#[derive(Clone, Debug)]
enum Overlay {
    None,
    Dialog(OpenDialog),
    Menu(OpenMenu),
}

/// Lazy-loaded editor chrome state shared by ribbon HTML and command dispatch.
pub struct EditorChrome {
    pub commands: ChromeCommands,
    active_tab: String,
    read_only: bool,
    loaded: bool,
    overlay: Overlay,
    focus: focus::FocusStack,
    /// Formula bar fragment. Empty while read-only (#69 negative control).
    formula_html: String,
}

impl EditorChrome {
    pub fn new() -> Self {
        Self {
            commands: ChromeCommands::new(),
            active_tab: "home".to_string(),
            read_only: true,
            loaded: false,
            overlay: Overlay::None,
            focus: focus::FocusStack::new(),
            formula_html: String::new(),
        }
    }

    pub fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.commands.register_chrome_stubs();
        self.loaded = true;
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.commands.set_read_only(read_only);
        if read_only {
            self.overlay = Overlay::None;
            self.focus.clear();
            self.formula_html.clear();
        }
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn loaded(&self) -> bool {
        self.loaded
    }

    pub fn active_tab(&self) -> &str {
        &self.active_tab
    }

    pub fn ribbon_html(&self) -> String {
        ribbon::ribbon_html(self)
    }

    /// Dialog / menu overlay HTML (empty when readonly or nothing open).
    pub fn overlay_html(&self) -> String {
        if self.read_only || !self.loaded {
            return String::new();
        }
        match &self.overlay {
            Overlay::None => String::new(),
            Overlay::Dialog(d) => dialog_html(d),
            Overlay::Menu(m) => menu_html(m),
        }
    }

    pub fn has_overlay(&self) -> bool {
        !matches!(self.overlay, Overlay::None)
    }

    pub fn formula_html(&self) -> &str {
        if self.read_only || !self.loaded {
            ""
        } else {
            &self.formula_html
        }
    }

    pub fn set_formula_html(&mut self, html: String) {
        if self.read_only || !self.loaded {
            self.formula_html.clear();
            return;
        }
        self.formula_html = html;
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.commands.is_enabled(id)
    }

    pub fn is_checked(&self, id: &str) -> bool {
        self.commands.is_checked(id)
    }

    pub fn dispatch_command(&mut self, id: &str, host: &mut dyn std::any::Any) -> bool {
        match self.commands.dispatch(id, host) {
            Ok(()) => true,
            Err(_) => false,
        }
    }

    pub fn match_shortcut(
        &self,
        event: &KeyEventLike<'_>,
        mac: bool,
    ) -> Option<shortcut::ShortcutMatch> {
        if !self.loaded || self.read_only {
            return None;
        }
        find_shortcut(event, mac).map(|s| shortcut::ShortcutMatch {
            command: s.command,
            action: s.action,
        })
    }

    /// Open a modal dialog. Returns false when readonly / not loaded.
    pub fn open_dialog(&mut self, spec: DialogSpec, restore_focus_id: &str) -> bool {
        if self.read_only || !self.loaded {
            return false;
        }
        self.overlay = Overlay::None;
        self.focus.clear();
        self.focus.push(restore_focus_id);
        self.overlay = Overlay::Dialog(OpenDialog::new(spec));
        true
    }

    /// Open a context menu at viewport (x, y). Returns false when readonly / not loaded.
    pub fn open_context_menu(&mut self, opts: MenuOpenOpts<'_>, items: Vec<MenuItem>) -> bool {
        if self.read_only || !self.loaded {
            return false;
        }
        self.overlay = Overlay::None;
        self.focus.clear();
        self.focus.push(opts.restore_focus_id);
        let mut menu = OpenMenu::context(opts.label, opts.x, opts.y, items);
        menu.clamp_to_viewport(
            180.0,
            8.0 + menu.items.len() as f32 * 28.0,
            opts.viewport_w,
            opts.viewport_h,
        );
        self.overlay = Overlay::Menu(menu);
        true
    }

    /// Open an anchored (ribbon) menu. Returns false when readonly / not loaded.
    pub fn open_anchored_menu(&mut self, opts: MenuOpenOpts<'_>, items: Vec<MenuItem>) -> bool {
        if self.read_only || !self.loaded {
            return false;
        }
        self.overlay = Overlay::None;
        self.focus.clear();
        self.focus.push(opts.restore_focus_id);
        let mut menu = OpenMenu::anchored(opts.label, opts.x, opts.y, items);
        menu.clamp_to_viewport(
            180.0,
            8.0 + menu.items.len() as f32 * 28.0,
            opts.viewport_w,
            opts.viewport_h,
        );
        self.overlay = Overlay::Menu(menu);
        true
    }

    /// Handle Tab inside an open dialog (focus trap). Returns whether the host should preventDefault.
    pub fn overlay_tab(&mut self, shift: bool) -> bool {
        if matches!(self.overlay, Overlay::Menu(_)) {
            // Tab closes menus (upstream office-menu-close reason `tab`).
            let _ = self.close_overlay("tab");
            return true;
        }
        match &mut self.overlay {
            Overlay::Dialog(d) => {
                let prevent = d.should_prevent_tab(shift);
                let _ = d.on_tab(shift);
                prevent
            }
            _ => false,
        }
    }

    /// Arrow / type-ahead navigation inside an open menu.
    pub fn overlay_menu_key(&mut self, key: &str) -> bool {
        if key == "Escape" && matches!(self.overlay, Overlay::Menu(_)) {
            let _ = self.close_overlay("escape");
            return true;
        }
        match &mut self.overlay {
            Overlay::Menu(m) => match key {
                "ArrowDown" => {
                    m.move_active(1);
                    true
                }
                "ArrowUp" => {
                    m.move_active(-1);
                    true
                }
                s if s.len() == 1 => {
                    m.type_ahead(s);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    pub fn focus_restore_peek(&self) -> Option<&str> {
        self.focus.peek().map(|f| f.target_id.as_str())
    }

    /// Close the current overlay. `reason` is a dialog or menu close reason string.
    /// Returns the restore-focus id when focus should return to the opener.
    pub fn close_overlay(&mut self, reason: &str) -> Option<String> {
        let before = self.focus.peek().map(|f| f.target_id.clone());
        match &self.overlay {
            Overlay::None => None,
            Overlay::Dialog(d) => {
                let r = parse_dialog_reason(reason);
                if !d.accepts_close(r) {
                    return None;
                }
                let _ = self.focus.pop();
                self.overlay = Overlay::None;
                before
            }
            Overlay::Menu(_) => {
                let r = parse_menu_reason(reason);
                let _ = self.focus.pop();
                self.overlay = Overlay::None;
                if r.restores_focus() {
                    before
                } else {
                    None
                }
            }
        }
    }
}

fn parse_dialog_reason(reason: &str) -> DialogCloseReason {
    match reason {
        "escape" => DialogCloseReason::Escape,
        "close-button" => DialogCloseReason::CloseButton,
        "backdrop" => DialogCloseReason::Backdrop,
        "ok" => DialogCloseReason::Ok,
        "cancel" => DialogCloseReason::Cancel,
        _ => DialogCloseReason::Api,
    }
}

fn parse_menu_reason(reason: &str) -> MenuCloseReason {
    match reason {
        "escape" => MenuCloseReason::Escape,
        "outside" => MenuCloseReason::Outside,
        "tab" => MenuCloseReason::Tab,
        "item" => MenuCloseReason::Item,
        _ => MenuCloseReason::Api,
    }
}

pub mod shortcut {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ShortcutMatch {
        pub command: Option<&'static str>,
        pub action: Option<&'static str>,
    }
}

/// Headless overlay eval for chrome_gate / browser negative (#54).
#[wasm_bindgen(js_name = chromeOverlayEval)]
pub fn chrome_overlay_eval(script_json: &str) -> Result<String, JsValue> {
    let v: serde_json::Value = serde_json::from_str(script_json)
        .map_err(|e| JsValue::from_str(&format!("bad script: {e}")))?;
    let read_only = v.get("readOnly").and_then(|x| x.as_bool()).unwrap_or(true);
    let mut chrome = EditorChrome::new();
    if !read_only {
        chrome.load();
        chrome.set_read_only(false);
    }
    let op = v.get("op").and_then(|x| x.as_str()).unwrap_or("");
    let mut restore = None;
    match op {
        "openDialog" => {
            let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("confirm");
            let heading = v
                .get("heading")
                .and_then(|x| x.as_str())
                .unwrap_or("Confirm");
            let body = v
                .get("body")
                .and_then(|x| x.as_str())
                .unwrap_or("<p>Body</p>");
            let ok = chrome.open_dialog(
                DialogSpec::simple(name, heading, body),
                v.get("restore")
                    .and_then(|x| x.as_str())
                    .unwrap_or("grid-sink"),
            );
            let _ = ok;
        }
        "openContextMenu" => {
            let opts = MenuOpenOpts {
                label: "Context menu",
                x: v.get("x").and_then(|x| x.as_f64()).unwrap_or(40.0) as f32,
                y: v.get("y").and_then(|x| x.as_f64()).unwrap_or(40.0) as f32,
                restore_focus_id: v
                    .get("restore")
                    .and_then(|x| x.as_str())
                    .unwrap_or("grid-sink"),
                viewport_w: v.get("vw").and_then(|x| x.as_f64()).unwrap_or(960.0) as f32,
                viewport_h: v.get("vh").and_then(|x| x.as_f64()).unwrap_or(600.0) as f32,
            };
            let _ = chrome.open_context_menu(opts, stub_cell_context_items());
        }
        "openAnchoredMenu" => {
            let opts = MenuOpenOpts {
                label: v.get("label").and_then(|x| x.as_str()).unwrap_or("Paste"),
                x: v.get("x").and_then(|x| x.as_f64()).unwrap_or(40.0) as f32,
                y: v.get("y").and_then(|x| x.as_f64()).unwrap_or(40.0) as f32,
                restore_focus_id: v
                    .get("restore")
                    .and_then(|x| x.as_str())
                    .unwrap_or("home.paste"),
                viewport_w: v.get("vw").and_then(|x| x.as_f64()).unwrap_or(375.0) as f32,
                viewport_h: v.get("vh").and_then(|x| x.as_f64()).unwrap_or(667.0) as f32,
            };
            let items = vec![
                MenuItem::cmd("edit.paste", "Paste"),
                MenuItem::cmd("edit.paste-special", "Paste Special").disabled(),
                MenuItem::cmd("edit.paste-values", "Paste Values"),
            ];
            let _ = chrome.open_anchored_menu(opts, items);
        }
        "menuKey" => {
            let key = v.get("key").and_then(|x| x.as_str()).unwrap_or("ArrowDown");
            let _ = chrome.overlay_menu_key(key);
        }
        "dialogTab" => {
            let shift = v.get("shift").and_then(|x| x.as_bool()).unwrap_or(false);
            let _ = chrome.overlay_tab(shift);
        }
        "close" => {
            restore =
                chrome.close_overlay(v.get("reason").and_then(|x| x.as_str()).unwrap_or("escape"));
        }
        _ => {}
    }
    let html = format!("{}{}", chrome.ribbon_html(), chrome.overlay_html());
    let active = match &chrome.overlay {
        Overlay::Menu(m) => Some(m.active_index),
        Overlay::Dialog(d) => Some(d.focus_index as isize),
        Overlay::None => None,
    };
    let _ = (
        chrome.focus_restore_peek(),
        chrome.focus.is_empty(),
        DialogCloseReason::Api.as_str(),
        MenuCloseReason::Api.as_str(),
    );
    Ok(serde_json::json!({
        "ok": true,
        "readOnly": chrome.read_only(),
        "loaded": chrome.loaded(),
        "hasOverlay": chrome.has_overlay(),
        "hasDialog": html.contains("role=\"dialog\""),
        "hasMenu": html.contains("role=\"menu\""),
        "hasRibbon": html.contains("aria-label=\"Ribbon\""),
        "activeIndex": active,
        "restore": restore,
        "html": html,
    })
    .to_string())
}

#[cfg(test)]
mod overlay_tests {
    use super::*;

    #[test]
    fn catches_readonly_hides_dialog_and_menu_overlays() {
        // Catches: readonly must not open dialog/menu chrome (negative control #54).
        let mut chrome = EditorChrome::new();
        chrome.load();
        // still readonly by default
        assert!(!chrome.open_dialog(DialogSpec::simple("x", "X", "<p>"), "g"));
        assert!(!chrome.open_context_menu(
            MenuOpenOpts {
                label: "m",
                x: 1.0,
                y: 1.0,
                restore_focus_id: "g",
                viewport_w: 400.0,
                viewport_h: 300.0,
            },
            stub_cell_context_items(),
        ));
        assert!(chrome.overlay_html().is_empty());

        chrome.set_read_only(false);
        assert!(chrome.open_dialog(DialogSpec::simple("x", "X", "<p>"), "g"));
        assert!(chrome.overlay_html().contains("role=\"dialog\""));
        // Tab from last focusable must cycle (preventDefault).
        let _ = chrome.overlay_tab(false);
        let _ = chrome.overlay_tab(false);
        assert!(chrome.overlay_tab(false));
        chrome.set_read_only(true);
        assert!(chrome.overlay_html().is_empty());
        assert!(!chrome.has_overlay());
    }

    #[test]
    fn catches_escape_restores_opener_focus_token() {
        // Catches: Escape on a dialog must return the restore-focus opener id.
        let mut chrome = EditorChrome::new();
        chrome.load();
        chrome.set_read_only(false);
        assert!(chrome.open_dialog(DialogSpec::simple("c", "C", "<p>"), "grid-sink"));
        let restored = chrome.close_overlay("escape");
        assert_eq!(restored.as_deref(), Some("grid-sink"));
        assert!(!chrome.has_overlay());
    }
}
