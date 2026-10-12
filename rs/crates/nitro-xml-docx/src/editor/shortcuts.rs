// SPDX-License-Identifier: MIT
//! Word editor keyboard shortcuts (#83). `Mod` is Cmd on macOS, Ctrl elsewhere.

/// `code:` entries match `KeyboardEvent.code` when present.
pub type KeySpec = &'static str;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub keys: &'static [KeySpec],
    pub label: &'static str,
    pub command: &'static str,
    pub global: bool,
}

pub const SHORTCUTS: &[Shortcut] = &[
    Shortcut {
        keys: &["Mod+S"],
        label: "Save",
        command: "file.save",
        global: true,
    },
    Shortcut {
        keys: &["Mod+Z"],
        label: "Undo",
        command: "edit.undo",
        global: false,
    },
    Shortcut {
        keys: &["Mod+Y", "Mod+Shift+Z"],
        label: "Redo",
        command: "edit.redo",
        global: false,
    },
    Shortcut {
        keys: &["Mod+F"],
        label: "Find",
        command: "find-and-replace",
        global: true,
    },
    Shortcut {
        keys: &["Mod+H"],
        label: "Replace",
        command: "replace",
        global: true,
    },
    Shortcut {
        keys: &["Mod+B"],
        label: "Bold",
        command: "bold",
        global: false,
    },
    Shortcut {
        keys: &["Mod+I"],
        label: "Italic",
        command: "italic",
        global: false,
    },
    Shortcut {
        keys: &["Mod+U"],
        label: "Underline",
        command: "underline",
        global: false,
    },
    Shortcut {
        keys: &["Mod+P"],
        label: "Print",
        command: "print",
        global: true,
    },
];

#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key: String,
    pub code: Option<String>,
    pub ctrl: bool,
    pub meta: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutMatch {
    pub command: &'static str,
}

struct Parsed {
    mod_key: bool,
    shift: bool,
    alt: bool,
    key: String,
    code: Option<String>,
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
    }
}

pub fn matches_keys(event: &KeyEvent, spec: KeySpec, mac: bool) -> bool {
    let wanted = parse(spec);
    let mod_key = if mac { event.meta } else { event.ctrl };
    let foreign = if mac { event.ctrl } else { event.meta };
    if mod_key != wanted.mod_key
        || foreign
        || event.alt != wanted.alt
        || event.shift != wanted.shift
    {
        return false;
    }
    if let Some(ref code) = wanted.code {
        return event.code.as_deref() == Some(code.as_str());
    }
    event.key.to_lowercase() == wanted.key
}

pub fn find_shortcut(event: &KeyEvent, mac: bool) -> Option<ShortcutMatch> {
    SHORTCUTS.iter().find_map(|shortcut| {
        shortcut
            .keys
            .iter()
            .find(|spec| matches_keys(event, spec, mac))
            .map(|_| ShortcutMatch {
                command: shortcut.command,
            })
    })
}
