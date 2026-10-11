// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/numbering-parse.ts, numbering-format.ts,
// numbering-text.ts; changes listed in PROVENANCE.md.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::{json, Map, Value};

use nitro_xml_opc::xml::{parse_xml_str, Element};

use crate::json_util::{insert_i64, insert_str};
use crate::simple_types::{on_off_element, parse_integer, parse_signed_twips, parse_twips};
use crate::styles::{resolve_style_numbering, ParagraphProps};
use crate::xml_util::{by_name_w, children_w, first_w, get_w};

pub fn is_st_number_format(value: &str) -> bool {
    ST_NUMBER_FORMAT.contains(&value)
}

const ST_NUMBER_FORMAT: &[&str] = &[
    "decimal",
    "upperRoman",
    "lowerRoman",
    "upperLetter",
    "lowerLetter",
    "ordinal",
    "cardinalText",
    "ordinalText",
    "hex",
    "chicago",
    "decimalZero",
    "bullet",
    "none",
];

pub fn parse_numbering_catalog(xml: &str) -> Value {
    let document = parse_xml_str(xml).expect("numbering xml");
    let root = &document.root;
    let mut abstract_nums = Map::new();
    let mut warnings = Vec::new();
    for element in by_name_w(root, "abstractNum") {
        if let Some(parsed) = parse_abstract_num(element) {
            let id = parsed
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            abstract_nums.insert(id.clone(), parsed);
            if let Some(link) = get_w(first_w(Some(element), "numStyleLink"), "val") {
                warnings.push(format!(
                    "Numbering definition abstractNum {id} links to style \"{link}\"; style-linked numbering levels are not resolved."
                ));
            }
        }
    }
    let mut nums = Map::new();
    for element in by_name_w(root, "num") {
        if let Some(parsed) = parse_num(element) {
            let id = parsed
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            nums.insert(id, parsed);
        }
    }
    json!({
        "abstractNums": abstract_nums,
        "nums": nums,
        "warnings": dedupe(warnings)
    })
}

fn parse_abstract_num(element: &Element) -> Option<Value> {
    let id = get_w(Some(element), "abstractNumId")?;
    let mut levels = Map::new();
    for lvl in children_w(element, "lvl") {
        let parsed = parse_numbering_level(lvl);
        let level = parsed.get("level").and_then(|v| v.as_i64()).unwrap_or(0);
        levels.insert(level.to_string(), parsed);
    }
    Some(json!({ "id": id, "levels": levels }))
}

fn parse_num(element: &Element) -> Option<Value> {
    let id = get_w(Some(element), "numId")?;
    let abstract_num_id = get_w(first_w(Some(element), "abstractNumId"), "val").unwrap_or_default();
    let mut level_overrides = Map::new();
    for lvl_override in children_w(element, "lvlOverride") {
        let level = parse_integer(get_w(Some(lvl_override), "ilvl").as_deref()).unwrap_or(0);
        let mut entry = Map::new();
        if let Some(so) = get_w(first_w(Some(lvl_override), "startOverride"), "val") {
            entry.insert(
                "startOverride".into(),
                json!(parse_integer(Some(&so)).unwrap_or(1)),
            );
        }
        if let Some(lvl) = first_w(Some(lvl_override), "lvl") {
            entry.insert("lvl".into(), parse_numbering_level(lvl));
        }
        if !entry.is_empty() {
            level_overrides.insert(level.to_string(), Value::Object(entry));
        }
    }
    let mut out = Map::new();
    out.insert("id".into(), json!(id));
    out.insert("abstractNumId".into(), json!(abstract_num_id));
    if !level_overrides.is_empty() {
        out.insert("levelOverrides".into(), Value::Object(level_overrides));
    }
    Some(Value::Object(out))
}

pub fn parse_numbering_level(lvl: &Element) -> Value {
    let level = parse_integer(get_w(Some(lvl), "ilvl").as_deref()).unwrap_or(0);
    let start = parse_integer(get_w(first_w(Some(lvl), "start"), "val").as_deref()).unwrap_or(1);
    let num_fmt_raw = get_w(first_w(Some(lvl), "numFmt"), "val");
    let num_fmt = num_fmt_raw
        .filter(|v| is_st_number_format(v))
        .unwrap_or_else(|| "decimal".to_owned());
    let lvl_text = get_w(first_w(Some(lvl), "lvlText"), "val").unwrap_or_default();
    let mut result = Map::new();
    result.insert("level".into(), json!(level));
    result.insert("start".into(), json!(start));
    result.insert("numFmt".into(), json!(num_fmt));
    result.insert("lvlText".into(), json!(lvl_text));
    if first_w(Some(lvl), "start").is_none() {
        result.insert("startWasOmitted".into(), json!(true));
    }
    insert_str(
        &mut result,
        "paragraphStyleId",
        get_w(first_w(Some(lvl), "pStyle"), "val"),
    );
    let jc = get_w(first_w(Some(lvl), "lvlJc"), "val");
    if matches!(jc.as_deref(), Some("left") | Some("center") | Some("right")) {
        insert_str(&mut result, "lvlJc", jc);
    }
    let ind = first_w(first_w(Some(lvl), "pPr"), "ind");
    let left = get_w(ind, "left").or_else(|| get_w(ind, "start"));
    if left.is_some() {
        insert_i64(
            &mut result,
            "indentLeftTwips",
            parse_signed_twips(left.as_deref()).or(Some(0)),
        );
    }
    if let Some(h) = get_w(ind, "hanging") {
        insert_i64(
            &mut result,
            "hangingTwips",
            parse_twips(Some(&h)).map(|n| n as i64).or(Some(0)),
        );
    }
    if let Some(fl) = get_w(ind, "firstLine") {
        insert_i64(
            &mut result,
            "firstLineTwips",
            parse_twips(Some(&fl)).map(|n| n as i64).or(Some(0)),
        );
    }
    if let Some(tabs) = first_w(first_w(Some(lvl), "pPr"), "tabs") {
        let tab = children_w(tabs, "tab")
            .into_iter()
            .find(|t| get_w(Some(t), "val").as_deref() != Some("clear"));
        if let Some(tab) = tab {
            if let Some(pos) = parse_twips(get_w(Some(tab), "pos").as_deref()) {
                insert_i64(&mut result, "tabStopTwips", Some(pos as i64));
            }
        }
    }
    if on_off_element(first_w(Some(lvl), "isLgl")) == Some(true) {
        result.insert("isLgl".into(), json!(true));
    }
    if let Some(restart) = get_w(first_w(Some(lvl), "lvlRestart"), "val") {
        insert_i64(
            &mut result,
            "lvlRestart",
            Some(parse_integer(Some(&restart)).unwrap_or(0)),
        );
    }
    let suff = get_w(first_w(Some(lvl), "suff"), "val");
    let suffix = match suff.as_deref() {
        Some("space") => "space",
        Some("nothing") => "none",
        _ => "tab",
    };
    result.insert("suffix".into(), json!(suffix));
    if let Some(marker) = parse_marker_format(first_w(Some(lvl), "rPr")) {
        result.insert("markerFormat".into(), marker);
    }
    Value::Object(result)
}

fn parse_marker_format(r_pr: Option<&Element>) -> Option<Value> {
    let r_pr = r_pr?;
    let mut format = Map::new();
    insert_str(
        &mut format,
        "fontFamily",
        get_w(first_w(Some(r_pr), "rFonts"), "ascii"),
    );
    if let Some(size) = parse_integer(get_w(first_w(Some(r_pr), "sz"), "val").as_deref()) {
        if size > 0 {
            format.insert("fontSizeHalfPoints".into(), json!(size));
        }
    }
    if on_off_element(first_w(Some(r_pr), "b")) == Some(true) {
        format.insert("bold".into(), json!(true));
    }
    if on_off_element(first_w(Some(r_pr), "i")) == Some(true) {
        format.insert("italic".into(), json!(true));
    }
    let color = get_w(first_w(Some(r_pr), "color"), "val");
    if let Some(c) = color {
        if c.len() == 6 && c.chars().all(|ch| ch.is_ascii_hexdigit()) {
            format.insert("color".into(), json!(format!("#{}", c.to_lowercase())));
        }
    }
    if format.is_empty() {
        None
    } else {
        Some(Value::Object(format))
    }
}

pub fn resolve_numbering_level(catalog: &Value, num_id: &str, level: i64) -> Option<Value> {
    let nums = catalog.get("nums").and_then(|v| v.as_object())?;
    let num = nums.get(num_id)?;
    let abstract_id = num.get("abstractNumId").and_then(|v| v.as_str())?;
    let abstract_nums = catalog.get("abstractNums").and_then(|v| v.as_object())?;
    let abstract_def = abstract_nums.get(abstract_id)?;
    let levels = abstract_def.get("levels").and_then(|v| v.as_object())?;
    let level_override = num
        .get("levelOverrides")
        .and_then(|v| v.as_object())
        .and_then(|m| m.get(&level.to_string()));
    let source_abstract = levels.get(&level.to_string());
    let abstract_level = source_abstract.cloned();
    let abstract_adj = abstract_level.as_ref().and_then(|a| {
        if a.get("startWasOmitted") == Some(&json!(true)) && a.get("start") == Some(&json!(1)) {
            let mut copy = object_to_map(a.clone());
            copy.insert("start".into(), json!(0));
            Some(Value::Object(copy))
        } else {
            abstract_level.clone()
        }
    });
    let abstract_adj_for_restart = abstract_adj.clone();
    let mut base = level_override
        .and_then(|o| o.get("lvl"))
        .cloned()
        .or(abstract_adj)?;
    if let Some(ov) = level_override {
        if ov.get("lvl").is_some() {
            let abstract_lr = abstract_adj_for_restart
                .as_ref()
                .and_then(|a| a.get("lvlRestart"))
                .cloned();
            let mut props = object_to_map(base.clone());
            props.remove("lvlRestart");
            if let Some(lr) = abstract_lr {
                props.insert("lvlRestart".into(), lr);
            }
            base = Value::Object(props);
        }
    }
    let start_override = level_override
        .and_then(|o| o.get("startOverride"))
        .and_then(|v| v.as_i64());
    let mut start = start_override
        .or_else(|| base.get("start").and_then(|v| v.as_i64()))
        .unwrap_or(1);
    if let Some(ov) = level_override {
        if ov.get("lvl").is_some() {
            let abstract_lr = abstract_adj_for_restart
                .as_ref()
                .and_then(|a| a.get("lvlRestart"))
                .and_then(|v| v.as_i64());
            let ov_lvl = ov.get("lvl").unwrap();
            if abstract_lr == Some(0) && start_override.is_none() {
                start = abstract_adj_for_restart
                    .as_ref()
                    .and_then(|a| a.get("start"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(start);
            } else if ov_lvl.get("startWasOmitted") != Some(&json!(true))
                || ov_lvl.get("start") != Some(&json!(1))
            {
                start = ov_lvl
                    .get("start")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(start);
            } else {
                start = start_override.unwrap_or(0);
            }
        }
    }
    let base_start = base.get("start").and_then(|v| v.as_i64()).unwrap_or(1);
    if start != base_start {
        let mut copy = object_to_map(base);
        copy.insert("start".into(), json!(start));
        Some(Value::Object(copy))
    } else {
        Some(base)
    }
}

pub fn format_list_number(num_fmt: &str, value: i64) -> String {
    match num_fmt {
        "decimal" => value.to_string(),
        "decimalZero" => {
            if (0..10).contains(&value) {
                format!("0{}", value)
            } else {
                value.to_string()
            }
        }
        "hex" => {
            if (0..=65535).contains(&value) {
                format!("{:X}", value)
            } else {
                String::new()
            }
        }
        "upperRoman" => roman_numeral(value),
        "lowerRoman" => roman_numeral(value).to_lowercase(),
        "upperLetter" => letter_label(value).to_uppercase(),
        "lowerLetter" => letter_label(value),
        "ordinal" => ordinal_numeral(value),
        "ordinalText" => ordinal_words(value),
        "cardinalText" => cardinal_words(value),
        "none" | "bullet" => String::new(),
        _ => value.to_string(),
    }
}

pub fn resolve_paragraph_numbering(
    paragraph: &ParagraphProps,
    style_catalog: Option<&Value>,
    numbering_catalog: Option<&Value>,
) -> Option<(i64, i64)> {
    if let Some((num_id, level)) = paragraph.numbering {
        return if num_id > 0 {
            Some((num_id, level))
        } else {
            None
        };
    }
    let style_catalog = style_catalog?;
    let inherited = resolve_style_numbering(paragraph.style.as_deref(), style_catalog)?;
    if inherited.0 <= 0 {
        return None;
    }
    if let Some(cat) = numbering_catalog {
        if paragraph.style.is_some() {
            for level in 0..9 {
                let def = resolve_numbering_level(cat, &inherited.0.to_string(), level);
                if def
                    .as_ref()
                    .and_then(|d| d.get("paragraphStyleId"))
                    .and_then(|v| v.as_str())
                    == paragraph.style.as_deref()
                {
                    return Some((inherited.0, level));
                }
            }
        }
    }
    Some(inherited)
}

pub fn compute_list_labels(
    paragraphs: &[ParagraphProps],
    style_catalog: Option<&Value>,
    numbering_catalog: Option<&Value>,
) -> BTreeMap<String, Value> {
    let mut labels = BTreeMap::new();
    let Some(catalog) = numbering_catalog else {
        return labels;
    };
    let mut counters = NumberingCounters::new();
    for paragraph in paragraphs {
        let numbering = resolve_paragraph_numbering(paragraph, style_catalog, Some(catalog));
        if numbering.is_none() {
            continue;
        }
        let (num_id, level) = numbering.unwrap();
        let num_id_s = num_id.to_string();
        let level = level.clamp(0, 8);
        let def = resolve_numbering_level(catalog, &num_id_s, level);
        if def.is_none() {
            continue;
        }
        let def = def.unwrap();
        if def.get("numFmt").and_then(|v| v.as_str()) == Some("none") {
            continue;
        }
        counters.advance(&num_id_s, level, &def, catalog);
        let text = if def.get("numFmt").and_then(|v| v.as_str()) == Some("bullet") {
            def.get("lvlText")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned()
        } else {
            substitute_lvl_text(
                def.get("lvlText").and_then(|v| v.as_str()).unwrap_or(""),
                &num_id_s,
                level,
                catalog,
                &counters,
                def.get("isLgl") == Some(&json!(true)),
            )
        };
        let mut label = Map::new();
        label.insert("numId".into(), json!(num_id_s));
        label.insert("level".into(), json!(level));
        label.insert("text".into(), json!(text));
        label.insert(
            "suffix".into(),
            json!(def.get("suffix").and_then(|v| v.as_str()).unwrap_or("tab")),
        );
        for key in [
            "alignment",
            "indentLeftTwips",
            "hangingTwips",
            "firstLineTwips",
            "markerFormat",
            "tabStopTwips",
        ] {
            let json_key = if key == "alignment" { "lvlJc" } else { key };
            if let Some(v) = def.get(json_key) {
                let out_key = if key == "alignment" { "alignment" } else { key };
                label.insert(out_key.into(), v.clone());
            }
        }
        labels.insert(paragraph.id.clone(), Value::Object(label));
    }
    labels
}

pub fn display_list_label(text: &str) -> String {
    text.chars()
        .map(|c| {
            if ('\u{f000}'..='\u{f0ff}').contains(&c) {
                match c {
                    '\u{f0b7}' => '•',
                    '\u{f0a7}' => '▪',
                    '\u{f0d8}' => '➢',
                    '\u{f076}' => '❖',
                    '\u{f0fc}' => '✓',
                    '\u{f0fe}' => '■',
                    '\u{f071}' => '❑',
                    '\u{f0e0}' => '➩',
                    '\u{f0b2}' => '○',
                    _ => '•',
                }
            } else {
                c
            }
        })
        .collect()
}

struct NumberingCounters {
    counts: HashMap<String, HashMap<i64, i64>>,
    started: HashMap<String, HashSet<i64>>,
}

impl NumberingCounters {
    fn new() -> Self {
        Self {
            counts: HashMap::new(),
            started: HashMap::new(),
        }
    }
}

impl NumberingCounters {
    fn advance(&mut self, num_id: &str, level: i64, def: &Value, catalog: &Value) -> i64 {
        let counts = self.counts.entry(num_id.to_owned()).or_default();
        let started = self.started.entry(num_id.to_owned()).or_default();
        let def_start = def.get("start").and_then(|v| v.as_i64()).unwrap_or(1);
        let next = if started.contains(&level) {
            counts.get(&level).unwrap_or(&def_start) + 1
        } else {
            def_start
        };
        counts.insert(level, next);
        started.insert(level);
        let deeper_started: Vec<i64> = started.iter().copied().collect();
        for deeper in deeper_started {
            if deeper <= level {
                continue;
            }
            let deeper_def = resolve_numbering_level(catalog, num_id, deeper);
            let restart = deeper_def
                .as_ref()
                .and_then(|d| d.get("lvlRestart"))
                .and_then(|v| v.as_i64());
            if restart == Some(0) {
                continue;
            }
            let trigger = if restart.unwrap_or(0) > 0 && restart.unwrap_or(0) <= deeper {
                restart.unwrap_or(0) - 1
            } else {
                deeper - 1
            };
            if level > trigger {
                continue;
            }
            counts.remove(&deeper);
            started.remove(&deeper);
        }
        for ancestor in 0..level {
            if started.contains(&ancestor) {
                continue;
            }
            let ancestor_def = resolve_numbering_level(catalog, num_id, ancestor);
            if ancestor_def.is_none() {
                continue;
            }
            let a_start = ancestor_def
                .as_ref()
                .and_then(|d| d.get("start"))
                .and_then(|v| v.as_i64())
                .unwrap_or(1);
            counts.insert(ancestor, a_start);
            started.insert(ancestor);
        }
        next
    }

    fn current(&self, num_id: &str, level: i64, start: i64) -> i64 {
        self.counts
            .get(num_id)
            .and_then(|m| m.get(&level))
            .copied()
            .unwrap_or(start)
    }
}

fn substitute_lvl_text(
    lvl_text: &str,
    num_id: &str,
    level: i64,
    catalog: &Value,
    counters: &NumberingCounters,
    force_decimal: bool,
) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < lvl_text.len() {
        if lvl_text[i..].starts_with('%') && i + 1 < lvl_text.len() {
            let digit = lvl_text[i + 1..i + 2].parse::<u8>();
            if let Ok(d) = digit {
                if (1..=9).contains(&d) {
                    let ancestor_level = (d as i64) - 1;
                    if ancestor_level > level {
                        out.push('%');
                        out.push((b'0' + d) as char);
                    } else {
                        let ancestor_def = resolve_numbering_level(catalog, num_id, ancestor_level);
                        let start = ancestor_def
                            .as_ref()
                            .and_then(|d| d.get("start"))
                            .and_then(|v| v.as_i64())
                            .unwrap_or(1);
                        let value = counters.current(num_id, ancestor_level, start);
                        let fmt = if force_decimal {
                            "decimal"
                        } else {
                            ancestor_def
                                .as_ref()
                                .and_then(|d| d.get("numFmt"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("decimal")
                        };
                        out.push_str(&format_list_number(fmt, value));
                    }
                    i += 2;
                    continue;
                }
            }
        }
        let ch = lvl_text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn roman_numeral(n: i64) -> String {
    let values = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1];
    let numerals = [
        "M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I",
    ];
    let mut remaining = n.clamp(1, 3999);
    let mut result = String::new();
    for (i, value) in values.iter().enumerate() {
        while remaining >= *value {
            result.push_str(numerals[i]);
            remaining -= *value;
        }
    }
    result
}

fn letter_label(n: i64) -> String {
    let index = (n.max(1) - 1) as usize;
    let letter = (b'a' + (index % 26) as u8) as char;
    letter.to_string().repeat(index / 26 + 1)
}

fn ordinal_numeral(n: i64) -> String {
    format!("{}{}", n, ordinal_suffix(n))
}

fn ordinal_suffix(n: i64) -> &'static str {
    let abs = n.abs();
    if abs % 100 >= 11 && abs % 100 <= 13 {
        return "th";
    }
    match abs % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}

fn cardinal_words(n: i64) -> String {
    if !(0..=999_999).contains(&n) {
        return n.to_string();
    }
    if n == 0 {
        return "zero".to_owned();
    }
    const ONES: [&str; 20] = [
        "",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 10] = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    fn chunk(v: i64, ones: &[&str], tens: &[&str]) -> String {
        let mut parts = Vec::new();
        let mut v = v;
        if v >= 100 {
            parts.push(ones[(v / 100) as usize].to_owned());
            parts.push("hundred".to_owned());
            v %= 100;
        }
        if v >= 20 {
            let mut s = tens[(v / 10) as usize].to_owned();
            if v % 10 != 0 {
                s.push('-');
                s.push_str(ones[(v % 10) as usize]);
            }
            parts.push(s);
        } else if v > 0 {
            parts.push(ones[v as usize].to_owned());
        }
        parts.join(" ")
    }
    let thousands = n / 1000;
    let rest = n % 1000;
    let mut words = Vec::new();
    if thousands > 0 {
        words.push(chunk(thousands, &ONES, &TENS));
        words.push("thousand".to_owned());
    }
    if rest > 0 || thousands == 0 {
        words.push(chunk(rest, &ONES, &TENS));
    }
    words
        .into_iter()
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn ordinal_words(n: i64) -> String {
    if !(0..=999_999).contains(&n) {
        return ordinal_numeral(n);
    }
    let words = cardinal_words(n);
    let split_at = words.rfind(' ').max(words.rfind('-')).unwrap_or(0);
    let (head, last_word) = if split_at > 0 {
        (
            words[..split_at + 1].to_owned(),
            words[split_at + 1..].to_owned(),
        )
    } else {
        (String::new(), words)
    };
    const ORD: [(&str, &str); 7] = [
        ("one", "first"),
        ("two", "second"),
        ("three", "third"),
        ("five", "fifth"),
        ("eight", "eighth"),
        ("nine", "ninth"),
        ("twelve", "twelfth"),
    ];
    for (w, o) in ORD {
        if last_word == w {
            return head + o;
        }
    }
    if last_word.ends_with('y') {
        return format!(
            "{}{}ieth",
            head,
            last_word[..last_word.len() - 1].to_owned()
        );
    }
    format!("{}{}th", head, last_word)
}

fn object_to_map(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn dedupe(mut warnings: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    warnings.retain(|w| seen.insert(w.clone()));
    warnings
}
