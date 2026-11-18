// SPDX-License-Identifier: MIT
//! Find/replace over [`DocxHost`] paragraph plain text (#83).

use serde_json::Value;

use crate::edit::host::DocxHost;

/// A match inside one top-level paragraph block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub block_index: usize,
    pub path: String,
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
    pub last_replaced: usize,
    pub whole_word: bool,
}

impl SearchController {
    pub fn new(read_only: bool) -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            matches: Vec::new(),
            active_index: -1,
            read_only,
            last_replaced: 0,
            whole_word: false,
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
        self.matches = collect_matches(host, &self.query, self.case_sensitive, self.whole_word);
        self.active_index = -1;
        self.status()
    }

    pub fn refresh(&mut self, host: &DocxHost) -> SearchStatus {
        self.matches = collect_matches(host, &self.query, self.case_sensitive, self.whole_word);
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

fn collect_matches(
    host: &DocxHost,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<SearchMatch> {
    fn visit(
        block: &Value,
        path: String,
        bi: usize,
        query: &str,
        case: bool,
        word: bool,
        out: &mut Vec<SearchMatch>,
    ) {
        if block.get("runs").and_then(Value::as_array).is_some() {
            let text = paragraph_plain_text(block);
            // Map folded character boundaries back to original UTF-8 offsets (İ expands).
            let mut hay = String::new();
            let mut boundaries = std::collections::BTreeMap::new();
            for (i, ch) in text.char_indices() {
                boundaries.insert(hay.len(), i);
                if case {
                    hay.push(ch);
                } else {
                    hay.extend(ch.to_lowercase());
                }
            }
            boundaries.insert(hay.len(), text.len());
            let needle = if case {
                query.to_owned()
            } else {
                query.to_lowercase()
            };
            if !needle.is_empty() {
                for (start, _) in hay.match_indices(&needle) {
                    if let (Some(&s), Some(&e)) = (
                        boundaries.get(&start),
                        boundaries.get(&(start + needle.len())),
                    ) {
                        let is_word = |c: char| c.is_alphanumeric() || c == '_';
                        if !word
                            || (!text[..s].chars().next_back().is_some_and(is_word)
                                && !text[e..].chars().next().is_some_and(is_word))
                        {
                            out.push(SearchMatch {
                                block_index: bi,
                                path: path.clone(),
                                start: s,
                                end: e,
                            });
                        }
                    }
                }
            }
        }
        // Table rows are arrays of cells; cells contain paragraphs, including nested tables.
        if let Some(rows) = block.get("rows").and_then(Value::as_array) {
            for (ri, row) in rows.iter().enumerate() {
                if let Some(cells) = row.as_array() {
                    for (ci, cell) in cells.iter().enumerate() {
                        if let Some(paras) = cell.get("paragraphs").and_then(Value::as_array) {
                            for (pi, para) in paras.iter().enumerate() {
                                visit(
                                    para,
                                    format!("{path}/rows/{ri}/{ci}/paragraphs/{pi}"),
                                    bi,
                                    query,
                                    case,
                                    word,
                                    out,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for (bi, block) in host.blocks.iter().enumerate() {
        visit(
            block,
            String::new(),
            bi,
            query,
            case_sensitive,
            whole_word,
            &mut out,
        );
    }
    out
}

fn apply_replacement(host: &mut DocxHost, m: &SearchMatch, replacement: &str) -> bool {
    host.ensure_base_blocks();
    let Some(block) = host
        .blocks
        .get_mut(m.block_index)
        .and_then(|b| b.pointer_mut(&m.path))
    else {
        return false;
    };
    let Some(runs) = block.get_mut("runs").and_then(Value::as_array_mut) else {
        return false;
    };
    let mut output = Vec::new();
    let mut pos = 0;
    let mut inserted = false;
    for run in runs.iter() {
        let text = run.get("text").and_then(Value::as_str).unwrap_or("");
        let end = pos + text.len();
        if end <= m.start || pos >= m.end {
            output.push(run.clone());
        } else {
            let mut piece = run.clone();
            let left = m.start.saturating_sub(pos).min(text.len());
            let right = m.end.saturating_sub(pos).min(text.len());
            let mut value = text[..left].to_owned();
            if !inserted {
                value.push_str(replacement);
                inserted = true;
            }
            value.push_str(&text[right..]);
            piece["text"] = Value::String(value);
            output.push(piece);
        }
        pos = end;
    }
    if !inserted {
        return false;
    }
    *runs = output;
    host.mark_touched(m.block_index);
    true
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
