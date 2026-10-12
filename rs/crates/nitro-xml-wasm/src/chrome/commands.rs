// SPDX-License-Identifier: MIT
//! WASM-facing command registry facade for editor chrome (#53).
//!
//! Runners stay stubs until #66+; ribbon and shortcuts share ids with upstream.

use std::any::Any;
use std::collections::BTreeMap;

use nitro_xml::opc::edit::{Command, CommandRegistry, EditError};
use serde_json::Value;

/// Chrome-layer metadata (upstream `Command` fields not stored on opc `Command`).
#[derive(Debug, Clone)]
pub struct CommandMeta {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub editing: bool,
}

macro_rules! chrome_cmd {
    ($id:expr, $label:expr, $shortcut:expr, $editing:expr) => {
        CommandMeta {
            id: $id,
            label: $label,
            shortcut: $shortcut,
            editing: $editing,
        }
    };
}

/// Every Home ribbon control + shortcut command id with display metadata.
pub const CHROME_COMMANDS: &[CommandMeta] = &[
    chrome_cmd!("home.paste", "Paste", Some("Ctrl+V"), true),
    chrome_cmd!("home.paste-values", "Paste Values", None, true),
    chrome_cmd!("home.paste-formulas", "Paste Formulas", None, true),
    chrome_cmd!("home.paste-formatting", "Paste Formatting", None, true),
    chrome_cmd!("home.paste-transpose", "Transpose", None, true),
    chrome_cmd!(
        "home.paste-special",
        "Paste Special...",
        Some("Ctrl+Alt+V"),
        true
    ),
    chrome_cmd!("home.cut", "Cut", Some("Ctrl+X"), true),
    chrome_cmd!("home.copy", "Copy", Some("Ctrl+C"), false),
    chrome_cmd!("home.format-painter", "Format Painter", None, true),
    chrome_cmd!("home.font-name", "Font", None, true),
    chrome_cmd!("home.font-size", "Font Size", None, true),
    chrome_cmd!(
        "home.grow-font",
        "Increase Font Size",
        Some("Ctrl+Shift+>"),
        true
    ),
    chrome_cmd!(
        "home.shrink-font",
        "Decrease Font Size",
        Some("Ctrl+Shift+<"),
        true
    ),
    chrome_cmd!("home.bold", "Bold", Some("Ctrl+B"), true),
    chrome_cmd!("home.italic", "Italic", Some("Ctrl+I"), true),
    chrome_cmd!("home.underline", "Underline", Some("Ctrl+U"), true),
    chrome_cmd!("home.underline-double", "Double Underline", None, true),
    chrome_cmd!("home.strikethrough", "Strikethrough", Some("Ctrl+5"), true),
    chrome_cmd!("home.borders", "Borders", None, true),
    chrome_cmd!("home.borders-more", "More Borders...", None, true),
    chrome_cmd!("home.fill-color", "Fill Color", None, true),
    chrome_cmd!("home.font-color", "Font Color", None, true),
    chrome_cmd!("home.font-settings", "Font Settings", None, true),
    chrome_cmd!("home.align-top", "Top Align", None, true),
    chrome_cmd!("home.align-middle", "Middle Align", None, true),
    chrome_cmd!("home.align-bottom", "Bottom Align", None, true),
    chrome_cmd!("home.orientation", "Orientation", None, true),
    chrome_cmd!(
        "home.alignment-settings",
        "Format Cell Alignment",
        None,
        true
    ),
    chrome_cmd!("home.wrap-text", "Wrap Text", None, true),
    chrome_cmd!("home.align-left", "Align Left", None, true),
    chrome_cmd!("home.align-center", "Center", None, true),
    chrome_cmd!("home.align-right", "Align Right", None, true),
    chrome_cmd!("home.indent-decrease", "Decrease Indent", None, true),
    chrome_cmd!("home.indent-increase", "Increase Indent", None, true),
    chrome_cmd!("home.merge-center", "Merge & Center", None, true),
    chrome_cmd!("home.merge-across", "Merge Across", None, true),
    chrome_cmd!("home.merge-cells", "Merge Cells", None, true),
    chrome_cmd!("home.unmerge", "Unmerge Cells", None, true),
    chrome_cmd!("home.number-format", "Number Format", None, true),
    chrome_cmd!("home.number-settings", "Number Format Settings", None, true),
    chrome_cmd!("home.accounting", "Accounting Number Format", None, true),
    chrome_cmd!(
        "home.accounting-more",
        "More Accounting Formats...",
        None,
        true
    ),
    chrome_cmd!("home.percent", "Percent Style", None, true),
    chrome_cmd!("home.comma", "Comma Style", None, true),
    chrome_cmd!("home.decimal-increase", "Increase Decimal", None, true),
    chrome_cmd!("home.decimal-decrease", "Decrease Decimal", None, true),
    chrome_cmd!("home.cf-quick", "Conditional Formatting", None, true),
    chrome_cmd!("home.cf-data-bar", "Data Bars", None, true),
    chrome_cmd!("home.cf-color-scale", "Color Scales", None, true),
    chrome_cmd!("home.cf-icon-set", "Icon Sets", None, true),
    chrome_cmd!("home.cf-new-rule", "New Rule...", None, true),
    chrome_cmd!(
        "home.cf-clear-selection",
        "Clear Rules from Selected Cells",
        None,
        true
    ),
    chrome_cmd!(
        "home.cf-clear-sheet",
        "Clear Rules from Entire Sheet",
        None,
        true
    ),
    chrome_cmd!("home.cf-manage", "Manage Rules...", None, true),
    chrome_cmd!("home.format-as-table", "Format as Table", None, true),
    chrome_cmd!("home.cell-styles", "Cell Styles", None, true),
    chrome_cmd!("cells.insert-split", "Insert", None, true),
    chrome_cmd!("cells.delete-split", "Delete", None, true),
    chrome_cmd!(
        "cells.insert",
        "Insert Cells...",
        Some("Ctrl+Shift++"),
        true
    ),
    chrome_cmd!("cells.delete", "Delete Cells...", Some("Ctrl+-"), true),
    chrome_cmd!("cells.insert-rows", "Insert Sheet Rows", None, true),
    chrome_cmd!("cells.delete-rows", "Delete Sheet Rows", None, true),
    chrome_cmd!("cells.insert-columns", "Insert Sheet Columns", None, true),
    chrome_cmd!("cells.delete-columns", "Delete Sheet Columns", None, true),
    chrome_cmd!("sheet.insert", "Insert Sheet", None, true),
    chrome_cmd!("sheet.delete", "Delete Sheet", None, true),
    chrome_cmd!("format.row-height", "Row Height...", None, true),
    chrome_cmd!("format.autofit-rows", "AutoFit Row Height", None, true),
    chrome_cmd!("format.column-width", "Column Width...", None, true),
    chrome_cmd!("format.autofit-columns", "AutoFit Column Width", None, true),
    chrome_cmd!("format.default-width", "Default Width...", None, true),
    chrome_cmd!("format.hide-rows", "Hide Rows", None, true),
    chrome_cmd!("format.hide-columns", "Hide Columns", None, true),
    chrome_cmd!("sheet.hide", "Hide Sheet", None, true),
    chrome_cmd!("format.unhide-rows", "Unhide Rows", None, true),
    chrome_cmd!("format.unhide-columns", "Unhide Columns", None, true),
    chrome_cmd!("sheet.unhide", "Unhide Sheet...", None, true),
    chrome_cmd!("sheet.rename", "Rename Sheet", None, true),
    chrome_cmd!("sheet.move-copy", "Move or Copy Sheet...", None, true),
    chrome_cmd!("sheet.tab-color", "Tab Color", None, true),
    chrome_cmd!("review.protect-sheet", "Protect Sheet", None, true),
    chrome_cmd!("format.lock-cell", "Lock Cell", None, true),
    chrome_cmd!("format.cells", "Format Cells...", Some("Ctrl+1"), true),
    chrome_cmd!("home.autosum", "AutoSum", Some("Alt+="), true),
    chrome_cmd!("home.autosum-average", "Average", None, true),
    chrome_cmd!("home.autosum-count", "Count Numbers", None, true),
    chrome_cmd!("home.autosum-max", "Max", None, true),
    chrome_cmd!("home.autosum-min", "Min", None, true),
    chrome_cmd!("home.autosum-more", "More Functions...", None, true),
    chrome_cmd!("home.fill-down", "Down", Some("Ctrl+D"), true),
    chrome_cmd!("home.fill-right", "Right", Some("Ctrl+R"), true),
    chrome_cmd!("home.fill-up", "Up", None, true),
    chrome_cmd!("home.fill-left", "Left", None, true),
    chrome_cmd!("home.fill-series", "Series...", None, true),
    chrome_cmd!("home.clear-all", "Clear All", None, true),
    chrome_cmd!("home.clear-formats", "Clear Formats", None, true),
    chrome_cmd!("home.clear-contents", "Clear Contents", None, true),
    chrome_cmd!(
        "home.clear-comments",
        "Clear Comments and Notes",
        None,
        true
    ),
    chrome_cmd!("home.clear-hyperlinks", "Clear Hyperlinks", None, true),
    chrome_cmd!("data.sort-asc", "Sort A to Z", None, true),
    chrome_cmd!("data.sort-desc", "Sort Z to A", None, true),
    chrome_cmd!("data.sort-custom", "Custom Sort...", None, true),
    chrome_cmd!("data.filter", "Filter", Some("Ctrl+Shift+L"), true),
    chrome_cmd!("data.filter-clear", "Clear", None, true),
    chrome_cmd!("data.filter-reapply", "Reapply", None, true),
    chrome_cmd!("home.find", "Find...", Some("Ctrl+F"), false),
    chrome_cmd!("home.replace", "Replace...", Some("Ctrl+H"), true),
    chrome_cmd!("home.go-to", "Go To...", Some("Ctrl+G"), false),
    chrome_cmd!("home.go-to-special", "Go To Special...", None, false),
    chrome_cmd!("home.select-formulas", "Formulas", None, false),
    chrome_cmd!("home.select-comments", "Notes", None, false),
    chrome_cmd!(
        "home.select-conditional",
        "Conditional Formatting",
        None,
        false
    ),
    chrome_cmd!("home.select-constants", "Constants", None, false),
    chrome_cmd!("home.select-validation", "Data Validation", None, false),
    // Shortcut-only / file / edit ids
    chrome_cmd!("file.save", "Save", Some("Ctrl+S"), true),
    chrome_cmd!("file.open", "Open", Some("Ctrl+O"), true),
    chrome_cmd!("file.new", "New workbook", Some("Ctrl+N"), true),
    chrome_cmd!("file.print", "Print", Some("Ctrl+P"), true),
    chrome_cmd!("edit.undo", "Undo", Some("Ctrl+Z"), true),
    chrome_cmd!("edit.redo", "Redo", Some("Ctrl+Y"), true),
    chrome_cmd!("home.format-general", "General number format", None, true),
    chrome_cmd!("home.format-currency", "Currency format", None, true),
    chrome_cmd!("home.format-percent", "Percentage format", None, true),
    chrome_cmd!("home.format-scientific", "Scientific format", None, true),
    chrome_cmd!("home.format-date", "Date format", None, true),
    chrome_cmd!("home.format-time", "Time format", None, true),
    chrome_cmd!("home.format-number", "Number format", None, true),
    chrome_cmd!("edit.insert-date", "Insert the current date", None, true),
    chrome_cmd!("edit.insert-time", "Insert the current time", None, true),
    chrome_cmd!("formulas.show-formulas", "Show Formulas", None, true),
    chrome_cmd!("edit.select-column", "Select the entire column", None, true),
    chrome_cmd!("edit.select-row", "Select the entire row", None, true),
    chrome_cmd!(
        "sheet.previous",
        "Previous sheet",
        Some("Ctrl+PageUp"),
        false
    ),
    chrome_cmd!("sheet.next", "Next sheet", Some("Ctrl+PageDown"), false),
    chrome_cmd!("insert.table", "Create a table", None, true),
    chrome_cmd!("insert.link", "Insert link", Some("Ctrl+K"), true),
    chrome_cmd!("review.new-comment", "New comment", None, true),
    chrome_cmd!("formulas.calculate-now", "Calculate Now", None, true),
];

/// Chrome command facade: opc registry + ribbon-action tracking + metadata map.
pub struct ChromeCommands {
    registry: CommandRegistry,
    meta: BTreeMap<String, CommandMeta>,
    read_only: bool,
    pub last_ribbon_action: Option<String>,
}

impl Default for ChromeCommands {
    fn default() -> Self {
        Self::new()
    }
}

impl ChromeCommands {
    pub fn new() -> Self {
        Self {
            registry: CommandRegistry::new(),
            meta: BTreeMap::new(),
            read_only: true,
            last_ribbon_action: None,
        }
    }

    pub fn register_chrome_stubs(&mut self) {
        for m in CHROME_COMMANDS {
            let mut cmd = Command::stub(m.id, m.label);
            cmd.editing = m.editing;
            self.registry.register(cmd);
            self.meta.insert(m.id.to_string(), m.clone());
        }
        for sc in super::shortcuts::SHORTCUTS {
            if let Some(id) = sc.command {
                if self.meta.contains_key(id) {
                    continue;
                }
                let cmd = Command::stub(id, sc.label);
                self.registry.register(cmd);
                self.meta.insert(
                    id.to_string(),
                    CommandMeta {
                        id,
                        label: sc.label,
                        shortcut: sc.keys.first().copied(),
                        editing: true,
                    },
                );
            }
        }
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.registry.set_read_only(read_only);
    }

    pub fn label(&self, id: &str) -> Option<&str> {
        self.meta.get(id).map(|m| m.label)
    }

    pub fn shortcut_display(&self, id: &str) -> Option<String> {
        self.meta
            .get(id)
            .and_then(|m| m.shortcut)
            .map(|s| s.to_string())
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.registry.is_enabled(id)
    }

    pub fn is_checked(&self, _id: &str) -> bool {
        // Stub chrome: toggles default false until #66 wires cell state.
        false
    }

    pub fn dispatch(&mut self, id: &str, host: &mut dyn Any) -> Result<(), EditError> {
        if !self.registry.is_enabled(id) {
            return Err(EditError::unsupported(id));
        }
        self.last_ribbon_action = Some(id.to_string());
        let _ = self.registry.run(id, host, &Value::Null)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_command_disabled_when_readonly() {
        // Catches: editing ribbon commands must not run in read-only mode.
        let mut chrome = ChromeCommands::new();
        chrome.register_chrome_stubs();
        chrome.set_read_only(true);
        assert!(!chrome.is_enabled("home.bold"));
        let host: &mut dyn Any = &mut ();
        assert!(chrome.dispatch("home.bold", host).is_err());
    }
}
