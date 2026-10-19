// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/sheet-data-source.ts; changes listed in
// PROVENANCE.md.
//! Split `<sheetData>` out of a worksheet part for lightweight cell parsing.

/// Part with emptied `<sheetData>` and the cell XML slice.
pub struct SplitSheetData {
    pub shell_xml: String,
    pub sheet_data_inner: String,
    pub sheet_data_tag: String,
}

/// Shell XML plus byte range of `<sheetData>` inner content (no inner clone).
pub struct SheetDataBounds {
    pub shell_xml: String,
    pub inner_start: usize,
    pub inner_end: usize,
    pub sheet_data_tag: String,
}

/// Locates `<sheetData>` without copying the cell XML, or `None` when the whole part must be parsed.
pub fn sheet_data_bounds(xml: &str) -> Option<SheetDataBounds> {
    if xml.contains("<!--") || xml.contains("<![CDATA[") {
        return None;
    }
    let _namespaces = root_namespaces(xml);
    let open = find_sheet_data_open(xml)?;
    let prefix = open.prefix;
    let tag_name = format!("{prefix}sheetData");
    let open_end = xml[open.start..]
        .find('>')
        .map(|i| open.start + i)
        .filter(|&i| i < xml.len())?;
    let attributes = &xml[open.start + tag_name.len() + 1..open_end];
    if attributes.trim_end().ends_with('/') || attributes.contains("xmlns") {
        return None;
    }
    let close_tag = format!("</{tag_name}");
    let close = xml[open_end..].find(&close_tag).map(|i| open_end + i)?;
    let inner_start = open_end + 1;
    if xml[inner_start..close].contains("<sheetData") {
        return None;
    }
    let shell_xml = format!("{}{}", &xml[..open_end + 1], &xml[close..]);
    Some(SheetDataBounds {
        shell_xml,
        inner_start,
        inner_end: close,
        sheet_data_tag: tag_name,
    })
}

/// Splits cell data from a worksheet part, or `None` when the whole part must be parsed.
pub fn split_sheet_data(xml: &str) -> Option<SplitSheetData> {
    let bounds = sheet_data_bounds(xml)?;
    Some(SplitSheetData {
        shell_xml: bounds.shell_xml,
        sheet_data_inner: xml[bounds.inner_start..bounds.inner_end].to_owned(),
        sheet_data_tag: bounds.sheet_data_tag,
    })
}

struct OpenMatch {
    start: usize,
    prefix: String,
}

fn find_sheet_data_open(xml: &str) -> Option<OpenMatch> {
    let needle = "sheetData";
    let mut search_from = 0;
    while let Some(idx) = xml[search_from..].find(needle) {
        let at = search_from + idx;
        let before = xml[..at].rfind('<').unwrap_or(0);
        let tag_start = before;
        let slice = &xml[tag_start..];
        if !slice.starts_with('<') {
            search_from = at + needle.len();
            continue;
        }
        let after_lt = &slice[1..];
        let colon = after_lt.find(':');
        let local_start = if let Some(c) = colon { c + 1 } else { 0 };
        if after_lt[local_start..].starts_with(needle) {
            let prefix = match colon {
                Some(c) => after_lt[..c + 1].to_owned(),
                None => String::new(),
            };
            let idx = local_start + needle.len();
            let next = after_lt.chars().nth(idx);
            if next == Some(' ') || next == Some('/') || next == Some('>') {
                return Some(OpenMatch {
                    start: tag_start,
                    prefix,
                });
            }
        }
        search_from = at + needle.len();
    }
    None
}

fn root_namespaces(xml: &str) -> Option<std::collections::HashMap<String, String>> {
    let head = xml.chars().take(8192).collect::<String>();
    let re_start = head.find('<').unwrap_or(0);
    let rest = &head[re_start..];
    if !rest.starts_with('<') {
        return None;
    }
    let end = rest.find('>').unwrap_or(0);
    let tag = &rest[..end];
    if !tag.contains("xmlns") {
        return Some(std::collections::HashMap::new());
    }
    let mut out = std::collections::HashMap::new();
    let mut i = 0;
    let bytes = tag.as_bytes();
    while i < bytes.len() {
        if bytes[i..].starts_with(b"xmlns") {
            let mut j = i + 5;
            let prefix = if j < bytes.len() && bytes[j] == b':' {
                j += 1;
                let start = j;
                while j < bytes.len() && bytes[j] != b'=' && bytes[j] != b' ' {
                    j += 1;
                }
                tag[start..j].to_owned()
            } else {
                String::new()
            };
            while j < bytes.len() && bytes[j] != b'=' {
                j += 1;
            }
            if j >= bytes.len() || bytes[j] != b'=' {
                break;
            }
            j += 1;
            let quote = bytes.get(j).copied();
            if quote != Some(b'"') && quote != Some(b'\'') {
                break;
            }
            j += 1;
            let start = j;
            while j < bytes.len() && bytes[j] != quote.unwrap() {
                j += 1;
            }
            out.insert(prefix, tag[start..j].to_owned());
            i = j + 1;
        } else {
            i += 1;
        }
    }
    Some(out)
}
