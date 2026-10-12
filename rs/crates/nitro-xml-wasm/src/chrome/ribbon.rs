// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/xlsx/ribbon/tabs/home.ts and home-groups.ts; changes listed in PROVENANCE.md.

//! Declarative Home ribbon HTML (#53). Galleries render as menu stubs (TODO(#54)).

use super::EditorChrome;

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

fn cmd_button(chrome: &EditorChrome, id: &str, pressed: bool) -> String {
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
    format!(
        "<button type=\"button\" data-a=\"cmd\" data-g=\"{}\" aria-label=\"{}\"{}{}{}>{}</button>",
        esc(id),
        esc(label),
        pressed_attr,
        title,
        disabled,
        esc(label),
    )
}

fn render_control(chrome: &EditorChrome, c: &Control, out: &mut String) {
    match c.kind {
        ControlKind::Separator => {
            out.push_str("<span class=\"rb-sep\" aria-hidden=\"true\"></span>")
        }
        ControlKind::Stack => {
            out.push_str("<span class=\"rb-stack\">");
            for sc in STACK_CLIP {
                render_control(chrome, sc, out);
            }
            out.push_str("</span>");
        }
        ControlKind::Button | ControlKind::Color | ControlKind::Select => {
            if let Some(id) = c.command {
                let lbl = c
                    .label
                    .unwrap_or_else(|| chrome.commands.label(id).unwrap_or(id));
                let title = chrome
                    .commands
                    .shortcut_display(id)
                    .map(|s| format!(" title=\"{}\"", esc(&s)))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "<button type=\"button\" data-a=\"cmd\" data-g=\"{}\" aria-label=\"{}\"{}>{}</button>",
                    esc(id),
                    esc(lbl),
                    title,
                    esc(lbl),
                ));
            }
        }
        ControlKind::Toggle => {
            if let Some(id) = c.command {
                let pressed = chrome.is_checked(id);
                out.push_str(&cmd_button(chrome, id, pressed));
            }
        }
        ControlKind::Split => {
            if let Some(id) = c.command {
                out.push_str(&cmd_button(chrome, id, chrome.is_checked(id)));
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
            let cmd_attr = c
                .command
                .map(|id| format!(" data-g=\"{}\"", esc(id)))
                .unwrap_or_default();
            out.push_str(&format!(
                "<button type=\"button\" data-a=\"cmd\"{} aria-label=\"{}\" aria-haspopup=\"menu\">{}</button>",
                cmd_attr,
                esc(lbl),
                esc(lbl),
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
        groups.push_str("<div class=\"rb-g\" role=\"group\" aria-label=\"");
        groups.push_str(&esc(g.label));
        groups.push_str("\">");
        for c in g.controls {
            render_control(chrome, c, &mut groups);
        }
        groups.push_str("</div>");
    }
    format!(
        r##"<div class="rb" role="toolbar" aria-label="Ribbon"><style>
.rb{{display:flex;flex-wrap:wrap;gap:.35rem;padding:.25rem .35rem;border-bottom:1px solid #c8c8c8;font:11px/1.2 system-ui,sans-serif;background:#f3f3f3}}
.rb-g{{display:flex;flex-wrap:wrap;gap:.15rem;align-items:center;padding:0 .25rem;border-right:1px solid #d0d0d0}}
.rb-g:last-child{{border-right:none}}
.rb button{{font:inherit;padding:.1rem .3rem;max-width:6rem;overflow:hidden;text-overflow:ellipsis}}
.rb-stack{{display:flex;flex-direction:column;gap:.1rem}}
.rb-sep{{width:1px;height:1.2rem;background:#ccc;margin:0 .1rem}}
.rb-menu-stub{{display:none}}
</style>
<div role="tablist" aria-label="Ribbon tabs"><button type="button" role="tab" aria-selected="true" aria-label="Home" data-tab="{}">Home</button></div>
{groups}</div>"##,
        esc(chrome.active_tab()),
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
        assert!(html.contains("role=\"toolbar\""));
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
}
