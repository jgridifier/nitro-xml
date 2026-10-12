// SPDX-License-Identifier: MIT
//! Find/replace over [`DocxHost`] paragraph plain text (#83).

use serde_json::Map;

use crate::edit::host::DocxHost;
use crate::edit::text::set_paragraph_text;

/// A match inside one top-level paragraph block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub block_index: usize,
    /// UTF-8 byte range in the paragraph's plain text (concatenated runs).
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SearchStatus {
    pub query: String,
    pub count: usize,
    pub active: usize,
    pub read_only: bool,
}

#[derive(Debug, Clone)]
pub struct SearchController {
    query: String,
    case_sensitive: bool,
    matches: Vec<SearchMatch>,
    active_index: isize,
    read_only: bool,
}

impl SearchController {
    pub fn new(read_only: bool) -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            matches: Vec::new(),
            active_index: -1,
            read_only,
        }
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    pub fn status(&self) -> SearchStatus {
        SearchStatus {
            query: self.query.clone(),
            count: self.matches.len(),
            active: if self.active_index < 0 {
                0
            } else {
                (self.active_index as usize) + 1
            },
            read_only: self.read_only,
        }
    }

    pub fn search(&mut self, host: &DocxHost, query: &str, case_sensitive: bool) -> SearchStatus {
        self.query = query.to_string();
        self.case_sensitive = case_sensitive;
        self.matches = collect_matches(host, &self.query, self.case_sensitive);
        self.active_index = -1;
        self.status()
    }

    pub fn refresh(&mut self, host: &DocxHost) -> SearchStatus {
        self.matches = collect_matches(host, &self.query, self.case_sensitive);
        if self.active_index >= self.matches.len() as isize {
            self.active_index = -1;
        }
        self.status()
    }

    pub fn active_match(&self) -> Option<&SearchMatch> {
        if self.active_index < 0 {
            return None;
        }
        self.matches.get(self.active_index as usize)
    }

    pub fn find_next(&mut self, host: &DocxHost, cursor: Option<usize>) -> Option<SearchMatch> {
        self.refresh(host);
        if self.matches.is_empty() {
            return None;
        }
        let next = if self.active_index < 0 {
            let cursor = cursor.unwrap_or(0);
            self.matches
                .iter()
                .position(|m| m.start >= cursor)
                .unwrap_or(0)
        } else {
            (self.active_index as usize + 1) % self.matches.len()
        };
        self.active_index = next as isize;
        self.matches.get(next).cloned()
    }

    pub fn find_previous(&mut self, host: &DocxHost, cursor: Option<usize>) -> Option<SearchMatch> {
        self.refresh(host);
        if self.matches.is_empty() {
            return None;
        }
        let prev = if self.active_index < 0 {
            let cursor = cursor.unwrap_or(usize::MAX);
            self.matches
                .iter()
                .rposition(|m| m.start < cursor)
                .unwrap_or(self.matches.len() - 1)
        } else {
            let n = self.matches.len();
            (self.active_index as usize + n - 1) % n
        };
        self.active_index = prev as isize;
        self.matches.get(prev).cloned()
    }

    /// Replace the active match. Returns false when readonly, empty query, or no match.
    pub fn replace_one(
        &mut self,
        host: &mut DocxHost,
        replacement: &str,
        cursor: Option<usize>,
    ) -> bool {
        if self.read_only || self.query.is_empty() {
            return false;
        }
        if self.active_index < 0 && self.find_next(host, cursor).is_none() {
            return false;
        }
        let m = self
            .active_match()
            .cloned()
            .expect("active after find_next");
        if !apply_replacement(host, &m, replacement) {
            return false;
        }
        self.refresh(host);
        if self.matches.is_empty() {
            self.active_index = -1;
        } else {
            let idx = (self.active_index as usize).min(self.matches.len() - 1);
            self.active_index = idx as isize;
        }
        true
    }

    /// Replace every match. Returns count replaced (0 when readonly or empty query).
    pub fn replace_all(&mut self, host: &mut DocxHost, replacement: &str) -> usize {
        if self.read_only || self.query.is_empty() {
            return 0;
        }
        self.refresh(host);
        if self.matches.is_empty() {
            return 0;
        }
        let targets = self.matches.clone();
        let mut count = 0usize;
        for m in targets.into_iter().rev() {
            if apply_replacement(host, &m, replacement) {
                count += 1;
            }
        }
        self.refresh(host);
        self.active_index = -1;
        count
    }
}

pub fn paragraph_plain_text(block: &serde_json::Value) -> String {
    block
        .get("runs")
        .and_then(|r| r.as_array())
        .map(|rs| {
            rs.iter()
                .map(|r| r.get("text").and_then(|t| t.as_str()).unwrap_or(""))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn collect_matches(host: &DocxHost, query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (bi, block) in host.blocks.iter().enumerate() {
        if block.get("type").and_then(|t| t.as_str()) != Some("paragraph") {
            continue;
        }
        let text = paragraph_plain_text(block);
        let hay = if case_sensitive {
            text.clone()
        } else {
            text.to_lowercase()
        };
        let needle = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };
        if needle.is_empty() {
            continue;
        }
        let mut start = 0usize;
        while let Some(rel) = hay[start..].find(&needle) {
            let s = start + rel;
            let e = s + needle.len();
            if text.is_char_boundary(s) && text.is_char_boundary(e) {
                out.push(SearchMatch {
                    block_index: bi,
                    start: s,
                    end: e,
                });
            }
            start = s + 1;
            if start >= hay.len() {
                break;
            }
        }
    }
    out
}

fn run_attrs_at_offset(block: &serde_json::Value, offset: usize) -> Map<String, serde_json::Value> {
    let mut pos = 0usize;
    let runs = block
        .get("runs")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    for run in runs {
        let t = run.get("text").and_then(|x| x.as_str()).unwrap_or("");
        let end = pos + t.len();
        if offset <= end {
            let mut attrs = Map::new();
            for key in ["bold", "italic", "fontSize", "color", "underline"] {
                if let Some(v) = run.get(key) {
                    attrs.insert(key.into(), v.clone());
                }
            }
            return attrs;
        }
        pos = end;
    }
    Map::new()
}

fn apply_replacement(host: &mut DocxHost, m: &SearchMatch, replacement: &str) -> bool {
    let block = match host.paragraph(m.block_index) {
        Ok(b) => b.clone(),
        Err(_) => return false,
    };
    let text = paragraph_plain_text(&block);
    if m.end > text.len() || !text.is_char_boundary(m.start) || !text.is_char_boundary(m.end) {
        return false;
    }
    let attrs = run_attrs_at_offset(&block, m.start);
    let mut new_text = String::new();
    new_text.push_str(&text[..m.start]);
    new_text.push_str(replacement);
    new_text.push_str(&text[m.end..]);
    set_paragraph_text(host, m.block_index, &new_text, &attrs).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn host_with_para(text: &str) -> DocxHost {
        DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": text }],
        })])
    }

    #[test]
    fn catches_empty_query_collects_no_matches() {
        // Catches: find/replace running with an empty query instead of no-op (#83).
        let host = host_with_para("hello");
        let mut search = SearchController::new(false);
        let st = search.search(&host, "", false);
        assert_eq!(st.count, 0);
        assert!(!search.replace_one(&mut host_with_para("hello"), "x", None));
        assert_eq!(search.replace_all(&mut host_with_para("hello"), "x"), 0);
    }

    #[test]
    fn catches_readonly_replace_is_no_op() {
        // Catches: replace mutating the model while the editor is read-only (#83).
        let mut host = host_with_para("foo bar foo");
        let mut search = SearchController::new(true);
        search.search(&host, "foo", false);
        assert_eq!(search.replace_all(&mut host, "baz"), 0);
        assert_eq!(paragraph_plain_text(&host.blocks[0]), "foo bar foo");
    }

    #[test]
    fn catches_replace_all_on_no_matches() {
        // Catches: replaceAll reporting success when the query matches nothing (#83).
        let mut host = host_with_para("hello");
        let mut search = SearchController::new(false);
        search.search(&host, "zzz", false);
        assert_eq!(search.replace_all(&mut host, "a"), 0);
    }
}
