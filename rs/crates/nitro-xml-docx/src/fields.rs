// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/field-runs.ts and field-date.ts; changes listed in PROVENANCE.md.

pub fn field_name(instr: &str) -> String {
    instr
        .trim()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_uppercase()
}

pub fn is_field_locked(run: &serde_json::Value) -> bool {
    run.get("fieldFlags")
        .and_then(|f| f.get("locked"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn date_picture(instr: &str) -> Option<String> {
    if let Some(idx) = instr.find("\\@") {
        let tail = instr[idx + 2..].trim();
        if tail.starts_with('"') {
            let end = tail[1..].find('"')?;
            return Some(tail[1..1 + end].to_string());
        }
        return tail.split_whitespace().next().map(|s| s.to_string());
    }
    None
}

pub fn date_field_result(
    name: &str,
    instr: &str,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
) -> String {
    let picture = date_picture(instr).unwrap_or_else(|| {
        if name == "DATE" {
            "M/d/yyyy".into()
        } else {
            "h:mm am/pm".into()
        }
    });
    format_word_date(year, month, day, hour, minute, &picture)
}

fn format_word_date(year: i32, month: u32, day: u32, hour: u32, minute: u32, picture: &str) -> String {
    let h12 = hour % 12;
    let h12 = if h12 == 0 { 12 } else { h12 };
    let mut out = String::new();
    let mut i = 0usize;
    while i < picture.len() {
        let rest = &picture[i..];
        if rest.starts_with("yyyy") {
            out.push_str(&year.to_string());
            i += 4;
        } else if rest.starts_with("yy") {
            out.push_str(&format!("{:02}", year % 100));
            i += 2;
        } else if rest.starts_with("MM") {
            out.push_str(&format!("{:02}", month));
            i += 2;
        } else if rest.starts_with("dd") {
            out.push_str(&format!("{:02}", day));
            i += 2;
        } else if rest.starts_with("HH") {
            out.push_str(&format!("{:02}", hour));
            i += 2;
        } else if rest.starts_with("hh") {
            out.push_str(&format!("{:02}", h12));
            i += 2;
        } else if rest.starts_with("mm") {
            out.push_str(&format!("{:02}", minute));
            i += 2;
        } else if rest.starts_with("am/pm") {
            out.push_str(if hour < 12 { "am" } else { "pm" });
            i += 5;
        } else if rest.starts_with('M') {
            out.push_str(&month.to_string());
            i += 1;
        } else if rest.starts_with('d') {
            out.push_str(&day.to_string());
            i += 1;
        } else if rest.starts_with('H') {
            out.push_str(&hour.to_string());
            i += 1;
        } else if rest.starts_with('h') {
            out.push_str(&h12.to_string());
            i += 1;
        } else if rest.starts_with('\'') {
            let end = rest[1..].find('\'').map(|p| p + 2).unwrap_or(1);
            out.push_str(&rest[1..end - 1]);
            i += end;
        } else {
            out.push(picture.as_bytes()[i] as char);
            i += 1;
        }
    }
    out
}
