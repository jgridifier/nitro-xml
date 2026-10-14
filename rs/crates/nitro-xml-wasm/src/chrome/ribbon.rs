// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/ribbon/tabs/home.ts and home-groups.ts; changes listed in PROVENANCE.md.

//! Declarative Home ribbon HTML (#53). Galleries render as menu stubs (TODO(#54)).

use super::ribbon_icons::{command_icon, icon_path, menu_icon, ribbon_icon_svg};
use super::EditorChrome;

const RIBBON_SKIN: &str = include_str!("ribbon_skin.css");

const RIBBON_VARS: &str = r#"--xl:#217346;--ink:#444444;--muted:#666666;--line:#e1e1e1;--surface:#ffffff;--chrome:#f5f5f5;--accent-soft:rgb(33 115 70 / 0.12);"#;

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

#[derive(Clone, Copy)]
enum ControlKind {
    Button,
    Toggle,
    Split,
    Menu,
    Select,
    Color,
    Stack,
    Separator,
    Gallery,
}

struct Control {
    kind: ControlKind,
    command: Option<&'static str>,
    label: Option<&'static str>,
    menu: &'static [MenuEntry],
}

enum MenuEntry {
    Sep,
    Cmd(&'static str),
}

struct Group {
    label: &'static str,
    controls: &'static [Control],
}

const MENU: &[MenuEntry] = &[];
const CLIPBOARD_PASTE_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("home.paste"),
    MenuEntry::Cmd("home.paste-values"),
    MenuEntry::Cmd("home.paste-formulas"),
    MenuEntry::Cmd("home.paste-formatting"),
    MenuEntry::Cmd("home.paste-transpose"),
    MenuEntry::Sep,
    MenuEntry::Cmd("home.paste-special"),
];
const UNDERLINE_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("home.underline"),
    MenuEntry::Cmd("home.underline-double"),
];
const MERGE_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("home.merge-center"),
    MenuEntry::Cmd("home.merge-across"),
    MenuEntry::Cmd("home.merge-cells"),
    MenuEntry::Cmd("home.unmerge"),
];
const INSERT_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("cells.insert"),
    MenuEntry::Cmd("cells.insert-rows"),
    MenuEntry::Cmd("cells.insert-columns"),
    MenuEntry::Cmd("sheet.insert"),
];
const DELETE_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("cells.delete"),
    MenuEntry::Cmd("cells.delete-rows"),
    MenuEntry::Cmd("cells.delete-columns"),
    MenuEntry::Cmd("sheet.delete"),
];
const FORMAT_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("format.row-height"),
    MenuEntry::Cmd("format.autofit-rows"),
    MenuEntry::Cmd("format.column-width"),
    MenuEntry::Cmd("format.autofit-columns"),
    MenuEntry::Cmd("format.default-width"),
    MenuEntry::Sep,
    MenuEntry::Cmd("format.hide-rows"),
    MenuEntry::Cmd("format.hide-columns"),
    MenuEntry::Cmd("sheet.hide"),
    MenuEntry::Cmd("format.unhide-rows"),
    MenuEntry::Cmd("format.unhide-columns"),
    MenuEntry::Cmd("sheet.unhide"),
    MenuEntry::Sep,
    MenuEntry::Cmd("sheet.rename"),
    MenuEntry::Cmd("sheet.move-copy"),
    MenuEntry::Cmd("sheet.tab-color"),
    MenuEntry::Sep,
    MenuEntry::Cmd("review.protect-sheet"),
    MenuEntry::Cmd("format.lock-cell"),
    MenuEntry::Cmd("format.cells"),
];
const AUTOSUM_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("home.autosum"),
    MenuEntry::Cmd("home.autosum-average"),
    MenuEntry::Cmd("home.autosum-count"),
    MenuEntry::Cmd("home.autosum-max"),
    MenuEntry::Cmd("home.autosum-min"),
    MenuEntry::Sep,
    MenuEntry::Cmd("home.autosum-more"),
];
const FILL_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("home.fill-down"),
    MenuEntry::Cmd("home.fill-right"),
    MenuEntry::Cmd("home.fill-up"),
    MenuEntry::Cmd("home.fill-left"),
    MenuEntry::Sep,
    MenuEntry::Cmd("home.fill-series"),
];
const CLEAR_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("home.clear-all"),
    MenuEntry::Cmd("home.clear-formats"),
    MenuEntry::Cmd("home.clear-contents"),
    MenuEntry::Cmd("home.clear-comments"),
    MenuEntry::Cmd("home.clear-hyperlinks"),
];
const SORT_FILTER_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("data.sort-asc"),
    MenuEntry::Cmd("data.sort-desc"),
    MenuEntry::Cmd("data.sort-custom"),
    MenuEntry::Sep,
    MenuEntry::Cmd("data.filter"),
    MenuEntry::Cmd("data.filter-clear"),
    MenuEntry::Cmd("data.filter-reapply"),
];
const FIND_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("home.find"),
    MenuEntry::Cmd("home.replace"),
    MenuEntry::Cmd("home.go-to"),
    MenuEntry::Cmd("home.go-to-special"),
    MenuEntry::Sep,
    MenuEntry::Cmd("home.select-formulas"),
    MenuEntry::Cmd("home.select-comments"),
    MenuEntry::Cmd("home.select-conditional"),
    MenuEntry::Cmd("home.select-constants"),
    MenuEntry::Cmd("home.select-validation"),
];
const CF_MENU: &[MenuEntry] = &[
    MenuEntry::Cmd("home.cf-quick"),
    MenuEntry::Sep,
    MenuEntry::Cmd("home.cf-data-bar"),
    MenuEntry::Sep,
    MenuEntry::Cmd("home.cf-color-scale"),
    MenuEntry::Sep,
    MenuEntry::Cmd("home.cf-icon-set"),
    MenuEntry::Sep,
    MenuEntry::Cmd("home.cf-new-rule"),
    MenuEntry::Cmd("home.cf-clear-selection"),
    MenuEntry::Cmd("home.cf-clear-sheet"),
    MenuEntry::Cmd("home.cf-manage"),
];
const STACK_CLIP: &[Control] = &[
    Control {
        kind: ControlKind::Button,
        command: Some("home.cut"),
        label: None,
        menu: MENU,
    },
    Control {
        kind: ControlKind::Button,
        command: Some("home.copy"),
        label: None,
        menu: MENU,
    },
    Control {
        kind: ControlKind::Toggle,
        command: Some("home.format-painter"),
        label: None,
        menu: MENU,
    },
];

const HOME_GROUPS: &[Group] = &[
    Group {
        label: "Clipboard",
        controls: &[
            Control {
                kind: ControlKind::Split,
                command: Some("home.paste"),
                label: None,
                menu: CLIPBOARD_PASTE_MENU,
            },
            Control {
                kind: ControlKind::Stack,
                command: None,
                label: None,
                menu: MENU,
            },
        ],
    },
    Group {
        label: "Font",
        controls: &[
            Control {
                kind: ControlKind::Select,
                command: Some("home.font-name"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Select,
                command: Some("home.font-size"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Button,
                command: Some("home.grow-font"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Button,
                command: Some("home.shrink-font"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Separator,
                command: None,
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Toggle,
                command: Some("home.bold"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Toggle,
                command: Some("home.italic"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Split,
                command: Some("home.underline"),
                label: None,
                menu: UNDERLINE_MENU,
            },
            Control {
                kind: ControlKind::Toggle,
                command: Some("home.strikethrough"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Split,
                command: Some("home.borders"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Color,
                command: Some("home.fill-color"),
                label: Some("Fill Color"),
                menu: MENU,
            },
            Control {
                kind: ControlKind::Color,
                command: Some("home.font-color"),
                label: Some("Font Color"),
                menu: MENU,
            },
        ],
    },
    Group {
        label: "Alignment",
        controls: &[
            Control {
                kind: ControlKind::Toggle,
                command: Some("home.align-top"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Toggle,
                command: Some("home.align-middle"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Toggle,
                command: Some("home.align-bottom"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Menu,
                command: None,
                label: Some("Orientation"),
                menu: MENU,
            },
            Control {
                kind: ControlKind::Toggle,
                command: Some("home.wrap-text"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Separator,
                command: None,
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Toggle,
                command: Some("home.align-left"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Toggle,
                command: Some("home.align-center"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Toggle,
                command: Some("home.align-right"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Button,
                command: Some("home.indent-decrease"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Button,
                command: Some("home.indent-increase"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Split,
                command: Some("home.merge-center"),
                label: None,
                menu: MERGE_MENU,
            },
        ],
    },
    Group {
        label: "Number",
        controls: &[
            Control {
                kind: ControlKind::Select,
                command: Some("home.number-format"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Split,
                command: Some("home.accounting"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Button,
                command: Some("home.percent"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Button,
                command: Some("home.comma"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Button,
                command: Some("home.decimal-increase"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Button,
                command: Some("home.decimal-decrease"),
                label: None,
                menu: MENU,
            },
        ],
    },
    Group {
        label: "Styles",
        controls: &[
            Control {
                kind: ControlKind::Menu,
                command: None,
                label: Some("Conditional Formatting"),
                menu: CF_MENU,
            },
            Control {
                kind: ControlKind::Gallery,
                command: Some("home.format-as-table"),
                label: None,
                menu: MENU,
            },
            Control {
                kind: ControlKind::Gallery,
                command: Some("home.cell-styles"),
                label: None,
                menu: MENU,
            },
        ],
    },
    Group {
        label: "Cells",
        controls: &[
            Control {
                kind: ControlKind::Split,
                command: Some("cells.insert-split"),
                label: None,
                menu: INSERT_MENU,
            },
            Control {
                kind: ControlKind::Split,
                command: Some("cells.delete-split"),
                label: None,
                menu: DELETE_MENU,
            },
            Control {
                kind: ControlKind::Menu,
                command: None,
                label: Some("Format"),
                menu: FORMAT_MENU,
            },
        ],
    },
    Group {
        label: "Editing",
        controls: &[
            Control {
                kind: ControlKind::Split,
                command: Some("home.autosum"),
                label: None,
                menu: AUTOSUM_MENU,
            },
            Control {
                kind: ControlKind::Menu,
                command: None,
                label: Some("Fill"),
                menu: FILL_MENU,
            },
            Control {
                kind: ControlKind::Menu,
                command: None,
                label: Some("Clear"),
                menu: CLEAR_MENU,
            },
            Control {
                kind: ControlKind::Menu,
                command: None,
                label: Some("Sort & Filter"),
                menu: SORT_FILTER_MENU,
            },
            Control {
                kind: ControlKind::Menu,
                command: None,
                label: Some("Find & Select"),
                menu: FIND_MENU,
            },
        ],
    },
];

fn control_is_large(c: &Control) -> bool {
    match c.kind {
        ControlKind::Split => matches!(
            c.command,
            Some("home.paste") | Some("cells.insert-split") | Some("cells.delete-split")
        ),
        ControlKind::Menu | ControlKind::Gallery => c.label.is_some(),
        _ => false,
    }
}

fn icon_markup(command: &str, label: &str, large: bool) -> String {
    let size = if large { 28 } else { 16 };
    let path = command_icon(command)
        .or_else(|| menu_icon(label))
        .or_else(|| icon_path("caret"));
    path.map(|p| ribbon_icon_svg(p, size, command))
        .unwrap_or_default()
}

fn caret_button() -> String {
    let p = icon_path("caret").unwrap_or("M7 10l5 5 5-5");
    format!(
        "<button type=\"button\" class=\"ribbon-caret\" aria-label=\"Open menu\" aria-haspopup=\"menu\">{}</button>",
        ribbon_icon_svg(p, 10, "caret"),
    )
}

fn cmd_button(chrome: &EditorChrome, id: &str, pressed: bool, large: bool, show_label: bool) -> String {
    let label = chrome.commands.label(id).unwrap_or(id);
    let title = chrome
        .commands
        .shortcut_display(id)
        .map(|s| format!(" title=\"{}\"", esc(&s)))
        .unwrap_or_default();
    let pressed_attr = if pressed {
        " aria-pressed=\"true\""
    } else {
        " aria-pressed=\"false\""
    };
    let disabled = if chrome.is_enabled(id) {
        ""
    } else {
        " disabled"
    };
    let cls = if large {
        "ribbon-large"
    } else {
        "ribbon-small ribbon-labelled"
    };
    let mut body = icon_markup(id, label, large);
    if show_label && (large || !icon_markup(id, label, large).is_empty()) {
        body.push_str(&format!("<span>{}</span>", esc(label)));
    } else if show_label {
        body.push_str(&esc(label));
    }
    format!(
        "<button type=\"button\" class=\"{cls}\" data-a=\"cmd\" data-g=\"{id}\" aria-label=\"{lbl}\"{pressed}{title}{disabled}>{body}</button>",
        cls = cls,
        id = esc(id),
        lbl = esc(label),
        pressed = pressed_attr,
        title = title,
        disabled = disabled,
        body = body,
    )
}

fn render_select(chrome: &EditorChrome, id: &str, width: u32) -> String {
    let label = chrome.commands.label(id).unwrap_or(id);
    // Static defaults until selection sync (#53 follow-up). 10 matches poi-45544 A1.
    let value = if id == "home.font-name" {
        "Arial"
    } else if id == "home.font-size" {
        "10"
    } else {
        "General"
    };
    format!(
        r#"<span class="ribbon-combo" style="width:{w}px"><input type="text" data-a="cmd" data-g="{id}" aria-label="{lbl}" value="{val}" />{caret}</span>"#,
        w = width,
        id = esc(id),
        lbl = esc(label),
        val = esc(value),
        caret = caret_button(),
    )
}

fn render_control(chrome: &EditorChrome, c: &Control, out: &mut String) {
    match c.kind {
        ControlKind::Separator => {
            out.push_str("<span class=\"ribbon-separator\" aria-hidden=\"true\"></span>")
        }
        ControlKind::Stack => {
            out.push_str("<div class=\"ribbon-stack\">");
            for sc in STACK_CLIP {
                render_control(chrome, sc, out);
            }
            out.push_str("</div>");
        }
        ControlKind::Button => {
            if let Some(id) = c.command {
                // Home icon buttons are glyph-only (aria-label carries the name), matching upstream.
                let show_label = command_icon(id).is_none();
                out.push_str(&cmd_button(chrome, id, false, false, show_label));
            }
        }
        ControlKind::Color => {
            if let Some(id) = c.command {
                let lbl = c.label.unwrap_or("Color");
                let bar = if id == "home.font-color" {
                    // Upstream default font-color swatch (theme accent red), not workbook green.
                    " style=\"--bar:#c00000\""
                } else {
                    " style=\"--bar:#ffc000\""
                };
                out.push_str(&format!(
                    "<button type=\"button\" class=\"ribbon-small ribbon-color\" data-a=\"cmd\" data-g=\"{}\" aria-label=\"{}\"{}>{}</button>",
                    esc(id),
                    esc(lbl),
                    bar,
                    icon_markup(id, lbl, false),
                ));
            }
        }
        ControlKind::Select => {
            if let Some(id) = c.command {
                let w = if id == "home.font-name" {
                    130
                } else if id == "home.font-size" {
                    52
                } else {
                    96
                };
                out.push_str(&render_select(chrome, id, w));
            }
        }
        ControlKind::Toggle => {
            if let Some(id) = c.command {
                let pressed = chrome.is_checked(id);
                out.push_str(&cmd_button(chrome, id, pressed, false, false));
            }
        }
        ControlKind::Split => {
            if let Some(id) = c.command {
                let large = control_is_large(c);
                let split_cls = if large {
                    "ribbon-split ribbon-split-large"
                } else {
                    "ribbon-split"
                };
                out.push_str(&format!("<span class=\"{}\">", split_cls));
                out.push_str(&cmd_button(chrome, id, chrome.is_checked(id), large, large));
                out.push_str(&caret_button());
                out.push_str("</span>");
                if !c.menu.is_empty() {
                    out.push_str("<span class=\"rb-menu-stub\" hidden>");
                    for entry in c.menu {
                        render_menu_entry(chrome, entry, out);
                    }
                    out.push_str("</span>");
                }
            }
        }
        ControlKind::Menu | ControlKind::Gallery => {
            let lbl = c
                .label
                .or(c.command.and_then(|id| chrome.commands.label(id)))
                .unwrap_or("Menu");
            let large = control_is_large(c);
            let cls = if large {
                "ribbon-large"
            } else {
                "ribbon-small ribbon-labelled"
            };
            let cmd_attr = c
                .command
                .map(|id| format!(" data-g=\"{}\"", esc(id)))
                .unwrap_or_default();
            let icon = menu_icon(lbl)
                .or_else(|| c.command.and_then(command_icon))
                .map(|p| ribbon_icon_svg(p, if large { 28 } else { 16 }, lbl))
                .unwrap_or_default();
            out.push_str(&format!(
                "<button type=\"button\" class=\"{cls}\" data-a=\"cmd\"{cmd} aria-label=\"{lbl}\" aria-haspopup=\"menu\">{icon}<span>{text}</span></button>",
                cls = cls,
                cmd = cmd_attr,
                lbl = esc(lbl),
                icon = icon,
                text = esc(lbl),
            ));
            if !c.menu.is_empty() {
                out.push_str("<span class=\"rb-menu-stub\" hidden>");
                for entry in c.menu {
                    render_menu_entry(chrome, entry, out);
                }
                out.push_str("</span>");
            }
        }
    }
}

fn render_menu_entry(chrome: &EditorChrome, entry: &MenuEntry, out: &mut String) {
    match entry {
        MenuEntry::Sep => out.push_str("<span class=\"rb-menu-sep\"></span>"),
        MenuEntry::Cmd(id) => {
            let label = chrome.commands.label(id).unwrap_or(id);
            out.push_str(&format!(
                "<button type=\"button\" data-a=\"cmd\" data-g=\"{}\" aria-label=\"{}\">{}</button>",
                esc(id),
                esc(label),
                esc(label),
            ));
        }
    }
}

/// Home tab ribbon fragment, or empty when hidden.
pub fn ribbon_html(chrome: &EditorChrome) -> String {
    if chrome.read_only() || !chrome.loaded() {
        return String::new();
    }
    let mut groups = String::new();
    for g in HOME_GROUPS {
        groups.push_str("<div class=\"ribbon-group\" data-caption=\"");
        groups.push_str(&esc(g.label));
        groups.push_str("\" role=\"group\" aria-label=\"");
        groups.push_str(&esc(g.label));
        groups.push_str("\">");
        for c in g.controls {
            render_control(chrome, c, &mut groups);
        }
        groups.push_str("</div>");
    }
    let tab = esc(chrome.active_tab());
    format!(
        r##"<div class="xve-ribbon" style="{vars}"><style>.xve-ribbon{{{vars}}}{skin}.rb-menu-stub{{display:none!important}}</style><div class="ribbon-panel" id="xve-panel-{tab}" data-tab="{tab}" data-ribbon-tab="{tab}" data-label="Home">{groups}</div></div>"##,
        vars = RIBBON_VARS,
        skin = RIBBON_SKIN,
        tab = tab,
        groups = groups,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::EditorChrome;

    #[test]
    fn ribbon_visible_when_loaded_editable() {
        // Catches: edit-mode chrome must expose Home ribbon controls.
        let mut chrome = EditorChrome::new();
        chrome.load();
        chrome.set_read_only(false);
        let html = ribbon_html(&chrome);
        assert!(html.contains("ribbon-panel"));
        assert!(html.contains("data-g=\"home.bold\""));
    }

    #[test]
    fn ribbon_hidden_in_readonly() {
        // Catches: readonly viewers must not leak editor ribbon HTML (negative control).
        let mut chrome = EditorChrome::new();
        chrome.load();
        chrome.set_read_only(true);
        assert!(ribbon_html(&chrome).is_empty());
    }

    #[test]
    fn formula_bar_cleared_when_readonly() {
        // Catches: readonly chrome must not keep formula-bar HTML after leaving edit mode.
        let mut chrome = EditorChrome::new();
        chrome.load();
        chrome.set_read_only(false);
        chrome.set_formula_html("<div class=\"xfb\"></div>".into());
        assert!(chrome.formula_html().contains("xfb"));
        chrome.set_read_only(true);
        assert!(
            chrome.formula_html().is_empty(),
            "readonly must drop the formula bar"
        );
        chrome.set_formula_html("<div class=\"xfb\"></div>".into());
        assert!(chrome.formula_html().is_empty());
    }
}
