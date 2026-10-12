// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/text-wrap-estimate.ts;
// changes listed in PROVENANCE.md.

fn estimated_glyph_advance_ratio(glyph: char) -> f64 {
    if glyph.is_whitespace() {
        return 0.33;
    }
    if is_cjk_or_emoji_glyph(glyph) {
        return 1.0;
    }
    if glyph.is_ascii_uppercase() {
        return 0.65;
    }
    if glyph.is_ascii_lowercase() || glyph.is_ascii_digit() {
        return 0.55;
    }
    if matches!(
        glyph,
        ',' | '.'
            | ';'
            | ':'
            | '!'
            | '?'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '\''
            | '"'
            | '`'
            | '-'
            | '_'
            | '/'
            | '\\'
    ) {
        return 0.35;
    }
    0.6
}

fn is_cjk_or_emoji_glyph(c: char) -> bool {
    let u = c as u32;
    (0x4E00..=0x9FFF).contains(&u)
        || (0x3400..=0x4DBF).contains(&u)
        || (0x3040..=0x30FF).contains(&u)
        || (0xAC00..=0xD7AF).contains(&u)
        || (0x1F300..=0x1FAFF).contains(&u)
}

fn estimated_text_width(text: &str, font_size: f64) -> f64 {
    text.chars()
        .map(|g| estimated_glyph_advance_ratio(g) * font_size)
        .sum()
}

/// Break `text` into lines that approximately fit `max_width` (same units as `font_size`).
pub fn wrap_text_by_estimated_width(text: &str, max_width: f64, font_size: f64) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    if max_width <= 0.0 || font_size <= 0.0 || !max_width.is_finite() || !font_size.is_finite() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    for paragraph in text.split(['\r', '\n']) {
        if paragraph.trim().is_empty() {
            continue;
        }
        wrap_paragraph(paragraph, max_width, font_size, &mut lines);
    }
    lines
}

fn wrap_paragraph(paragraph: &str, max_width: f64, font_size: f64, lines: &mut Vec<String>) {
    let units = estimated_wrap_units(paragraph);
    let mut current = String::new();
    let mut pending_whitespace = false;

    for unit in units {
        if unit.chars().all(char::is_whitespace) {
            pending_whitespace = !current.is_empty();
            continue;
        }

        let separator = if pending_whitespace && !current.is_empty() {
            " "
        } else {
            ""
        };
        let candidate = format!("{}{}{}", current, separator, unit);
        if current.is_empty() || estimated_text_width(&candidate, font_size) <= max_width {
            current = candidate;
            pending_whitespace = false;
            continue;
        }
        if is_hanging_punctuation(&unit) {
            current.push_str(&unit);
            pending_whitespace = false;
            continue;
        }

        lines.push(current);
        current = unit;
        pending_whitespace = false;
    }
    let completed = current.trim_end();
    if !completed.is_empty() {
        lines.push(completed.to_owned());
    }
}

fn is_hanging_punctuation(unit: &str) -> bool {
    let mut chars = unit.chars();
    let c = match chars.next() {
        Some(c) if chars.next().is_none() => c,
        _ => return false,
    };
    matches!(
        c,
        '\u{3001}'
            | '\u{3002}'
            | '\u{FF0C}'
            | '\u{FF0E}'
            | '\u{FF01}'
            | '\u{FF1F}'
            | '\u{FF1B}'
            | '\u{FF1A}'
            | '\u{3009}'
            | '\u{300B}'
            | '\u{300D}'
            | '\u{300F}'
            | '\u{3011}'
            | '\u{3015}'
            | '\u{3017}'
            | '\u{3019}'
            | '\u{301B}'
            | '\u{FF09}'
            | '\u{FF3D}'
            | '\u{FF5D}'
            | '\u{2019}'
            | '\u{201D}'
            | '\u{00BB}'
            | '\u{203A}'
    )
}

fn estimated_wrap_units(text: &str) -> Vec<String> {
    let mut units = Vec::new();
    let mut unbreakable_run = String::new();

    for glyph in text.chars() {
        if glyph.is_whitespace() || is_cjk_or_emoji_glyph(glyph) {
            if !unbreakable_run.is_empty() {
                units.push(unbreakable_run.clone());
                unbreakable_run.clear();
            }
            units.push(glyph.to_string());
        } else {
            unbreakable_run.push(glyph);
        }
    }
    if !unbreakable_run.is_empty() {
        units.push(unbreakable_run);
    }
    units
}

/// Han / kana / hangul in label text → PowerPoint's taller SmartArt line box (1.33×).
pub fn drawing_text_base_line_height_ratio(text: &str) -> f64 {
    const CJK_SMARTART_LINE_HEIGHT_RATIO: f64 = 1.33;
    const DEFAULT_SMARTART_LINE_HEIGHT_RATIO: f64 = 1.2;
    if text.chars().any(is_cjk_or_emoji_glyph) {
        CJK_SMARTART_LINE_HEIGHT_RATIO
    } else {
        DEFAULT_SMARTART_LINE_HEIGHT_RATIO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: flat 0.55×char-count wrap keeping "Node Two has a longer label" on one line while TS estimated width breaks before "label".
    #[test]
    fn hier5_child_label_wraps_at_shape_font_not_smaller_run_font() {
        let text = "Node Two has a longer label";
        let max_width = 192.48;
        let shape_font = 32.0 * (96.0 / 72.0);
        let lines = wrap_text_by_estimated_width(text, max_width, shape_font);
        assert!(
            lines.len() >= 2,
            "expected wrap at first-run font width; got {:?}",
            lines
        );
        assert_eq!(lines.join(" "), text);
    }
}
