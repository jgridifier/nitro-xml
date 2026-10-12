// SPDX-License-Identifier: MIT
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/keyboard.ts; changes listed in PROVENANCE.md.

//! Global keyboard shortcuts for the xlsx editor chrome (#53).

/// `Mod` is Cmd on Apple platforms and Ctrl elsewhere; `code:` matches `KeyboardEvent.code`.
pub type KeySpec = &'static str;

pub type ShellAction = &'static str; // 'help' | 'keytips' | 'next-region' | 'previous-region' | 'context-menu'

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub keys: &'static [KeySpec],
    pub label: &'static str,
    pub command: Option<&'static str>,
    pub arg: Option<&'static str>,
    pub action: Option<ShellAction>,
    /// True: also works while typing in a text field (formula bar, dialogs).
    pub global: bool,
}

/// Excel's shortcuts, mapped to command ids (`file.*` and `sheet.next/previous` are the shell's).
pub const SHORTCUTS: &[Shortcut] = &[
    Shortcut {
        keys: &["Mod+S"],
        label: "Save",
        command: Some("file.save"),
        arg: None,
        action: None,
        global: true,
    },
    Shortcut {
        keys: &["Mod+O"],
        label: "Open",
        command: Some("file.open"),
        arg: None,
        action: None,
        global: true,
    },
    Shortcut {
        keys: &["Mod+N"],
        label: "New workbook",
        command: Some("file.new"),
        arg: None,
        action: None,
        global: true,
    },
    Shortcut {
        keys: &["Mod+P"],
        label: "Print",
        command: Some("file.print"),
        arg: None,
        action: None,
        global: true,
    },
    Shortcut {
        keys: &["Mod+F"],
        label: "Find",
        command: Some("home.find"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+H"],
        label: "Replace",
        command: Some("home.replace"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+G", "F5"],
        label: "Go To",
        command: Some("home.go-to"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Z"],
        label: "Undo",
        command: Some("edit.undo"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Y", "Mod+Shift+Z"],
        label: "Redo",
        command: Some("edit.redo"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Alt+V"],
        label: "Paste Special",
        command: Some("home.paste-special"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+1"],
        label: "Format Cells",
        command: Some("format.cells"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+B", "Mod+2"],
        label: "Bold",
        command: Some("home.bold"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+I", "Mod+3"],
        label: "Italic",
        command: Some("home.italic"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+U", "Mod+4"],
        label: "Underline",
        command: Some("home.underline"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+5"],
        label: "Strikethrough",
        command: Some("home.strikethrough"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift+~", "Mod+Shift+code:Backquote"],
        label: "General number format",
        command: Some("home.format-general"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift+$", "Mod+Shift+code:Digit4"],
        label: "Currency format",
        command: Some("home.format-currency"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift+%", "Mod+Shift+code:Digit5"],
        label: "Percentage format",
        command: Some("home.format-percent"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift+^", "Mod+Shift+code:Digit6"],
        label: "Scientific format",
        command: Some("home.format-scientific"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift+#", "Mod+Shift+code:Digit3"],
        label: "Date format",
        command: Some("home.format-date"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift+@", "Mod+Shift+code:Digit2"],
        label: "Time format",
        command: Some("home.format-time"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift+!", "Mod+Shift+code:Digit1"],
        label: "Number format",
        command: Some("home.format-number"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+;"],
        label: "Insert the current date",
        command: Some("edit.insert-date"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift+:", "Mod+Shift+;"],
        label: "Insert the current time",
        command: Some("edit.insert-time"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+`", "Mod+code:Backquote"],
        label: "Show Formulas",
        command: Some("formulas.show-formulas"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Alt+="],
        label: "AutoSum",
        command: Some("home.autosum"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+ "],
        label: "Select the entire column",
        command: Some("edit.select-column"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Shift+ "],
        label: "Select the entire row",
        command: Some("edit.select-row"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+PageUp"],
        label: "Previous sheet",
        command: Some("sheet.previous"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+PageDown"],
        label: "Next sheet",
        command: Some("sheet.next"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift+L"],
        label: "Filter",
        command: Some("data.filter"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+T", "Mod+L"],
        label: "Create a table",
        command: Some("insert.table"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+K"],
        label: "Insert link",
        command: Some("insert.link"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Shift+F2"],
        label: "New comment",
        command: Some("review.new-comment"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+-"],
        label: "Delete cells",
        command: Some("cells.delete"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift++", "Mod++", "Mod+Shift+="],
        label: "Insert cells",
        command: Some("cells.insert"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["F9"],
        label: "Calculate Now",
        command: Some("formulas.calculate-now"),
        arg: None,
        action: None,
        global: false,
    },
    Shortcut {
        keys: &["F6"],
        label: "Move to the next region",
        command: None,
        arg: None,
        action: Some("next-region"),
        global: true,
    },
    Shortcut {
        keys: &["Shift+F6"],
        label: "Move to the previous region",
        command: None,
        arg: None,
        action: Some("previous-region"),
        global: true,
    },
    Shortcut {
        keys: &["F10", "Alt"],
        label: "Show KeyTips",
        command: None,
        arg: None,
        action: Some("keytips"),
        global: true,
    },
    Shortcut {
        keys: &["Shift+F10", "ContextMenu"],
        label: "Open the context menu",
        command: None,
        arg: None,
        action: Some("context-menu"),
        global: false,
    },
    Shortcut {
        keys: &["Mod+/", "F1"],
        label: "Keyboard shortcuts",
        command: None,
        arg: None,
        action: Some("help"),
        global: true,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEventLike<'a> {
    pub key: &'a str,
    pub code: Option<&'a str>,
    pub ctrl_key: bool,
    pub meta_key: bool,
    pub alt_key: bool,
    pub shift_key: bool,
}

struct Parsed {
    mod_key: bool,
    shift: bool,
    alt: bool,
    key: String,
    code: Option<String>,
    tap: bool,
}

fn parse(spec: KeySpec) -> Parsed {
    let parts: Vec<&str> = spec.split('+').collect();
    let mut key = parts.last().unwrap_or(&"").to_string();
    if key.is_empty() && spec.ends_with('+') {
        key = "+".to_string();
    }
    let modifiers: std::collections::HashSet<String> = parts
        .iter()
        .take(parts.len().saturating_sub(1))
        .map(|p| p.to_lowercase())
        .filter(|p| !p.is_empty())
        .collect();
    let code = key
        .strip_prefix("code:")
        .map(|stripped| stripped.to_string());
    Parsed {
        mod_key: modifiers.contains("mod"),
        shift: modifiers.contains("shift"),
        alt: modifiers.contains("alt"),
        key: if code.is_some() {
            String::new()
        } else {
            key.to_lowercase()
        },
        code,
        tap: key == "Alt" && modifiers.is_empty(),
    }
}

#[allow(dead_code)]
const DISPLAY: &[(&str, &str)] = &[
    (" ", "Space"),
    ("escape", "Esc"),
    ("contextmenu", "Menu"),
    ("pageup", "PgUp"),
    ("pagedown", "PgDn"),
];

/// Exact modifier match: Ctrl on a Mac, or Cmd elsewhere, never satisfies `Mod`.
pub fn matches_keys(event: &KeyEventLike<'_>, spec: KeySpec, mac: bool) -> bool {
    let wanted = parse(spec);
    if wanted.tap {
        return false;
    }
    let mod_key = if mac { event.meta_key } else { event.ctrl_key };
    let foreign = if mac { event.ctrl_key } else { event.meta_key };
    if mod_key != wanted.mod_key
        || foreign
        || event.alt_key != wanted.alt
        || event.shift_key != wanted.shift
    {
        return false;
    }
    if let Some(ref code) = wanted.code {
        return event.code == Some(code.as_str());
    }
    event.key.to_lowercase() == wanted.key
}

/// Human-readable key text (`Ctrl+Shift+L`, `Cmd+S` on a Mac). Legends are not translated.
#[allow(dead_code)]
pub fn format_keys(spec: KeySpec, mac: bool) -> String {
    let parsed = parse(spec);
    let key = parsed.code.clone().unwrap_or_else(|| {
        if spec.ends_with('+') {
            "+".to_string()
        } else {
            spec.split('+').next_back().unwrap_or("").to_string()
        }
    });
    let display_key = DISPLAY
        .iter()
        .find(|(k, _)| *k == key.to_lowercase())
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| {
            if key.len() == 1 {
                key.to_uppercase()
            } else {
                key.clone()
            }
        });
    let names = [
        if parsed.mod_key {
            if mac {
                "Cmd"
            } else {
                "Ctrl"
            }
        } else {
            ""
        },
        if parsed.alt {
            if mac {
                "Option"
            } else {
                "Alt"
            }
        } else {
            ""
        },
        if parsed.shift { "Shift" } else { "" },
        display_key.as_str(),
    ];
    names
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("+")
}

pub fn find_shortcut(event: &KeyEventLike<'_>, mac: bool) -> Option<&'static Shortcut> {
    SHORTCUTS.iter().find(|shortcut| {
        shortcut
            .keys
            .iter()
            .any(|spec| matches_keys(event, spec, mac))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_b_matches_bold_non_mac() {
        // Catches: Mod+B must map to home.bold with Ctrl on non-Mac.
        let ev = KeyEventLike {
            key: "b",
            code: Some("KeyB"),
            ctrl_key: true,
            meta_key: false,
            alt_key: false,
            shift_key: false,
        };
        assert!(matches_keys(&ev, "Mod+B", false));
        let s = find_shortcut(&ev, false).expect("shortcut");
        assert_eq!(s.command, Some("home.bold"));
    }

    #[test]
    fn ctrl_on_mac_does_not_match_mod() {
        // Catches: Ctrl+B on Mac must not satisfy Mod+B (foreign modifier).
        let ev = KeyEventLike {
            key: "b",
            code: Some("KeyB"),
            ctrl_key: true,
            meta_key: false,
            alt_key: false,
            shift_key: false,
        };
        assert!(!matches_keys(&ev, "Mod+B", true));
    }
}
