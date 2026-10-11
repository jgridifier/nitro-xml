// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/run-formatting.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Map, Value};

const TOGGLE_KEYS: [&str; 9] = [
    "bold",
    "italic",
    "boldComplexScript",
    "italicComplexScript",
    "caps",
    "smallCaps",
    "strike",
    "doubleStrike",
    "vanish",
];

const OVERRIDE_KEYS: [&str; 20] = [
    "underline",
    "underlineStyle",
    "underlineColor",
    "highlight",
    "verticalAlign",
    "fontSize",
    "fontFamily",
    "fontFamilyEastAsia",
    "fontFamilyComplexScript",
    "fontSizeComplexScript",
    "fontTheme",
    "color",
    "colorTheme",
    "characterSpacingTwips",
    "textScalePercent",
    "ligatures",
    "kerningHalfPoints",
    "positionHalfPoints",
    "shadingFill",
    "shadingThemeFill",
];

pub struct RunFormattingContext {
    pub paragraph_style_id: Option<String>,
    pub paragraph_catalog: Option<Value>,
    pub run_catalog: Option<Value>,
}

pub fn resolve_run_formatting(run: &Value, context: RunFormattingContext) -> Value {
    let run_catalog = context.run_catalog.as_ref();
    let doc_defaults = run_catalog
        .and_then(|c| c.get("docDefaults"))
        .cloned()
        .unwrap_or(json!({}));
    let paragraph_chain = paragraph_style_run_chain(
        context.paragraph_style_id.as_deref(),
        context.paragraph_catalog.as_ref(),
        run_catalog,
    );
    let character_chain =
        character_style_chain(run.get("style").and_then(|v| v.as_str()), run_catalog);
    let levels: Vec<Value> = std::iter::once(doc_defaults.clone())
        .chain(paragraph_chain.clone())
        .chain(character_chain.clone())
        .chain(std::iter::once(run.clone()))
        .collect();
    let mut result = Map::new();
    for key in TOGGLE_KEYS {
        let direct = run.get(key);
        let state = if direct.is_some() {
            direct.and_then(|v| v.as_bool()).unwrap_or(false)
        } else {
            let doc = doc_defaults
                .get(key)
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let p = chain_value(&paragraph_chain, key);
            let c = chain_value(&character_chain, key);
            doc != (p != c)
        };
        if state {
            result.insert(key.into(), json!(true));
        }
    }
    for key in OVERRIDE_KEYS {
        for level in &levels {
            if let Some(value) = level.get(key) {
                result.insert(key.into(), value.clone());
            }
        }
    }
    Value::Object(result)
}

fn chain_value(chain: &[Value], key: &str) -> bool {
    for level in chain.iter().rev() {
        if let Some(v) = level.get(key) {
            return v.as_bool().unwrap_or(false);
        }
    }
    false
}

fn paragraph_style_run_chain(
    style_id: Option<&str>,
    paragraph_catalog: Option<&Value>,
    run_catalog: Option<&Value>,
) -> Vec<Value> {
    let paragraph_catalog = match paragraph_catalog {
        Some(c) => c,
        None => return Vec::new(),
    };
    let styles = match paragraph_catalog.get("styles").and_then(|v| v.as_object()) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let mut chain = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut current = style_id.or_else(|| {
        styles
            .values()
            .find(|s| {
                s.get("isDefault")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .and_then(|s| s.get("id").and_then(|v| v.as_str()))
    });
    while let Some(id) = current {
        if visited.contains(id) {
            break;
        }
        visited.insert(id);
        let style = match styles.get(id) {
            Some(s) => s,
            None => break,
        };
        if let Some(run_catalog) = run_catalog {
            if let Some(run_def) = run_catalog
                .get("styles")
                .and_then(|s| s.get(id))
                .and_then(|s| s.get("formatting"))
            {
                chain.push(run_def.clone());
            }
        }
        current = style.get("basedOn").and_then(|v| v.as_str());
    }
    chain.reverse();
    chain
}

fn character_style_chain(style_id: Option<&str>, run_catalog: Option<&Value>) -> Vec<Value> {
    let style_id = match style_id {
        Some(id) => id,
        None => return Vec::new(),
    };
    let run_catalog = match run_catalog {
        Some(c) => c,
        None => return Vec::new(),
    };
    let styles = match run_catalog.get("styles").and_then(|v| v.as_object()) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let mut chain = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut current = Some(style_id);
    while let Some(id) = current {
        if visited.contains(id) {
            break;
        }
        visited.insert(id);
        let style = match styles.get(id) {
            Some(s) => s,
            None => break,
        };
        if let Some(fmt) = style.get("formatting") {
            chain.push(fmt.clone());
        }
        current = style.get("basedOn").and_then(|v| v.as_str());
    }
    chain.reverse();
    chain
}
