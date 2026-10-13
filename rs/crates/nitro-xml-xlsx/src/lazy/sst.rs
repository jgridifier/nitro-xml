// SPDX-License-Identifier: MIT
//! On-demand shared-string resolution. The table is inflated only through the highest index the
//! viewport needs, and only those `<si>` entries are materialised.

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::opc::Package;
use nitro_xml_opc::xml::parse_xml_str;

use crate::model::SharedString;
use crate::shared_strings::parse_rich_string;
use crate::xml_util::x_children;

pub(crate) fn collect_shared_indices(row_xml: &str, out: &mut HashSet<usize>) {
    let bytes = row_xml.as_bytes();
    let mut i = 0;
    while i + 4 < bytes.len() {
        if is_shared_type(bytes, i) {
            if let Some(value) = value_text(row_xml, i) {
                if let Ok(idx) = value.trim().parse::<usize>() {
                    out.insert(idx);
                }
            }
        }
        i += 1;
    }
}

fn is_shared_type(bytes: &[u8], i: usize) -> bool {
    let Some(q) = bytes.get(i + 2).copied() else {
        return false;
    };
    bytes[i] == b't'
        && bytes[i + 1] == b'='
        && (q == b'"' || q == b'\'')
        && bytes.get(i + 3) == Some(&b's')
        && bytes.get(i + 4) == Some(&q)
}

fn value_text(xml: &str, from: usize) -> Option<&str> {
    let v_rel = xml[from..].find("<v>")?;
    let start = from + v_rel + 3;
    let end_rel = xml[start..].find("</v>")?;
    Some(&xml[start..start + end_rel])
}

/// Rewrite `t="s"` cell indexes to a compact table so the worksheet reader can stay index-based
/// without allocating a slot per unused string.
pub(crate) fn rewrite_shared_indices(xml: &str, map: &HashMap<usize, usize>) -> String {
    let bytes = xml.as_bytes();
    let mut out = String::with_capacity(xml.len());
    let mut i = 0;
    while i < bytes.len() {
        if is_shared_type(bytes, i) {
            if let Some(v_rel) = xml[i..].find("<v>") {
                let v_start = i + v_rel + 3;
                if let Some(end_rel) = xml[v_start..].find("</v>") {
                    let v_end = v_start + end_rel;
                    out.push_str(&xml[i..v_start]);
                    let token = xml[v_start..v_end].trim();
                    if let Ok(idx) = token.parse::<usize>() {
                        if let Some(mapped) = map.get(&idx) {
                            out.push_str(&mapped.to_string());
                        } else {
                            out.push_str(token);
                        }
                    } else {
                        out.push_str(&xml[v_start..v_end]);
                    }
                    i = v_end;
                    continue;
                }
            }
        }
        let ch = xml[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8().max(1);
    }
    out
}

pub(crate) fn resolve_shared_strings(
    pkg: &mut Package,
    part: &str,
    needed: &HashSet<usize>,
    palette: Option<&[String]>,
) -> Result<HashMap<usize, SharedString>, String> {
    if needed.is_empty() {
        return Ok(HashMap::new());
    }
    let max_needed = needed.iter().copied().max().unwrap_or(0);
    let mut parser = SstParser {
        needed,
        palette,
        max_needed,
        index: 0,
        out: HashMap::new(),
        text: String::new(),
        pending: Vec::new(),
        done: false,
        bom_checked: false,
    };
    pkg.stream_part(part, |chunk| {
        parser.push_bytes(chunk);
        !parser.done
    })
    .map_err(|e| e.to_string())?;
    parser.finish();
    Ok(parser.out)
}

struct SstParser<'a> {
    needed: &'a HashSet<usize>,
    palette: Option<&'a [String]>,
    max_needed: usize,
    index: usize,
    out: HashMap<usize, SharedString>,
    text: String,
    pending: Vec<u8>,
    done: bool,
    bom_checked: bool,
}

impl SstParser<'_> {
    fn push_bytes(&mut self, chunk: &[u8]) {
        if self.done {
            return;
        }
        self.pending.extend_from_slice(chunk);
        let valid = match std::str::from_utf8(&self.pending) {
            Ok(s) => s.len(),
            Err(e) => e.valid_up_to(),
        };
        if valid > 0 {
            let s = std::str::from_utf8(&self.pending[..valid]).unwrap_or("");
            self.text.push_str(s);
            self.pending.drain(..valid);
        }
        self.strip_bom();
        self.pump();
    }

    fn finish(&mut self) {
        if !self.pending.is_empty() {
            let lossy = String::from_utf8_lossy(&self.pending);
            self.text.push_str(&lossy);
            self.pending.clear();
            self.strip_bom();
            self.pump();
        }
    }

    fn strip_bom(&mut self) {
        if self.bom_checked || self.text.is_empty() {
            return;
        }
        self.bom_checked = true;
        if self.text.starts_with('\u{feff}') {
            self.text.replace_range(..'\u{feff}'.len_utf8(), "");
        }
    }

    fn pump(&mut self) {
        while !self.done {
            match next_si(&self.text) {
                SiScan::NeedMore => return,
                SiScan::End => {
                    self.done = true;
                    return;
                }
                SiScan::Item { start, end } => {
                    let xml = self.text[start..end].to_owned();
                    self.text = self.text[end..].to_owned();
                    if self.needed.contains(&self.index) {
                        self.out.insert(self.index, parse_si(&xml, self.palette));
                    }
                    self.index += 1;
                    if self.index > self.max_needed {
                        self.done = true;
                        return;
                    }
                }
            }
        }
    }
}

enum SiScan {
    NeedMore,
    End,
    Item { start: usize, end: usize },
}

fn next_si(text: &str) -> SiScan {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if i + 1 >= bytes.len() {
            return SiScan::NeedMore;
        }
        match tag_kind(text, i) {
            TagKind::Incomplete => return SiScan::NeedMore,
            TagKind::Close(local) if local == "sst" => return SiScan::End,
            TagKind::Open(local) if local == "si" => {
                let Some((open_end, self_close)) = end_of_tag(text, i) else {
                    return SiScan::NeedMore;
                };
                if self_close {
                    return SiScan::Item {
                        start: i,
                        end: open_end,
                    };
                }
                let Some(end) = find_close(text, open_end, "si") else {
                    return SiScan::NeedMore;
                };
                return SiScan::Item { start: i, end };
            }
            TagKind::Open(_) | TagKind::Close(_) | TagKind::Other => {
                if end_of_tag(text, i).is_none() {
                    return SiScan::NeedMore;
                }
            }
        }
        i += 1;
    }
    SiScan::NeedMore
}

fn parse_si(xml: &str, palette: Option<&[String]>) -> SharedString {
    let wrapped = format!(
        "<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">{xml}</sst>"
    );
    let Ok(doc) = parse_xml_str(&wrapped) else {
        return SharedString {
            text: String::new(),
            runs: None,
        };
    };
    match x_children(&doc.root, "si").into_iter().next() {
        Some(el) => parse_rich_string(el, palette),
        None => SharedString {
            text: String::new(),
            runs: None,
        },
    }
}

enum TagKind {
    Incomplete,
    Open(String),
    Close(String),
    Other,
}

fn tag_kind(s: &str, lt: usize) -> TagKind {
    let bytes = s.as_bytes();
    if lt >= bytes.len() || bytes[lt] != b'<' {
        return TagKind::Other;
    }
    let mut j = lt + 1;
    if j >= bytes.len() {
        return TagKind::Incomplete;
    }
    let is_close = if bytes[j] == b'/' {
        j += 1;
        true
    } else if bytes[j] == b'!' || bytes[j] == b'?' {
        return TagKind::Other;
    } else {
        false
    };
    if j >= bytes.len() {
        return TagKind::Incomplete;
    }
    let mut local_start = j;
    let mut k = j;
    while k < bytes.len() && is_name_byte(bytes[k]) {
        k += 1;
    }
    if k < bytes.len() && bytes[k] == b':' {
        local_start = k + 1;
        k = local_start;
        while k < bytes.len() && is_name_byte(bytes[k]) {
            k += 1;
        }
    }
    if k == local_start {
        return if k >= bytes.len() {
            TagKind::Incomplete
        } else {
            TagKind::Other
        };
    }
    if k >= bytes.len() {
        return TagKind::Incomplete;
    }
    let local = s[local_start..k].to_owned();
    if is_close {
        TagKind::Close(local)
    } else {
        TagKind::Open(local)
    }
}

fn is_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

fn end_of_tag(s: &str, lt: usize) -> Option<(usize, bool)> {
    let bytes = s.as_bytes();
    let mut i = lt + 1;
    let mut quote = 0u8;
    while i < bytes.len() {
        let c = bytes[i];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            let self_close = i > lt && bytes[i - 1] == b'/';
            return Some((i + 1, self_close));
        }
        i += 1;
    }
    None
}

fn find_close(s: &str, from: usize, local: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let TagKind::Close(name) = tag_kind(s, i) {
                if name == local {
                    return end_of_tag(s, i).map(|(end, _)| end);
                }
            } else if matches!(tag_kind(s, i), TagKind::Incomplete) {
                return None;
            }
        }
        i += 1;
    }
    None
}
