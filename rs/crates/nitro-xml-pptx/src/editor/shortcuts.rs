// SPDX-License-Identifier: MIT
//! PowerPoint editor keyboard shortcuts (#88). `Mod` is Cmd on macOS, Ctrl elsewhere.

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
        keys: &["Mod+B"],
        label: "Bold",
        command: "home.bold",
        global: false,
    },
    Shortcut {
        keys: &["Mod+I"],
        label: "Italic",
        command: "home.italic",
        global: false,
    },
    Shortcut {
        keys: &["Mod+D"],
        label: "Duplicate",
        command: "home.duplicate",
        global: false,
    },
    Shortcut {
        keys: &["Delete", "Backspace"],
        label: "Delete",
        command: "home.delete",
        global: false,
    },
    Shortcut {
        keys: &["Mod+A"],
        label: "Select All",
        command: "home.select-all",
        global: false,
    },
    Shortcut {
        keys: &["Mod+G"],
        label: "Group",
        command: "home.group",
        global: false,
    },
    Shortcut {
        keys: &["Mod+Shift+G"],
        label: "Ungroup",
        command: "home.ungroup",
        global: false,
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
    if wanted.mod_key {
        if mod_key != wanted.mod_key
            || foreign
            || event.alt != wanted.alt
            || event.shift != wanted.shift
        {
            return false;
        }
    } else if foreign
        || event.ctrl
        || event.meta
        || event.alt != wanted.alt
        || event.shift != wanted.shift
    {
        // Bare Delete/Backspace: reject when any Mod is held.
        if wanted.key != "delete" && wanted.key != "backspace" {
            return false;
        }
        if event.ctrl || event.meta || event.alt || event.shift {
            return false;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_delete_key_maps_to_home_delete() {
        // Catches: Delete key not wiring to home.delete in the pptx editor (#88).
        let ev = KeyEvent {
            key: "Delete".into(),
            code: Some("Delete".into()),
            ctrl: false,
            meta: false,
            shift: false,
            alt: false,
        };
        let m = find_shortcut(&ev, false).expect("match");
        assert_eq!(m.command, "home.delete");
    }
}
