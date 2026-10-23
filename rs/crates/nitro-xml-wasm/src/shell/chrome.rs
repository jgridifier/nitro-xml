// SPDX-License-Identifier: MIT
//! Trusted HTML for the Stage-6 shell chrome (#29). Generated in Rust so the
//! loader only assigns `innerHTML` and forwards events — no snapshot formatting
//! in JS (keeps the 16 KB brotli budget for docx/pptx viewers).

use super::state::{FormatKind, ShellState};
use crate::chrome::EditorChrome;

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

/// Hotkey table embedded in chrome (`data-hk`). Loader matches keydown against it.
const HOTKEYS: &str = "mf:find-open|mc:copy|m=:zoom-in|m+:zoom-in|m-:zoom-out|Escape:find-close|qEnter:find-next|qSEnter:find-prev|qEscape:find-close";
/// Pptx and docx bind arrow/page keys to page navigation (xlsx keeps arrows for the grid).
const PPTX_NAV: &str = "|ArrowRight:nav-next|ArrowLeft:nav-prev|PageDown:nav-next|PageUp:nav-prev";

/// Stage-6 shell plus optional editor ribbon (ribbon above toolbar when edit mode)
/// and any open dialog/menu overlay (#54).
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};

#[cfg(test)]
static CHROME_REST_HTML_CALLS: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static CHROME_REST_HTML_CALLS_LOCK: Mutex<()> = Mutex::new(());

/// Hold across reset → call → assert so parallel tests cannot interleave the counter.
#[cfg(test)]
pub fn chrome_rest_html_calls_lock() -> MutexGuard<'static, ()> {
    CHROME_REST_HTML_CALLS_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
pub fn chrome_rest_html_calls_reset() {
    CHROME_REST_HTML_CALLS.store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub fn chrome_rest_html_calls() -> usize {
    CHROME_REST_HTML_CALLS.load(Ordering::Relaxed)
}

/// Ribbon, toolbar/find/tabs shell, and overlay — everything except the formula bar.
pub fn chrome_rest_html(state: &ShellState, editor: &EditorChrome) -> String {
    #[cfg(test)]
    CHROME_REST_HTML_CALLS.fetch_add(1, Ordering::Relaxed);
    // Empty office ribbon keeps the xlsx Home ribbon byte-for-byte.
    let ribbon = if editor.office_ribbon().is_empty() {
        editor.ribbon_html()
    } else {
        editor.office_ribbon().to_owned()
    };
    let overlay = editor.overlay_html();
    // An editable docx keeps Left/Right for the caret; page keys still turn pages.
    let arrows = !(state.format == FormatKind::Docx && !editor.read_only());
    let shell = chrome_html_with(state, arrows);
    format!("{ribbon}{shell}{overlay}")
}

pub fn combined_chrome_html(state: &ShellState, editor: &EditorChrome) -> String {
    let rest = chrome_rest_html(state, editor);
    let formula = editor.formula_html();
    format!("{rest}{formula}")
}

/// Full chrome fragment (toolbar, find, tabs, diagnostics) for the loader host.
#[cfg(test)]
pub fn chrome_html(state: &ShellState) -> String {
    chrome_html_with(state, true)
}

fn chrome_html_with(state: &ShellState, arrows_navigate: bool) -> String {
    let hk = if state.format == FormatKind::Pptx || state.format == FormatKind::Docx {
        let nav = if arrows_navigate {
            PPTX_NAV
        } else {
            "|PageDown:nav-next|PageUp:nav-prev"
        };
        format!("{HOTKEYS}{nav}")
    } else {
        HOTKEYS.to_owned()
    };
    let zoom = state.zoom_pct.round().clamp(10.0, 400.0) as i32;
    let find_hidden = if state.find.open { "" } else { " hidden" };
    let find_autofocus = if state.find.open { " autofocus" } else { "" };
    let find_status = esc(&state.find.status());
    let find_query = esc(&state.find.query);
    let tabs_label = match state.format {
        FormatKind::Pptx => "Slides",
        FormatKind::Docx => "Pages",
        FormatKind::Xlsx => "Sheets",
        // Other formats keep a generic label until their shell inventory lands.
        _ => "Tabs",
    };
    let mut tabs = String::new();
    for (index, name) in &state.tabs {
        let cur = if *index == state.nav_index {
            " aria-current=\"true\""
        } else {
            ""
        };
        tabs.push_str(&format!(
            "<button type=\"button\" role=\"tab\" data-a=\"sheet\" data-g=\"{index}\"{cur}>{}</button>",
            esc(name)
        ));
    }
    let diag_hidden = if state.diagnostics_open {
        ""
    } else {
        " hidden"
    };
    let d = &state.diagnostics;
    let mut diag_body = format!(
        "format: {}\nparse: {:.1} ms · paint: {:.1} ms\nwarnings: {}\nfont substitutions (#121): {}\nunsupported: {}\n",
        state.format.as_str(),
        d.parse_ms,
        d.paint_ms,
        d.warnings.len(),
        d.font_substitutions.len(),
        d.unsupported.len(),
    );
    for item in &d.font_substitutions {
        diag_body.push_str(&format!("· font: {}\n", esc(&item.message)));
    }
    for item in &d.warnings {
        diag_body.push_str(&format!("· {}: {}\n", esc(&item.code), esc(&item.message)));
    }
    for item in &d.unsupported {
        diag_body.push_str(&format!(
            "· unsupported: {}\n",
            esc(if item.message.is_empty() {
                &item.code
            } else {
                &item.message
            })
        ));
    }
    format!(
        r##"<div data-hk="{hk}"><style>
.b,.f,.t,.d{{display:flex;flex-wrap:wrap;gap:.25rem;align-items:center;padding:.2rem .35rem;border-bottom:1px solid #ddd;font:12px/1.25 system-ui,sans-serif}}
.b button,.f button,.t button{{font:inherit;padding:.12rem .35rem}}
.t button[aria-current=true]{{font-weight:700}}
.f[hidden],.d[hidden]{{display:none!important}}
.f input{{flex:1;min-width:6rem;font:inherit}}
.d{{align-items:flex-start;white-space:pre-wrap;max-height:8rem;overflow:auto;background:#f7f7f7}}
</style>
<div class="b" part="toolbar">
<button type="button" data-a="zoom-out" title="Zoom out" aria-label="Zoom out"><span aria-hidden="true">−</span></button>
<span class="z" part="zoom" aria-live="polite">{zoom}%</span>
<button type="button" data-a="zoom-in" title="Zoom in" aria-label="Zoom in"><span aria-hidden="true">+</span></button>
<button type="button" data-a="find-open">Find</button>
<button type="button" data-a="copy">Copy</button>
<button type="button" data-a="diagnostics-toggle">Diagnostics</button>
</div>
<div class="f" part="find"{find_hidden}>
<input class="q" type="search" placeholder="Find" aria-label="Find" value="{find_query}"{find_autofocus}/>
<button type="button" data-a="find-prev" aria-label="Find previous"><span aria-hidden="true">↑</span></button>
<button type="button" data-a="find-next" aria-label="Find next"><span aria-hidden="true">↓</span></button>
<span class="fs" part="find-status" aria-live="polite">{find_status}</span>
<button type="button" data-a="find-close" aria-label="Close find"><span aria-hidden="true">✕</span></button>
</div>
<div class="t" part="tabs" role="tablist" aria-label="{tabs_label}">{tabs}</div>
<pre class="d" part="diagnostics"{diag_hidden}>{diag}</pre></div>"##,
        hk = hk,
        zoom = zoom,
        find_hidden = find_hidden,
        find_query = find_query,
        find_autofocus = find_autofocus,
        find_status = find_status,
        tabs_label = tabs_label,
        tabs = tabs,
        diag_hidden = diag_hidden,
        diag = diag_body.trim_end(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::state::{FindState, ShellState};

    #[test]
    fn chrome_escapes_tab_names() {
        // Catches: sheet names with <>& must not break chrome HTML injection.
        let state = ShellState {
            tabs: vec![(0, "A<B>&\"C".into())],
            ..Default::default()
        };
        let html = chrome_html(&state);
        assert!(html.contains("A&lt;B&gt;&amp;&quot;C"));
        assert!(!html.contains("A<B>"));
        assert!(html.contains("data-hk="));
    }

    #[test]
    fn chrome_autofocus_when_find_open() {
        // Catches: find open must request focus via HTML so JS stays apply-only.
        let state = ShellState {
            find: FindState {
                open: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let html = chrome_html(&state);
        assert!(html.contains(" autofocus"));
    }
}
