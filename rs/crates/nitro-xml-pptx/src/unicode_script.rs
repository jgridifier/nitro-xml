// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/text/unicode-script-detection.ts; changes in PROVENANCE.md.

/// OOXML font script category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontScriptCategory {
    Latin,
    EastAsia,
    ComplexScript,
    Symbol,
}

/// A contiguous run of characters sharing the same font script category.
#[derive(Debug, Clone)]
pub struct ScriptRun {
    pub text: String,
    pub script: FontScriptCategory,
}

/// Resolved per-script font faces for one run (already theme-resolved).
#[derive(Debug, Clone, Default)]
pub struct ScriptFontSet {
    pub latin: Option<String>,
    pub east_asia: Option<String>,
    pub complex_script: Option<String>,
    pub symbol: Option<String>,
}

pub fn detect_font_script(code_point: u32) -> FontScriptCategory {
    if (0x2e80..=0x9fff).contains(&code_point)
        || (0xac00..=0xd7af).contains(&code_point)
        || (0xf900..=0xfaff).contains(&code_point)
        || (0xfe30..=0xfe4f).contains(&code_point)
        || (0x20000..=0x2fa1f).contains(&code_point)
        || (0x3000..=0x303f).contains(&code_point)
        || (0x3040..=0x309f).contains(&code_point)
        || (0x30a0..=0x30ff).contains(&code_point)
        || (0x31f0..=0x31ff).contains(&code_point)
        || (0xff00..=0xffef).contains(&code_point)
    {
        return FontScriptCategory::EastAsia;
    }
    if (0x0590..=0x05ff).contains(&code_point)
        || (0x0600..=0x06ff).contains(&code_point)
        || (0x0700..=0x074f).contains(&code_point)
        || (0x0750..=0x077f).contains(&code_point)
        || (0x0780..=0x07bf).contains(&code_point)
        || (0x07c0..=0x07ff).contains(&code_point)
        || (0x0800..=0x083f).contains(&code_point)
        || (0x0900..=0x097f).contains(&code_point)
        || (0x0980..=0x09ff).contains(&code_point)
        || (0x0a00..=0x0a7f).contains(&code_point)
        || (0x0a80..=0x0aff).contains(&code_point)
        || (0x0b00..=0x0b7f).contains(&code_point)
        || (0x0b80..=0x0bff).contains(&code_point)
        || (0x0c00..=0x0c7f).contains(&code_point)
        || (0x0c80..=0x0cff).contains(&code_point)
        || (0x0d00..=0x0d7f).contains(&code_point)
        || (0x0d80..=0x0dff).contains(&code_point)
        || (0x0e00..=0x0e7f).contains(&code_point)
        || (0x0e80..=0x0eff).contains(&code_point)
        || (0x0f00..=0x0fff).contains(&code_point)
        || (0x1000..=0x109f).contains(&code_point)
        || (0x10a0..=0x10ff).contains(&code_point)
        || (0x1780..=0x17ff).contains(&code_point)
        || (0xfb50..=0xfdff).contains(&code_point)
        || (0xfe70..=0xfeff).contains(&code_point)
    {
        return FontScriptCategory::ComplexScript;
    }
    if (0x2190..=0x21ff).contains(&code_point)
        || (0x2200..=0x22ff).contains(&code_point)
        || (0x2300..=0x23ff).contains(&code_point)
        || (0x2500..=0x257f).contains(&code_point)
        || (0x2580..=0x259f).contains(&code_point)
        || (0x25a0..=0x25ff).contains(&code_point)
        || (0x2600..=0x26ff).contains(&code_point)
        || (0x2700..=0x27bf).contains(&code_point)
        || (0xe000..=0xf8ff).contains(&code_point)
        || (0x1f000..=0x1f9ff).contains(&code_point)
    {
        return FontScriptCategory::Symbol;
    }
    FontScriptCategory::Latin
}

fn is_neutral_ascii(ch: char) -> bool {
    let cp = ch as u32;
    cp <= 0x7f && !ch.is_ascii_alphanumeric()
}

/// Segment text into runs of the same font script category (upstream `segmentByScript`).
pub fn segment_by_script(text: &str) -> Vec<ScriptRun> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut runs: Vec<ScriptRun> = Vec::new();
    let mut current_script: Option<FontScriptCategory> = None;
    let mut current_text = String::new();

    for ch in text.chars() {
        let script = if is_neutral_ascii(ch) {
            current_script.unwrap_or(FontScriptCategory::Latin)
        } else {
            detect_font_script(ch as u32)
        };
        if let Some(prev) = current_script {
            if prev != script {
                runs.push(ScriptRun {
                    text: current_text,
                    script: prev,
                });
                current_text = String::new();
            }
        }
        current_script = Some(script);
        current_text.push(ch);
    }
    if let Some(script) = current_script {
        if !current_text.is_empty() {
            runs.push(ScriptRun {
                text: current_text,
                script,
            });
        }
    }
    runs
}

pub fn resolve_font_for_script(
    script: FontScriptCategory,
    fonts: &ScriptFontSet,
) -> Option<String> {
    match script {
        FontScriptCategory::Latin => fonts.latin.clone(),
        FontScriptCategory::EastAsia => fonts.east_asia.clone().or_else(|| fonts.latin.clone()),
        FontScriptCategory::ComplexScript => {
            fonts.complex_script.clone().or_else(|| fonts.latin.clone())
        }
        FontScriptCategory::Symbol => fonts.symbol.clone().or_else(|| fonts.latin.clone()),
    }
}

pub fn has_distinct_script_fonts(fonts: &ScriptFontSet) -> bool {
    let base = fonts.latin.as_deref();
    if base.is_none() {
        return false;
    }
    let base = base.unwrap();
    fonts.east_asia.as_deref().is_some_and(|f| f != base)
        || fonts.complex_script.as_deref().is_some_and(|f| f != base)
        || fonts.symbol.as_deref().is_some_and(|f| f != base)
}
