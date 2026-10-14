// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/utils/font-substitution.ts; changes in PROVENANCE.md.

/// Fallback stack when the deck declares no typeface on a text body (upstream `DEFAULT_FONT_FAMILY`).
pub const DEFAULT_FONT_FAMILY_STACK: &str = "\"Segoe UI\", \"Helvetica Neue\", Arial, sans-serif";

struct Substitute {
    fallbacks: &'static [&'static str],
}

fn substitutes() -> &'static [(&'static str, Substitute)] {
    &[
        (
            "aptos",
            Substitute {
                fallbacks: &[
                    "Calibri",
                    "Carlito",
                    "Liberation Sans",
                    "Arial",
                    "sans-serif",
                ],
            },
        ),
        (
            "aptos display",
            Substitute {
                fallbacks: &["Calibri Light", "Calibri", "Carlito", "Arial", "sans-serif"],
            },
        ),
        (
            "aptos narrow",
            Substitute {
                fallbacks: &["Calibri", "Liberation Sans Narrow", "Arial", "sans-serif"],
            },
        ),
        (
            "calibri",
            Substitute {
                fallbacks: &["Carlito", "Liberation Sans", "Arial", "sans-serif"],
            },
        ),
        (
            "calibri light",
            Substitute {
                fallbacks: &["Carlito", "Liberation Sans", "Arial", "sans-serif"],
            },
        ),
        (
            "cambria",
            Substitute {
                fallbacks: &["Caladea", "Liberation Serif", "Times New Roman", "serif"],
            },
        ),
        (
            "arial",
            Substitute {
                fallbacks: &["Liberation Sans", "Arimo", "Helvetica", "sans-serif"],
            },
        ),
        (
            "helvetica",
            Substitute {
                fallbacks: &["Liberation Sans", "Arimo", "Arial", "sans-serif"],
            },
        ),
        (
            "times new roman",
            Substitute {
                fallbacks: &["Liberation Serif", "Tinos", "Times", "serif"],
            },
        ),
        (
            "courier new",
            Substitute {
                fallbacks: &["Liberation Mono", "Cousine", "Courier", "monospace"],
            },
        ),
        (
            "consolas",
            Substitute {
                fallbacks: &["Liberation Mono", "Courier New", "monospace"],
            },
        ),
        (
            "georgia",
            Substitute {
                fallbacks: &["Gelasio", "Liberation Serif", "Times New Roman", "serif"],
            },
        ),
        (
            "segoe ui",
            Substitute {
                fallbacks: &[
                    "Selawik",
                    "Liberation Sans",
                    "Arimo",
                    "Helvetica Neue",
                    "Arial",
                    "sans-serif",
                ],
            },
        ),
        (
            "corbel",
            Substitute {
                fallbacks: &["sans-serif"],
            },
        ),
    ]
}

fn is_generic_family(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "serif"
            | "sans-serif"
            | "monospace"
            | "cursive"
            | "fantasy"
            | "system-ui"
            | "ui-serif"
            | "ui-sans-serif"
            | "ui-monospace"
            | "ui-rounded"
            | "math"
            | "emoji"
            | "fangsong"
    )
}

fn quote_font_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if is_generic_family(trimmed) {
        return trimmed.to_owned();
    }
    format!("\"{}\"", trimmed.replace('"', ""))
}

/// CSS `font-family` value mirroring upstream `getSubstituteFontFamily` (no PANOSE yet).
pub fn substitute_font_family_stack(font_name: Option<&str>) -> String {
    let trimmed = font_name.map(str::trim).filter(|s| !s.is_empty());
    let Some(primary) = trimmed else {
        return DEFAULT_FONT_FAMILY_STACK.to_owned();
    };

    let key = primary.to_ascii_lowercase();
    let subs = substitutes()
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.fallbacks);

    let mut parts = vec![quote_font_name(primary)];
    if let Some(fallbacks) = subs {
        for fb in fallbacks {
            let quoted = quote_font_name(fb);
            if !quoted.is_empty() && !parts.contains(&quoted) {
                parts.push(quoted);
            }
        }
    } else {
        let generic = quote_font_name("sans-serif");
        if !parts.contains(&generic) {
            parts.push(generic);
        }
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn informal_roman_short_stack() {
        // Catches: script/display faces forced through Calibri/Carlito ahead of sans-serif.
        let stack = substitute_font_family_stack(Some("Informal Roman"));
        assert_eq!(stack, "\"Informal Roman\", sans-serif");
    }

    #[test]
    fn calibri_metric_clone_chain() {
        let stack = substitute_font_family_stack(Some("Calibri"));
        assert!(stack.starts_with("\"Calibri\""));
        assert!(stack.contains("\"Carlito\""));
        assert!(stack.ends_with("sans-serif"));
    }

    #[test]
    fn missing_font_uses_default_stack() {
        let stack = substitute_font_family_stack(None);
        assert_eq!(stack, DEFAULT_FONT_FAMILY_STACK);
    }
}
