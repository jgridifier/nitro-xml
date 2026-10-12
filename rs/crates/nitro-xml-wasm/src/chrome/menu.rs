// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/menu/menu-model.ts
// (nextEnabledIndex, typeAheadIndex, clampFlyoutPosition) and context-menu /
// menu-button open/close behaviour; changes listed in PROVENANCE.md.
//! Menus and context menus for editor chrome (#54).

/// One row of a controlled menu (mirrors `OfficeMenuItem`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub separator_before: bool,
    pub heading: Option<String>,
    pub danger: bool,
    pub disabled: bool,
    pub checked: Option<bool>,
    pub shortcut: Option<String>,
}

impl MenuItem {
    pub fn cmd(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            separator_before: false,
            heading: None,
            danger: false,
            disabled: false,
            checked: None,
            shortcut: None,
        }
    }

    pub fn with_separator(mut self) -> Self {
        self.separator_before = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// Why a menu closed (mirrors `OfficeMenuCloseReason` + item activation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCloseReason {
    Escape,
    Outside,
    Tab,
    Item,
    Api,
}

impl MenuCloseReason {
    pub fn restores_focus(self) -> bool {
        matches!(self, Self::Escape | Self::Tab | Self::Item)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Escape => "escape",
            Self::Outside => "outside",
            Self::Tab => "tab",
            Self::Item => "item",
            Self::Api => "api",
        }
    }
}

/// Open menu / context-menu state.
#[derive(Clone, Debug)]
pub struct OpenMenu {
    pub x: f32,
    pub y: f32,
    pub label: String,
    pub items: Vec<MenuItem>,
    pub active_index: isize,
    pub context: bool,
    pub left: f32,
    pub top: f32,
}

impl OpenMenu {
    pub fn context(label: &str, x: f32, y: f32, items: Vec<MenuItem>) -> Self {
        let active = next_enabled_index(&items, -1, 1);
        Self {
            x,
            y,
            label: label.to_string(),
            items,
            active_index: active,
            context: true,
            left: x,
            top: y,
        }
    }

    pub fn anchored(label: &str, x: f32, y: f32, items: Vec<MenuItem>) -> Self {
        let mut m = Self::context(label, x, y, items);
        m.context = false;
        m
    }

    pub fn clamp_to_viewport(&mut self, width: f32, height: f32, vw: f32, vh: f32) {
        let p = clamp_flyout_position(self.x, self.y, width, height, vw, vh, 8.0);
        self.left = p.0;
        self.top = p.1;
    }

    pub fn move_active(&mut self, step: i8) {
        let step = if step >= 0 { 1 } else { -1 };
        let next = next_enabled_index(&self.items, self.active_index, step);
        if next >= 0 {
            self.active_index = next;
        }
    }

    pub fn type_ahead(&mut self, query: &str) {
        let next = type_ahead_index(&self.items, self.active_index, query);
        if next >= 0 {
            self.active_index = next;
        }
    }
}

/// Index of the next enabled row from `from` in `step` direction, wrapping; -1 if none.
pub fn next_enabled_index(items: &[MenuItem], from: isize, step: i8) -> isize {
    let count = items.len() as isize;
    if count == 0 {
        return -1;
    }
    let step = step as isize;
    for offset in 1..=count {
        let index = (((from + step * offset) % count) + count) % count;
        if !items[index as usize].disabled {
            return index;
        }
    }
    -1
}

/// Type-ahead target: next enabled row after `from` whose label starts with `query`.
pub fn type_ahead_index(items: &[MenuItem], from: isize, query: &str) -> isize {
    if items.is_empty() || query.is_empty() {
        return -1;
    }
    let needle = query.to_lowercase();
    let first = needle.chars().next().unwrap_or_default();
    let cycling = needle.chars().all(|c| c == first);
    let prefix: String = if cycling {
        first.to_string()
    } else {
        needle.clone()
    };
    let count = items.len() as isize;
    let start_offset = if cycling { 1 } else { 0 };
    for offset in start_offset..=count {
        let index = (from + offset + count) % count;
        let item = &items[index as usize];
        if !item.disabled && item.label.trim().to_lowercase().starts_with(&prefix) {
            return index;
        }
    }
    -1
}

/// Top-left corner keeping a pointer-anchored flyout fully on screen.
pub fn clamp_flyout_position(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    viewport_width: f32,
    viewport_height: f32,
    margin: f32,
) -> (f32, f32) {
    let max_left = viewport_width - width - margin;
    let max_top = viewport_height - height - margin;
    let left = if width > 0.0 {
        x.min(max_left).max(margin)
    } else {
        x.max(margin)
    };
    let top = if height > 0.0 {
        y.min(max_top).max(margin)
    } else {
        y.max(margin)
    };
    (left, top)
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

/// Render an open menu / context menu as trusted HTML.
pub fn menu_html(open: &OpenMenu) -> String {
    let kind = if open.context { "context-menu" } else { "menu" };
    let mut items = String::new();
    for (i, item) in open.items.iter().enumerate() {
        if item.separator_before {
            items.push_str("<hr class=\"rb-menu-sep\" role=\"separator\" />");
        }
        if let Some(h) = &item.heading {
            items.push_str(&format!(
                "<div class=\"rb-menu-heading\" role=\"presentation\">{}</div>",
                esc(h)
            ));
        }
        let disabled = if item.disabled {
            " aria-disabled=\"true\" tabindex=\"-1\""
        } else {
            " tabindex=\"-1\""
        };
        let active = if i as isize == open.active_index {
            " data-active=\"1\" class=\"rb-menu-item active\""
        } else {
            " class=\"rb-menu-item\""
        };
        let role = if item.checked.is_some() {
            "menuitemcheckbox"
        } else {
            "menuitem"
        };
        let checked = match item.checked {
            Some(true) => " aria-checked=\"true\"",
            Some(false) => " aria-checked=\"false\"",
            None => "",
        };
        let danger = if item.danger {
            " data-danger=\"1\""
        } else {
            ""
        };
        let shortcut = item
            .shortcut
            .as_ref()
            .map(|s| {
                format!(
                    " <span class=\"rb-menu-shortcut\" aria-hidden=\"true\">{}</span>",
                    esc(s)
                )
            })
            .unwrap_or_default();
        let aria_keys = item
            .shortcut
            .as_ref()
            .map(|s| format!(" aria-keyshortcuts=\"{}\"", esc(s)))
            .unwrap_or_default();
        items.push_str(&format!(
            "<button type=\"button\" role=\"{role}\" data-a=\"menu-item\" data-g=\"{id}\"{disabled}{active}{checked}{danger}{aria_keys}>{label}{shortcut}</button>",
            id = esc(&item.id),
            label = esc(&item.label),
            role = role,
            disabled = disabled,
            active = active,
            checked = checked,
            danger = danger,
            aria_keys = aria_keys,
            shortcut = shortcut,
        ));
    }
    format!(
        "<div class=\"xve-{kind}\" role=\"menu\" aria-label=\"{label}\" \
            data-menu=\"1\" data-context=\"{ctx}\" \
            style=\"position:fixed;left:{left}px;top:{top}px;z-index:40\">{items}</div>",
        kind = kind,
        label = esc(&open.label),
        ctx = if open.context { "1" } else { "0" },
        left = open.left,
        top = open.top,
        items = items,
    )
}

/// Demo / harness items for a cell context menu (labels only; runners TODO(#66)).
pub fn stub_cell_context_items() -> Vec<MenuItem> {
    vec![
        MenuItem::cmd("edit.cut", "Cut").with_separator(),
        MenuItem::cmd("edit.copy", "Copy"),
        MenuItem::cmd("edit.paste", "Paste"),
        MenuItem::cmd("home.clear", "Clear Contents").with_separator(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_arrow_navigation_skips_disabled_rows() {
        // Catches: ArrowDown must skip disabled items (menu-model nextEnabledIndex).
        let items = vec![
            MenuItem::cmd("a", "Alpha"),
            MenuItem::cmd("b", "Bravo").disabled(),
            MenuItem::cmd("c", "Charlie"),
        ];
        assert_eq!(next_enabled_index(&items, 0, 1), 2);
        assert_eq!(next_enabled_index(&items, 2, -1), 0);
    }

    #[test]
    fn catches_flyout_clamps_inside_viewport_margin() {
        // Catches: context menu near the bottom-right must flip inward (8px margin).
        let (left, top) = clamp_flyout_position(390.0, 290.0, 120.0, 80.0, 400.0, 300.0, 8.0);
        assert!((left - 272.0).abs() < 0.5, "left={left}");
        assert!((top - 212.0).abs() < 0.5, "top={top}");
        assert!(left >= 8.0 && top >= 8.0);
    }
}
