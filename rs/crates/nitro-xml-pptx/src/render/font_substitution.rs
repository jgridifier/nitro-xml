// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/utils/font-substitution.ts; changes in PROVENANCE.md.

/// Fallback stack when the deck declares no typeface on a text body (upstream `DEFAULT_FONT_FAMILY`).
pub const DEFAULT_FONT_FAMILY_STACK: &str = "\"Segoe UI\", \"Helvetica Neue\", Arial, sans-serif";

/// Upstream `FONT_SUBSTITUTION_MAP` (exact, case-sensitive keys).
const FONT_SUBSTITUTION_MAP: &[(&str, &[&str])] = &[
    (
        "Aptos",
        &[
            "Calibri",
            "Carlito",
            "Liberation Sans",
            "Arial",
            "sans-serif",
        ],
    ),
    (
        "Aptos Display",
        &["Calibri Light", "Calibri", "Carlito", "Arial", "sans-serif"],
    ),
    (
        "Aptos Narrow",
        &["Calibri", "Liberation Sans Narrow", "Arial", "sans-serif"],
    ),
    (
        "Aptos Serif",
        &[
            "Cambria",
            "Caladea",
            "Liberation Serif",
            "Times New Roman",
            "serif",
        ],
    ),
    (
        "Aptos Mono",
        &["Consolas", "Liberation Mono", "Courier New", "monospace"],
    ),
    (
        "Calibri",
        &["Carlito", "Liberation Sans", "Arial", "sans-serif"],
    ),
    (
        "Calibri Light",
        &["Carlito", "Liberation Sans", "Arial", "sans-serif"],
    ),
    (
        "Cambria",
        &["Caladea", "Liberation Serif", "Times New Roman", "serif"],
    ),
    (
        "Cambria Math",
        &[
            "STIX Two Math",
            "Latin Modern Math",
            "Times New Roman",
            "serif",
        ],
    ),
    ("Consolas", &["Liberation Mono", "Courier New", "monospace"]),
    (
        "Segoe UI",
        &[
            "Selawik",
            "Liberation Sans",
            "Arimo",
            "Helvetica Neue",
            "Arial",
            "sans-serif",
        ],
    ),
    (
        "Segoe UI Light",
        &[
            "Selawik Light",
            "Liberation Sans",
            "Arimo",
            "Helvetica Neue",
            "Arial",
            "sans-serif",
        ],
    ),
    (
        "Segoe UI Semibold",
        &[
            "Selawik Semibold",
            "Liberation Sans",
            "Arimo",
            "Helvetica Neue",
            "Arial",
            "sans-serif",
        ],
    ),
    (
        "Times New Roman",
        &["Liberation Serif", "Tinos", "Times", "serif"],
    ),
    (
        "Arial",
        &["Liberation Sans", "Arimo", "Helvetica", "sans-serif"],
    ),
    (
        "Arial Black",
        &["Liberation Sans", "Arimo", "Helvetica", "sans-serif"],
    ),
    (
        "Arial Narrow",
        &[
            "Liberation Sans Narrow",
            "Arimo",
            "Helvetica Neue",
            "sans-serif",
        ],
    ),
    (
        "Helvetica",
        &["Liberation Sans", "Arimo", "Arial", "sans-serif"],
    ),
    (
        "Helvetica Neue",
        &["Liberation Sans", "Arimo", "Arial", "sans-serif"],
    ),
    (
        "Courier New",
        &["Liberation Mono", "Cousine", "Courier", "monospace"],
    ),
    (
        "Verdana",
        &["DejaVu Sans", "Bitstream Vera Sans", "sans-serif"],
    ),
    (
        "Georgia",
        &["Gelasio", "Liberation Serif", "Times New Roman", "serif"],
    ),
    ("Tahoma", &["DejaVu Sans", "Liberation Sans", "sans-serif"]),
    ("Trebuchet", &["Liberation Sans", "sans-serif"]),
    ("Trebuchet MS", &["Liberation Sans", "sans-serif"]),
    ("Comic Sans MS", &["Comic Neue", "cursive"]),
    ("Impact", &["Charcoal", "sans-serif"]),
    ("Lucida Console", &["Liberation Mono", "monospace"]),
    (
        "Lucida Sans Unicode",
        &["Lucida Grande", "Liberation Sans", "sans-serif"],
    ),
    ("Palatino", &["Palatino Linotype", "Book Antiqua", "serif"]),
    ("Palatino Linotype", &["Palatino", "Book Antiqua", "serif"]),
    ("Book Antiqua", &["Palatino Linotype", "Palatino", "serif"]),
    (
        "MS PGothic",
        &[
            "Noto Sans CJK JP",
            "Hiragino Sans",
            "Yu Gothic",
            "sans-serif",
        ],
    ),
    (
        "MS PMincho",
        &[
            "Noto Serif CJK JP",
            "Hiragino Mincho ProN",
            "Yu Mincho",
            "serif",
        ],
    ),
    (
        "MS Gothic",
        &[
            "Noto Sans CJK JP",
            "Hiragino Sans",
            "Yu Gothic",
            "sans-serif",
        ],
    ),
    (
        "MS Mincho",
        &[
            "Noto Serif CJK JP",
            "Hiragino Mincho ProN",
            "Yu Mincho",
            "serif",
        ],
    ),
    ("SimSun", &["Noto Serif CJK SC", "STSong", "serif"]),
    ("SimHei", &["Noto Sans CJK SC", "STHeiti", "sans-serif"]),
    (
        "Microsoft YaHei",
        &["Noto Sans CJK SC", "PingFang SC", "sans-serif"],
    ),
    ("NSimSun", &["Noto Serif CJK SC", "STSong", "serif"]),
    ("FangSong", &["Noto Serif CJK SC", "STFangsong", "serif"]),
    ("KaiTi", &["Noto Serif CJK SC", "STKaiti", "serif"]),
    (
        "DengXian",
        &[
            "Microsoft YaHei",
            "PingFang SC",
            "Noto Sans CJK SC",
            "sans-serif",
        ],
    ),
    (
        "等线",
        &[
            "DengXian",
            "Microsoft YaHei",
            "PingFang SC",
            "Noto Sans CJK SC",
            "sans-serif",
        ],
    ),
    (
        "宋体",
        &[
            "SimSun",
            "Songti SC",
            "STSong",
            "Noto Serif CJK SC",
            "serif",
        ],
    ),
    (
        "新宋体",
        &[
            "NSimSun",
            "Songti SC",
            "STSong",
            "Noto Serif CJK SC",
            "serif",
        ],
    ),
    (
        "黑体",
        &[
            "SimHei",
            "Heiti SC",
            "STHeiti",
            "PingFang SC",
            "Noto Sans CJK SC",
            "sans-serif",
        ],
    ),
    (
        "仿宋",
        &[
            "FangSong",
            "STFangsong",
            "Songti SC",
            "Noto Serif CJK SC",
            "serif",
        ],
    ),
    (
        "楷体",
        &["KaiTi", "Kaiti SC", "STKaiti", "Noto Serif CJK SC", "serif"],
    ),
    ("Batang", &["Noto Serif CJK KR", "AppleMyungjo", "serif"]),
    ("Dotum", &["Noto Sans CJK KR", "AppleGothic", "sans-serif"]),
    ("Gulim", &["Noto Sans CJK KR", "AppleGothic", "sans-serif"]),
    (
        "Malgun",
        &["Noto Sans CJK KR", "Apple SD Gothic Neo", "sans-serif"],
    ),
    (
        "Malgun Gothic",
        &["Noto Sans CJK KR", "Apple SD Gothic Neo", "sans-serif"],
    ),
    (
        "思源黑体 CN",
        &[
            "Source Han Sans CN",
            "Noto Sans CJK SC",
            "Noto Sans SC",
            "Microsoft YaHei",
            "sans-serif",
        ],
    ),
    (
        "思源黑体",
        &[
            "Source Han Sans SC",
            "Noto Sans CJK SC",
            "Noto Sans SC",
            "Microsoft YaHei",
            "sans-serif",
        ],
    ),
    (
        "Source Han Sans CN",
        &[
            "思源黑体 CN",
            "Noto Sans CJK SC",
            "Noto Sans SC",
            "Microsoft YaHei",
            "sans-serif",
        ],
    ),
    (
        "微软雅黑",
        &[
            "Microsoft YaHei",
            "Noto Sans CJK SC",
            "PingFang SC",
            "sans-serif",
        ],
    ),
    ("Arabic Typesetting", &["Noto Naskh Arabic", "serif"]),
    ("Simplified Arabic", &["Noto Sans Arabic", "sans-serif"]),
    ("Traditional Arabic", &["Noto Naskh Arabic", "serif"]),
    ("Mangal", &["Noto Sans Devanagari", "sans-serif"]),
    ("Vrinda", &["Noto Sans Bengali", "sans-serif"]),
    ("Raavi", &["Noto Sans Gurmukhi", "sans-serif"]),
    ("Shruti", &["Noto Sans Gujarati", "sans-serif"]),
    ("Tunga", &["Noto Sans Kannada", "sans-serif"]),
    ("Kartika", &["Noto Sans Malayalam", "sans-serif"]),
    ("Iskoola", &["Noto Sans Sinhala", "sans-serif"]),
    ("Iskoola Pota", &["Noto Sans Sinhala", "sans-serif"]),
    ("Leelawadee", &["Noto Sans Thai", "sans-serif"]),
    ("Leelawadee UI", &["Noto Sans Thai", "sans-serif"]),
    ("Cordia New", &["Noto Sans Thai", "sans-serif"]),
    ("DokChampa", &["Noto Sans Lao", "sans-serif"]),
    ("Nyala", &["Noto Sans Ethiopic", "sans-serif"]),
    ("MoolBoran", &["Noto Sans Khmer", "sans-serif"]),
    (
        "Bebas Neue",
        &["Oswald", "Arial Narrow", "Impact", "sans-serif"],
    ),
    (
        "Source Sans Pro ExtraLight",
        &[
            "Source Sans Pro",
            "Calibri Light",
            "Segoe UI Light",
            "Arial",
            "sans-serif",
        ],
    ),
    ("Century Gothic", &["URW Gothic", "Futura", "sans-serif"]),
    ("Oswald", &["Agency FB", "Arial Narrow", "sans-serif"]),
    (
        "Franklin Gothic",
        &["Liberation Sans", "Helvetica Neue", "sans-serif"],
    ),
    (
        "Franklin Gothic Medium",
        &["Liberation Sans", "Helvetica Neue", "sans-serif"],
    ),
    ("Garamond", &["EB Garamond", "Cormorant Garamond", "serif"]),
    ("Tw Cen MT", &["Century Gothic", "Futura", "sans-serif"]),
    ("Rockwell", &["Roboto Slab", "Rockwell", "serif"]),
    ("Candara", &["Liberation Sans", "Optima", "sans-serif"]),
    ("Constantia", &["Liberation Serif", "Palatino", "serif"]),
    (
        "Corbel",
        &["Liberation Sans", "Lucida Grande", "sans-serif"],
    ),
];

/// Upstream `FONT_STYLE_SUFFIX_TOKENS`.
const FONT_STYLE_SUFFIX_TOKENS: &[&str] = &[
    "regular",
    "normal",
    "book",
    "roman",
    "thin",
    "hairline",
    "extralight",
    "ultralight",
    "light",
    "demilight",
    "semilight",
    "medium",
    "demibold",
    "semibold",
    "bold",
    "extrabold",
    "ultrabold",
    "heavy",
    "black",
    "extrablack",
    "ultra",
    "italic",
    "oblique",
];

fn direct_substitution(name: &str) -> Option<&'static [&'static str]> {
    FONT_SUBSTITUTION_MAP
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
}

/// Upstream `stripFontStyleSuffix`: drop up to two trailing weight/style tokens.
fn strip_font_style_suffix(font_name: &str) -> Option<String> {
    let mut name = font_name.trim().to_owned();
    let mut stripped = false;
    for _ in 0..2 {
        let Some(idx) = name.rfind(' ').filter(|i| *i > 0) else {
            break;
        };
        let token = name[idx + 1..].to_lowercase().replace('-', "");
        if !FONT_STYLE_SUFFIX_TOKENS.contains(&token.as_str()) {
            break;
        }
        name = name[..idx].trim().to_owned();
        stripped = true;
    }
    (stripped && !name.is_empty()).then_some(name)
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

    // A full font name ("Bebas Neue Regular") also tries its family base, ahead of the map chain.
    let base = strip_font_style_suffix(primary);
    let direct =
        direct_substitution(primary).or_else(|| base.as_deref().and_then(direct_substitution));
    let mut chain: Vec<&str> = base.as_deref().into_iter().collect();
    match direct {
        Some(fallbacks) => chain.extend(fallbacks.iter().copied()),
        None => chain.push("sans-serif"),
    }

    let mut parts = vec![quote_font_name(primary)];
    for fb in chain {
        let quoted = quote_font_name(fb);
        if !quoted.is_empty() && !parts.contains(&quoted) {
            parts.push(quoted);
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
        assert_eq!(stack, "\"Informal Roman\", \"Informal\", sans-serif");
    }

    // Catches: CJK faces missing from the table (YaHei fell to Liberation Sans), which changed
    // the measured GDI tracking and the painted Latin glyphs vs upstream (bug64693).
    #[test]
    fn microsoft_yahei_uses_cjk_chain_and_lookup_is_exact() {
        assert_eq!(
            substitute_font_family_stack(Some("Microsoft YaHei")),
            "\"Microsoft YaHei\", \"Noto Sans CJK SC\", \"PingFang SC\", sans-serif"
        );
        // Negative control: upstream keys are case-sensitive, so a lowercased name is unmapped.
        assert_eq!(
            substitute_font_family_stack(Some("microsoft yahei")),
            "\"microsoft yahei\", sans-serif"
        );
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
