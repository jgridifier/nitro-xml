// SPDX-License-Identifier: MIT
//! Docx Print Layout viewer state (#23).

use std::collections::BTreeMap;

use crate::events::Event;
use crate::shell::{push_text_hits, FindMatch, PendingHyperlink};
use crate::transport::StringTable;
use nitro_xml::opc::opc::hyperlink_policy;
use nitro_xml_docx::{PrintNow, PrintSession, PrintStringIntern};
use serde_json::Value;

impl PrintStringIntern for StringTable {
    fn intern(&mut self, s: &str) -> u32 {
        StringTable::intern(self, s)
    }
}

pub struct DocxState {
    pub session: PrintSession,
    pub active: usize,
    pub zoom: f64,
    /// Live editor session when the document is not read-only.
    pub edit: Option<nitro_xml_docx::DocxEditor>,
    /// Defer full-body parse until after first paint (xlsx SeekTail analogue).
    pending_body_complete: bool,
}

impl DocxState {
    /// Open with a body prefix (first hard page break) so first paint stays O(visible).
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        // Borrow the handle's package Arc during parse only — PrintSession owns
        // its model/media after open (no Box::leak second copy of the zip).
        let session = PrintSession::open_first_view(bytes, 1)?;
        let pending_body_complete = session.body_truncated;
        Ok(Self {
            session,
            active: 0,
            zoom: 1.0,
            edit: None,
            pending_body_complete,
        })
    }

    /// Run deferred `ensure_body_complete` once after first paint.
    pub fn flush_pending_body(&mut self, bytes: &[u8]) -> Result<bool, String> {
        if !self.pending_body_complete {
            return Ok(false);
        }
        self.pending_body_complete = false;
        self.session.ensure_body_complete(bytes)
    }

    pub fn page_count(&self) -> usize {
        self.session.result.pages.len()
    }

    /// True when layout and chart canvas measures (if any) are ready to paint.
    pub fn paint_ready(&self) -> bool {
        self.session.layout_ready && self.session.charts_measured()
    }

    pub fn set_active_page(&mut self, index: usize) -> Result<(), String> {
        let n = self.page_count();
        if n == 0 {
            self.active = 0;
            return Ok(());
        }
        if index >= n {
            return Err(format!("page index {index} out of range"));
        }
        self.active = index;
        Ok(())
    }

    pub fn set_zoom(&mut self, zoom_pct: f64) {
        self.zoom = (zoom_pct / 100.0).clamp(0.1, 4.0);
    }

    pub fn paint(
        &mut self,
        strings: &mut StringTable,
        commands: &mut nitro_xml_render::CommandBuffer,
    ) {
        self.session
            .paint(commands, strings, self.zoom, Some(self.active), print_now());
    }

    pub fn process_events(&mut self, events: &[Event], strings: &StringTable) {
        let n = self.page_count();
        for ev in events {
            if let Event::Keyboard { type_, key_id, .. } = ev {
                if *type_ != 0 || n == 0 {
                    continue;
                }
                let key = strings.resolve(*key_id).unwrap_or("");
                if key == "ArrowRight" || key == "PageDown" {
                    let next = self.active + 1;
                    if next < n {
                        self.active = next;
                    }
                } else if (key == "ArrowLeft" || key == "PageUp") && self.active > 0 {
                    self.active -= 1;
                }
            }
        }
    }

    pub fn page_tabs(&self) -> Vec<(u32, String)> {
        let n = self.page_count().max(1);
        (0..n)
            .map(|i| (i as u32, format!("Page {}", i + 1)))
            .collect()
    }

    /// Body find over paragraph plain text, including table cells.
    /// Page index comes from the print layout when a block has been placed.
    pub(crate) fn search_body(
        &self,
        query: &str,
        case_sensitive: bool,
        match_entire: bool,
    ) -> Vec<FindMatch> {
        let paras = body_paragraphs(&self.session.model.blocks);
        let pages = block_page_map(&self.session.result);
        let mut matches = Vec::new();
        for para in &paras {
            let sheet = pages.get(&para.block_id).copied().unwrap_or(0);
            push_text_hits(
                &mut matches,
                sheet,
                para.ordinal,
                &para.text,
                query,
                case_sensitive,
                match_entire,
            );
        }
        matches
    }

    /// Plain text for interactive copy: the selected paragraph only.
    /// No selection → empty (DOM text selection is the host's job; shell dump
    /// uses [`Self::body_plain`] so Ctrl+C without a block id does not dump the page).
    pub(crate) fn copy_plain(&self, selection_id: &str) -> String {
        let paras = body_paragraphs(&self.session.model.blocks);
        if selection_id.is_empty() || selection_id == "A1" {
            return String::new();
        }
        paras
            .iter()
            .find(|p| p.block_id == selection_id)
            .map(|p| p.text.clone())
            .unwrap_or_default()
    }

    /// Whole-body plain text for the #29 shell differential (`copySample`).
    pub(crate) fn body_plain(&self) -> String {
        body_paragraphs(&self.session.model.blocks)
            .iter()
            .map(|p| p.text.as_str())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn has_block_id(&self, id: &str) -> bool {
        body_paragraphs(&self.session.model.blocks)
            .iter()
            .any(|p| p.block_id == id)
    }

    /// Safe-href / bookmark target at paragraph ordinal `row`, run `col`.
    pub(crate) fn hyperlink_at(&self, row: u32, col: u32) -> Option<DocxHyperlinkHit> {
        let paras = body_paragraphs(&self.session.model.blocks);
        let para = paras.iter().find(|p| p.ordinal == row)?;
        let link = para.links.iter().find(|l| l.run == col)?;
        let pending = pending_from_link(link)?;
        let page = if let Some(anchor) = link.anchor.as_deref() {
            bookmark_page(&paras, &block_page_map(&self.session.result), anchor)
        } else {
            None
        };
        let block_id = if pending.location.is_some() {
            paras
                .iter()
                .find(|p| {
                    p.bookmarks
                        .iter()
                        .any(|b| Some(b.as_str()) == pending.location.as_deref())
                })
                .map(|p| p.block_id.clone())
        } else {
            None
        };
        Some(DocxHyperlinkHit {
            pending,
            page,
            block_id,
        })
    }

    pub(crate) fn hyperlinks_json(&self) -> Vec<serde_json::Value> {
        let paras = body_paragraphs(&self.session.model.blocks);
        let mut out = Vec::new();
        for para in &paras {
            for link in &para.links {
                let Some(pending) = pending_from_link(link) else {
                    continue;
                };
                let mut m = serde_json::Map::new();
                m.insert("row".into(), serde_json::json!(para.ordinal));
                m.insert("run".into(), serde_json::json!(link.run));
                m.insert("open".into(), serde_json::json!(pending.open));
                if let Some(t) = &pending.target {
                    m.insert("target".into(), serde_json::Value::String(t.clone()));
                }
                if let Some(l) = &pending.location {
                    m.insert("location".into(), serde_json::Value::String(l.clone()));
                }
                if let Some(tip) = &pending.tooltip {
                    m.insert("tooltip".into(), serde_json::Value::String(tip.clone()));
                }
                out.push(serde_json::Value::Object(m));
            }
        }
        out
    }
}

pub(crate) struct DocxHyperlinkHit {
    pub pending: PendingHyperlink,
    pub page: Option<u32>,
    pub block_id: Option<String>,
}

struct ParaRec {
    ordinal: u32,
    block_id: String,
    text: String,
    bookmarks: Vec<String>,
    links: Vec<RunLink>,
}

struct RunLink {
    run: u32,
    href: Option<String>,
    anchor: Option<String>,
    tooltip: Option<String>,
}

fn body_paragraphs(blocks: &[Value]) -> Vec<ParaRec> {
    let mut out = Vec::new();
    walk_blocks(blocks, &mut out);
    out
}

fn walk_blocks(blocks: &[Value], out: &mut Vec<ParaRec>) {
    for block in blocks {
        let ty = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if ty == "paragraph" {
            let ordinal = out.len() as u32;
            out.push(ParaRec {
                ordinal,
                block_id: block
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned(),
                text: paragraph_plain(block),
                bookmarks: string_list(block.get("bookmarks")),
                links: run_links(block),
            });
        } else if ty == "table" {
            let Some(rows) = block.get("rows").and_then(|v| v.as_array()) else {
                continue;
            };
            for row in rows {
                let Some(cells) = row.as_array() else {
                    continue;
                };
                for cell in cells {
                    if let Some(paras) = cell.get("paragraphs").and_then(|v| v.as_array()) {
                        walk_blocks(paras, out);
                    }
                    if let Some(nested) = cell.get("nestedTables").and_then(|v| v.as_array()) {
                        walk_blocks(nested, out);
                    }
                }
            }
        }
    }
}

fn paragraph_plain(block: &Value) -> String {
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

fn string_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn run_links(block: &Value) -> Vec<RunLink> {
    let Some(runs) = block.get("runs").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, run) in runs.iter().enumerate() {
        let Some(link) = run.get("link") else {
            continue;
        };
        out.push(RunLink {
            run: i as u32,
            href: nonempty_str(link.get("href")),
            anchor: nonempty_str(link.get("anchor")),
            tooltip: nonempty_str(link.get("tooltip")),
        });
    }
    out
}

fn nonempty_str(v: Option<&Value>) -> Option<String> {
    v.and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn pending_from_link(link: &RunLink) -> Option<PendingHyperlink> {
    let anchor = link.anchor.clone().filter(|s| !s.is_empty());
    let href = link.href.clone().filter(|s| !s.is_empty());
    let allowed = anchor.is_some() || hyperlink_policy(href.as_deref()).store;
    if !allowed {
        return None;
    }
    let open = anchor.is_some() || hyperlink_policy(href.as_deref()).open;
    Some(PendingHyperlink {
        target: if anchor.is_some() { None } else { href },
        location: anchor,
        open,
        tooltip: link.tooltip.clone(),
    })
}

fn bookmark_page(paras: &[ParaRec], pages: &BTreeMap<String, u32>, anchor: &str) -> Option<u32> {
    let para = paras
        .iter()
        .find(|p| p.bookmarks.iter().any(|b| b == anchor))?;
    pages.get(&para.block_id).copied()
}

fn block_page_map(result: &nitro_xml_docx::LayoutResult) -> BTreeMap<String, u32> {
    let Ok(value) = serde_json::to_value(result) else {
        return BTreeMap::new();
    };
    let mut map = BTreeMap::new();
    let Some(pages) = value.get("pages").and_then(|v| v.as_array()) else {
        return map;
    };
    for page in pages {
        let index = page.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let Some(columns) = page.get("columns").and_then(|v| v.as_array()) else {
            continue;
        };
        for column in columns {
            let Some(blocks) = column.get("blocks").and_then(|v| v.as_array()) else {
                continue;
            };
            for block in blocks {
                collect_block_ids(block, index, &mut map);
            }
        }
    }
    map
}

fn collect_block_ids(block: &Value, page: u32, map: &mut BTreeMap<String, u32>) {
    if let Some(id) = block.get("blockId").and_then(|v| v.as_str()) {
        map.entry(id.to_owned()).or_insert(page);
    }
    if let Some(rows) = block.get("rows").and_then(|v| v.as_array()) {
        for row in rows {
            let Some(cells) = row.get("cells").and_then(|v| v.as_array()) else {
                continue;
            };
            for cell in cells {
                let Some(paras) = cell.as_array() else {
                    continue;
                };
                for para in paras {
                    collect_block_ids(para, page, map);
                }
            }
        }
    }
}

pub fn is_docx_extension(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "docx" | "docm" | "dotx" | "dotm")
}

fn print_now() -> PrintNow {
    #[cfg(target_arch = "wasm32")]
    {
        let d = js_sys::Date::new_0();
        return PrintNow {
            year: d.get_full_year() as i32,
            month: d.get_month() as u32 + 1,
            day: d.get_date() as u32,
            hour: d.get_hours() as u32,
            minute: d.get_minutes() as u32,
        };
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        PrintNow::default()
    }
}

pub(crate) fn process_frame(handle: &mut crate::handle::Handle) {
    let events: Vec<Event> = handle.events.pending_snapshot();
    if let Some(ref mut docx) = handle.docx {
        docx.set_zoom(handle.shell.zoom_pct);
        docx.process_events(&events, &handle.strings);
    }
    handle.events.drain();

    let was_ready = handle
        .docx
        .as_ref()
        .map(|d| d.paint_ready())
        .unwrap_or(false);
    let prev_page_count = handle.docx.as_ref().map(|d| d.page_count()).unwrap_or(0);

    // After first paint is ready, finish the truncated body (SeekTail analogue).
    let mut body_just_completed = false;
    if was_ready {
        let bytes = std::sync::Arc::clone(&handle.bytes);
        if let Some(ref mut docx) = handle.docx {
            body_just_completed = docx.flush_pending_body(bytes.as_ref()).unwrap_or(false);
        }
    }

    let layout_ready = if let Some(ref mut docx) = handle.docx {
        docx.session
            .tick_measure(&mut handle.measures, &mut handle.strings)
    } else {
        false
    };

    let paint_ready = handle
        .docx
        .as_ref()
        .map(|d| d.paint_ready())
        .unwrap_or(false);

    if !layout_ready || !paint_ready {
        handle.commands.begin();
        return;
    }

    let mut new_warnings: Vec<String> = Vec::new();
    let mut tabs: Option<(Vec<(u32, String)>, u32)> = None;
    if let Some(ref mut docx) = handle.docx {
        let page_count = docx.page_count();
        if !was_ready || page_count != prev_page_count {
            tabs = Some((docx.page_tabs(), docx.active as u32));
        }
        if !was_ready || body_just_completed {
            for w in &docx.session.warnings {
                new_warnings.push(w.clone());
            }
        }
        docx.paint(&mut handle.strings, &mut handle.commands);
    }
    if let Some((tabs, idx)) = tabs {
        handle.shell.set_tabs(tabs, idx);
        handle.shell.dirty_ui = true;
    }
    if body_just_completed {
        // Truncated open ingested a prefix of warnings; replace with the full set.
        handle.resync_docx_shell_diagnostics();
    } else {
        for w in new_warnings {
            if handle.shell.diagnostics.has_message(&w) {
                continue;
            }
            let code = crate::shell::open_diagnostic_code(&w, "docx_warning");
            handle.shell.diagnostics.ingest_warning(code, &w, None);
            handle.push_warning(code.into(), w, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catches_docx_find_table_cells_and_script_hrefs() {
        // Catches: docx shell find skipping table-cell text, and javascript: hrefs stored as openable.
        let blocks = vec![
            json!({
                "type": "paragraph",
                "id": "p0",
                "runs": [
                    {"text": "See "},
                    {"text": "docs", "link": {"href": "javascript:alert(1)"}},
                    {"text": " and "},
                    {"text": "ok", "link": {"href": "https://example.com/a", "tooltip": "Ex"}}
                ]
            }),
            json!({
                "type": "table",
                "id": "t0",
                "rows": [[{
                    "paragraphs": [{
                        "type": "paragraph",
                        "id": "p1",
                        "bookmarks": ["_Intro"],
                        "runs": [
                            {"text": "cell alpha", "link": {"anchor": "_Intro"}}
                        ]
                    }]
                }]]
            }),
        ];
        let paras = body_paragraphs(&blocks);
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[1].text, "cell alpha");
        let mut hits = Vec::new();
        push_text_hits(
            &mut hits,
            0,
            paras[1].ordinal,
            &paras[1].text,
            "alpha",
            false,
            false,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "alpha");
        let stored: Vec<_> = paras
            .iter()
            .flat_map(|p| p.links.iter().filter_map(pending_from_link))
            .collect();
        assert!(stored
            .iter()
            .all(|l| l.target.as_deref() != Some("javascript:alert(1)")));
        assert!(stored
            .iter()
            .any(|l| l.target.as_deref() == Some("https://example.com/a") && l.open));
        assert!(stored
            .iter()
            .any(|l| l.location.as_deref() == Some("_Intro") && l.target.is_none()));
    }
}
