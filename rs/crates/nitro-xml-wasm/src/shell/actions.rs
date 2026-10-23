// SPDX-License-Identifier: MIT
//! Shell action parsing and application (#29).

use super::state::ShellState;

/// Outbound side-effects the thin loader must perform after an action.
#[derive(Debug, Clone, Default)]
pub struct ShellActionResult {
    pub copy_text: Option<String>,
    pub navigate_sheet: Option<u32>,
    pub scroll_to: Option<(u32, u32)>,
    pub reopen_paint: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShellAction {
    ZoomIn,
    ZoomOut,
    ZoomSet(f64),
    FindOpen,
    FindClose,
    FindQuery(String),
    FindNext,
    FindPrev,
    FindOpts {
        case_sensitive: bool,
        match_entire: bool,
    },
    Sheet(u32),
    /// Next/prev tab (sheet or slide).
    NavNext,
    NavPrev,
    Copy,
    DiagnosticsToggle,
    SelectRef(String),
    /// Activate a hyperlink. Xlsx uses sheet cell coordinates. Docx uses
    /// `(row, col)` as paragraph ordinal and run index. Pptx has no link model
    /// (`TODO(#27)`).
    HyperlinkAt {
        sheet: u32,
        row: u32,
        col: u32,
    },
}

impl ShellAction {
    /// Parse `op` + optional `arg` from the loader RPC.
    pub fn parse(op: &str, arg: &str) -> Option<Self> {
        match op {
            "zoom-in" => Some(Self::ZoomIn),
            "zoom-out" => Some(Self::ZoomOut),
            "zoom-set" => arg.parse().ok().map(Self::ZoomSet),
            "find-open" => Some(Self::FindOpen),
            "find-close" => Some(Self::FindClose),
            "find-query" => Some(Self::FindQuery(arg.to_owned())),
            "find-next" => Some(Self::FindNext),
            "find-prev" => Some(Self::FindPrev),
            "find-opts" => {
                // arg: "case=1&entire=0"
                let mut case_sensitive = false;
                let mut match_entire = false;
                for part in arg.split('&') {
                    if let Some((k, v)) = part.split_once('=') {
                        let on = v == "1" || v.eq_ignore_ascii_case("true");
                        match k {
                            "case" => case_sensitive = on,
                            "entire" => match_entire = on,
                            _ => {}
                        }
                    }
                }
                Some(Self::FindOpts {
                    case_sensitive,
                    match_entire,
                })
            }
            "sheet" => arg.parse().ok().map(Self::Sheet),
            "nav-next" => Some(Self::NavNext),
            "nav-prev" => Some(Self::NavPrev),
            "copy" => Some(Self::Copy),
            "diagnostics-toggle" => Some(Self::DiagnosticsToggle),
            "select" => Some(Self::SelectRef(if arg.is_empty() {
                "A1".into()
            } else {
                arg.to_owned()
            })),
            "hyperlink-at" => {
                // arg: "sheet,row,col"
                let mut parts = arg.split(',');
                let sheet = parts.next()?.parse().ok()?;
                let row = parts.next()?.parse().ok()?;
                let col = parts.next()?.parse().ok()?;
                Some(Self::HyperlinkAt { sheet, row, col })
            }
            _ => None,
        }
    }
}

/// Apply a pure shell-state action (no document I/O). Document-backed ops
/// (find run, copy, sheet switch, hyperlink) are completed by the handle.
pub fn apply_action(state: &mut ShellState, action: &ShellAction) -> ShellActionResult {
    let mut out = ShellActionResult::default();
    match action {
        ShellAction::ZoomIn => {
            state.zoom_in();
            out.reopen_paint = true;
        }
        ShellAction::ZoomOut => {
            state.zoom_out();
            out.reopen_paint = true;
        }
        ShellAction::ZoomSet(z) => {
            state.set_zoom(*z);
            out.reopen_paint = true;
        }
        ShellAction::FindOpen => state.open_find(),
        ShellAction::FindClose => state.close_find(),
        ShellAction::FindQuery(q) => {
            state.find.query = q.clone();
            state.touch_chrome_rest();
            // Matches filled by handle after search.
        }
        ShellAction::FindOpts {
            case_sensitive,
            match_entire,
        } => {
            state.find.case_sensitive = *case_sensitive;
            state.find.match_entire = *match_entire;
            state.touch_chrome_rest();
        }
        ShellAction::FindNext => {
            if !state.find.matches.is_empty() {
                let n = state.find.matches.len();
                let next = state.find.index.map(|i| (i + 1) % n).unwrap_or(0);
                state.find.index = Some(next);
                if let Some(m) = state.find.current() {
                    out.navigate_sheet = Some(m.sheet);
                    out.scroll_to = Some((m.row, m.col));
                }
                state.touch_chrome_rest();
            }
        }
        ShellAction::FindPrev => {
            if !state.find.matches.is_empty() {
                let n = state.find.matches.len();
                let prev = state
                    .find
                    .index
                    .map(|i| if i == 0 { n - 1 } else { i - 1 })
                    .unwrap_or(0);
                state.find.index = Some(prev);
                if let Some(m) = state.find.current() {
                    out.navigate_sheet = Some(m.sheet);
                    out.scroll_to = Some((m.row, m.col));
                }
                state.touch_chrome_rest();
            }
        }
        ShellAction::Sheet(i) => {
            state.nav_index = *i;
            out.navigate_sheet = Some(*i);
            out.reopen_paint = true;
            state.touch_chrome_rest();
        }
        ShellAction::NavNext => {
            if let Some(i) = state
                .tabs
                .iter()
                .position(|(idx, _)| *idx == state.nav_index)
            {
                if i + 1 < state.tabs.len() {
                    let next = state.tabs[i + 1].0;
                    state.nav_index = next;
                    out.navigate_sheet = Some(next);
                    out.reopen_paint = true;
                    state.touch_chrome_rest();
                }
            }
        }
        ShellAction::NavPrev => {
            if let Some(i) = state
                .tabs
                .iter()
                .position(|(idx, _)| *idx == state.nav_index)
            {
                if i > 0 {
                    let prev = state.tabs[i - 1].0;
                    state.nav_index = prev;
                    out.navigate_sheet = Some(prev);
                    out.reopen_paint = true;
                    state.touch_chrome_rest();
                }
            }
        }
        ShellAction::Copy => {
            // Text filled by handle.
            state.touch_ui();
        }
        ShellAction::DiagnosticsToggle => state.toggle_diagnostics(),
        ShellAction::SelectRef(r) => {
            state.selection_ref = r.clone();
            state.touch_ui();
        }
        ShellAction::HyperlinkAt { .. } => {
            // Pending link filled by handle.
            state.touch_ui();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::state::FindMatch;

    #[test]
    fn nav_next_prev_walks_tabs() {
        // Catches: pptx/xlsx chrome arrow keys must move nav_index within tabs only.
        let mut state = ShellState::default();
        state.set_tabs(vec![(0, "A".into()), (1, "B".into()), (2, "C".into())], 0);
        let r = apply_action(&mut state, &ShellAction::NavNext);
        assert_eq!(state.nav_index, 1);
        assert_eq!(r.navigate_sheet, Some(1));
        let r = apply_action(&mut state, &ShellAction::NavPrev);
        assert_eq!(state.nav_index, 0);
        assert_eq!(r.navigate_sheet, Some(0));
        apply_action(&mut state, &ShellAction::NavPrev);
        assert_eq!(state.nav_index, 0);
    }

    #[test]
    fn find_next_wraps_around() {
        // Catches: Find Next at the last hit must wrap to the first (Office-style).
        let mut state = ShellState::default();
        state.find.matches = vec![
            FindMatch {
                sheet: 0,
                row: 0,
                col: 0,
                text: "a".into(),
            },
            FindMatch {
                sheet: 0,
                row: 1,
                col: 0,
                text: "ab".into(),
            },
        ];
        state.find.index = Some(1);
        let r = apply_action(&mut state, &ShellAction::FindNext);
        assert_eq!(state.find.index, Some(0));
        assert_eq!(r.scroll_to, Some((0, 0)));
    }

    #[test]
    fn zoom_clamps_to_office_range() {
        // Catches: shell zoom must stay in the same 10–400% band as grid metrics.
        let mut state = ShellState::default();
        apply_action(&mut state, &ShellAction::ZoomSet(0.5)); // factor form → 50%
        assert!((state.zoom_pct - 50.0).abs() < 1e-9);
        apply_action(&mut state, &ShellAction::ZoomSet(999.0));
        assert!((state.zoom_pct - 400.0).abs() < 1e-9);
        apply_action(&mut state, &ShellAction::ZoomSet(1.0)); // factor → 100%
        assert!((state.zoom_pct - 100.0).abs() < 1e-9);
    }
}
