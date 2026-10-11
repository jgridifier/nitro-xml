// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/fonts.ts; changes listed in PROVENANCE.md.

use super::units::DEFAULT_FONT_FAMILY;

struct Substitute {
    fallbacks: &'static [&'static str],
    generic: &'static str,
}

fn substitutes() -> &'static [(&'static str, Substitute)] {
    &[
        (
            "calibri",
            Substitute {
                fallbacks: &["Carlito"],
                generic: "sans-serif",
            },
        ),
        (
            "calibri light",
            Substitute {
                fallbacks: &["Carlito"],
                generic: "sans-serif",
            },
        ),
        (
            "cambria",
            Substitute {
                fallbacks: &["Caladea"],
                generic: "serif",
            },
        ),
        (
            "arial",
            Substitute {
                fallbacks: &["Arimo", "Liberation Sans"],
                generic: "sans-serif",
            },
        ),
        (
            "helvetica",
            Substitute {
                fallbacks: &["Arimo", "Liberation Sans"],
                generic: "sans-serif",
            },
        ),
        (
            "times new roman",
            Substitute {
                fallbacks: &["Tinos", "Liberation Serif"],
                generic: "serif",
            },
        ),
        (
            "times",
            Substitute {
                fallbacks: &["Tinos", "Liberation Serif"],
                generic: "serif",
            },
        ),
        (
            "courier new",
            Substitute {
                fallbacks: &["Cousine", "Liberation Mono"],
                generic: "monospace",
            },
        ),
        (
            "courier",
            Substitute {
                fallbacks: &["Cousine", "Liberation Mono"],
                generic: "monospace",
            },
        ),
        (
            "georgia",
            Substitute {
                fallbacks: &["Gelasio"],
                generic: "serif",
            },
        ),
        (
            "consolas",
            Substitute {
                fallbacks: &["Inconsolata"],
                generic: "monospace",
            },
        ),
    ]
}

fn quote(family: &str) -> String {
    format!("\"{}\"", family.replace(['"', '\\'], ""))
}

pub fn css_font_stack(family: Option<&str>) -> String {
    let name = family
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_FONT_FAMILY);
    let known = substitutes()
        .iter()
        .find(|(k, _)| *k == name.to_lowercase())
        .map(|(_, v)| v);
    let mut parts = vec![quote(name)];
    if let Some(known) = known {
        for fb in known.fallbacks {
            parts.push(quote(fb));
        }
        parts.push(known.generic.to_string());
    } else {
        parts.push("sans-serif".to_string());
    }
    parts.join(", ")
}
