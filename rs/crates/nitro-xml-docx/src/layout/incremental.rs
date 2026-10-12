// SPDX-License-Identifier: MIT
//! Incremental print-layout re-pagination after edits (ADR 0003 / #81).
//!
//! Reuses a prefix of the previous [`LayoutResult`] and continues [`SectionFlow`]
//! from the first page that contains a dirty block. Must match eager
//! [`super::layout::layout_document`] for the same input and measurer.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::body::DocumentModel;
use crate::edit::host::DocxHost;

use super::input::LayoutDocumentInput;
use super::layout::{finalize_floats, layout_document};
use super::measure::TextMeasurer;
use super::result::{LayoutPageBox, LayoutResult};
use super::review_display::LayoutReviewOptions;
use super::section_flow::SectionFlow;
use super::wrap::{same_exclusions, wrap_exclusions};

/// Dirty summary for incremental relayout (from `DocxHost::touched` or change-sets).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirtyHint {
    pub touched_blocks: BTreeSet<usize>,
    pub structural: bool,
}

impl DirtyHint {
    pub fn from_host(host: &DocxHost) -> Self {
        Self {
            touched_blocks: host.touched.clone(),
            structural: false,
        }
    }

    pub fn structural_edit(touched_blocks: BTreeSet<usize>) -> Self {
        Self {
            touched_blocks,
            structural: true,
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.structural && self.touched_blocks.is_empty()
    }
}

/// Re-paginate from the first page that contains the earliest dirty block.
pub fn relayout_incremental(
    prev: &LayoutResult,
    input: &LayoutDocumentInput,
    measurer: &dyn TextMeasurer,
    dirty: DirtyHint,
) -> LayoutResult {
    if dirty.structural || dirty.is_empty() || prev.pages.is_empty() {
        return layout_document(input, measurer);
    }
    let min_block = dirty.touched_blocks.iter().copied().min().unwrap();
    let block_id = block_id_for_layout_index(input, min_block);
    let Some(block_id) = block_id else {
        return layout_document(input, measurer);
    };
    let Some((section_index, block_index)) = locate_block_in_input(input, &block_id) else {
        return layout_document(input, measurer);
    };
    let Some((prefix_pages, dirty_page)) = trim_prefix_pages(&prev.pages, &block_id) else {
        return layout_document(input, measurer);
    };
    if input_has_wrap_floats(input) && page_has_floats(&prev.pages, dirty_page) {
        return layout_document(input, measurer);
    }
    let mut approximations = HashSet::new();
    let pages = layout_sections_resume(
        input,
        measurer,
        None,
        section_index,
        block_index,
        prefix_pages,
        &mut approximations,
    );
    let mut result = LayoutResult {
        pages,
        approximations: approximations.into_iter().collect(),
    };
    reindex_pages(&mut result.pages);
    let result = finalize_floats(input, measurer, result);
    if float_wrap_invalidates_prefix(&result, input, measurer, dirty_page) {
        return layout_document(input, measurer);
    }
    result
}

/// Model path: adapt then incremental relayout on the previous layout snapshot.
pub fn relayout_incremental_model(
    prev: &LayoutResult,
    model: &DocumentModel,
    measurer: &dyn TextMeasurer,
    dirty: DirtyHint,
    options: LayoutReviewOptions,
) -> LayoutResult {
    let (input, approximations) = super::layout::prepare_layout_document_input(model, options);
    let result = relayout_incremental(prev, &input, measurer, dirty);
    let mut merged = HashSet::new();
    for a in approximations {
        merged.insert(a);
    }
    for a in result.approximations {
        merged.insert(a);
    }
    LayoutResult {
        pages: result.pages,
        approximations: merged.into_iter().collect(),
    }
}

#[derive(Clone, Debug)]
struct BlockOnPage {
    page_index: usize,
    column_index: usize,
    block_index: usize,
    y_px: f64,
    block_id: String,
}

fn block_id_for_layout_index(input: &LayoutDocumentInput, index: usize) -> Option<String> {
    let mut cursor = 0usize;
    for section in &input.sections {
        for block in &section.blocks {
            if cursor == index {
                return block.get("id").and_then(|v| v.as_str()).map(str::to_string);
            }
            cursor += 1;
        }
    }
    None
}

fn locate_block_in_input(input: &LayoutDocumentInput, block_id: &str) -> Option<(usize, usize)> {
    for (section_index, section) in input.sections.iter().enumerate() {
        for (block_index, block) in section.blocks.iter().enumerate() {
            if block.get("id").and_then(|v| v.as_str()) == Some(block_id) {
                return Some((section_index, block_index));
            }
        }
    }
    None
}

fn enumerate_blocks_on_page(page: &LayoutPageBox, page_index: usize) -> Vec<BlockOnPage> {
    let mut out = Vec::new();
    for (column_index, column) in page.columns.iter().enumerate() {
        for (block_index, block) in column.blocks.iter().enumerate() {
            let block_id = block
                .get("blockId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let y_px = block.get("yPx").and_then(|v| v.as_f64()).unwrap_or(0.0);
            out.push(BlockOnPage {
                page_index,
                column_index,
                block_index,
                y_px,
                block_id,
            });
        }
    }
    out.sort_by(|a, b| {
        a.column_index
            .cmp(&b.column_index)
            .then(a.y_px.total_cmp(&b.y_px))
            .then(a.block_index.cmp(&b.block_index))
    });
    out
}

fn trim_prefix_pages(
    pages: &[LayoutPageBox],
    dirty_block_id: &str,
) -> Option<(Vec<LayoutPageBox>, usize)> {
    let mut first_page: Option<usize> = None;
    for (page_index, page) in pages.iter().enumerate() {
        for slot in enumerate_blocks_on_page(page, page_index) {
            if slot.block_id == dirty_block_id {
                first_page = Some(page_index);
                break;
            }
        }
        if first_page.is_some() {
            break;
        }
    }
    let dirty_page = first_page?;
    let mut prefix: Vec<LayoutPageBox> = pages.iter().take(dirty_page).cloned().collect();
    let mut tail_page = pages[dirty_page].clone();
    tail_page.floats = None;
    tail_page.footnotes = None;
    let ordered = enumerate_blocks_on_page(&tail_page, dirty_page);
    let cut = ordered
        .iter()
        .position(|slot| slot.block_id == dirty_block_id)
        .unwrap_or(0);
    let keep_positions: HashSet<(usize, usize)> = ordered
        .iter()
        .take(cut)
        .map(|slot| (slot.column_index, slot.block_index))
        .collect();
    for (column_index, column) in tail_page.columns.iter_mut().enumerate() {
        let mut kept = Vec::new();
        for (block_index, block) in column.blocks.iter().enumerate() {
            if keep_positions.contains(&(column_index, block_index)) {
                kept.push(block.clone());
            }
        }
        column.blocks = kept;
    }
    for column in &mut tail_page.columns {
        column.end_y_px = None;
    }
    prefix.push(tail_page);
    Some((prefix, dirty_page))
}

fn layout_sections_resume(
    input: &LayoutDocumentInput,
    measurer: &dyn TextMeasurer,
    exclusions: Option<HashMap<usize, Vec<super::wrap::Exclusion>>>,
    start_section: usize,
    start_block: usize,
    prefix_pages: Vec<LayoutPageBox>,
    approximations: &mut HashSet<String>,
) -> Vec<LayoutPageBox> {
    let mut pages = prefix_pages;
    let mut previous_flow: Option<SectionFlow> = None;
    for (index, section) in input.sections.iter().enumerate() {
        if index < start_section {
            continue;
        }
        let continue_at = if index > start_section {
            if section.section_break.as_deref() == Some("continuous") {
                previous_flow.as_mut().map(|f| {
                    let at = f.cursor.finish_band();
                    pages = f.cursor.pages.clone();
                    at
                })
            } else {
                None
            }
        } else {
            None
        };
        let resume_same_section = index == start_section;
        let block_start = if resume_same_section { start_block } else { 0 };
        let mut flow = SectionFlow::new_from_block(
            section.clone(),
            pages,
            index,
            exclusions.clone(),
            continue_at,
            None,
            block_start,
            resume_same_section,
        );
        let mut note = |message: &str| {
            approximations.insert(message.to_string());
        };
        flow.run(measurer, &mut note);
        pages = flow.cursor.pages.clone();
        previous_flow = Some(flow);
    }
    pages
}

fn reindex_pages(pages: &mut [LayoutPageBox]) {
    for (index, page) in pages.iter_mut().enumerate() {
        page.index = index;
    }
}

fn input_has_wrap_floats(input: &LayoutDocumentInput) -> bool {
    input.sections.iter().any(|section| {
        section.blocks.iter().any(|block| {
            block
                .get("floats")
                .and_then(|v| v.as_array())
                .is_some_and(|floats| !floats.is_empty())
        })
    })
}

fn page_has_floats(pages: &[LayoutPageBox], from_page: usize) -> bool {
    pages
        .iter()
        .skip(from_page)
        .any(|page| page.floats.as_ref().is_some_and(|f| !f.is_empty()))
}

fn float_wrap_invalidates_prefix(
    incremental: &LayoutResult,
    input: &LayoutDocumentInput,
    measurer: &dyn TextMeasurer,
    dirty_page: usize,
) -> bool {
    if !input_has_wrap_floats(input) {
        return false;
    }
    let full = layout_document(input, measurer);
    if incremental.pages.len() != full.pages.len() {
        return true;
    }
    let inc_exc = wrap_exclusions(&incremental.pages);
    let full_exc = wrap_exclusions(&full.pages);
    if !same_exclusions(&inc_exc, &full_exc) {
        return true;
    }
    for page_index in 0..dirty_page.min(incremental.pages.len()) {
        if normalize_page_json(&incremental.pages[page_index])
            != normalize_page_json(&full.pages[page_index])
        {
            return true;
        }
    }
    false
}

fn normalize_page_json(page: &LayoutPageBox) -> String {
    serde_json::to_string(page).unwrap_or_default()
}

/// Test-only broken strategy: keep the prefix and skip reflow (must diverge from full layout).
#[cfg(test)]
pub(crate) fn relayout_incremental_freeze_suffix(
    prev: &LayoutResult,
    input: &LayoutDocumentInput,
    measurer: &dyn TextMeasurer,
    dirty: DirtyHint,
) -> LayoutResult {
    if dirty.is_empty() {
        return layout_document(input, measurer);
    }
    let min_block = dirty.touched_blocks.iter().copied().min().unwrap();
    let block_id = block_id_for_layout_index(input, min_block).unwrap_or_default();
    let (prefix_pages, _) =
        trim_prefix_pages(&prev.pages, &block_id).unwrap_or_else(|| (prev.pages.clone(), 0));
    let _ = measurer;
    LayoutResult {
        pages: prefix_pages,
        approximations: prev.approximations.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::DocumentModel;
    use crate::edit::{replay_docx_session, DocxHost};
    use crate::layout::input::{LayoutDocumentInput, LayoutPageGeometry, LayoutSectionSerde};
    use crate::layout::layout::{layout_document, layout_document_model};
    use crate::layout::measure::create_fake_measurer;
    use crate::layout::result::strip_layout_ir_chart_part_names;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    fn normalize_layout(result: &LayoutResult) -> String {
        let mut copy = result.clone();
        strip_layout_ir_chart_part_names(&mut copy);
        serde_json::to_string(&copy.pages).expect("pages json")
    }

    fn sample_page() -> LayoutPageGeometry {
        LayoutPageGeometry {
            width_px: 612.0,
            height_px: 792.0,
            margin_top_px: 72.0,
            margin_right_px: 72.0,
            margin_bottom_px: 72.0,
            margin_left_px: 72.0,
        }
    }

    fn model_from_host(host: &DocxHost) -> DocumentModel {
        DocumentModel {
            format: host.format.clone(),
            blocks: host.blocks.clone(),
            page: host.page.clone().unwrap_or(json!({})),
            warnings: Vec::new(),
            paragraph_styles: host.paragraph_styles.clone(),
            character_styles: host.character_styles.clone(),
            table_styles: None,
            numbering_catalog: host.numbering_catalog.clone(),
            theme: None,
            sections: host
                .sections
                .as_ref()
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
            footnotes: None,
            endnotes: None,
            footnote_num_fmt: None,
            endnote_num_fmt: None,
            even_and_odd_headers: host.even_and_odd_headers,
            auto_hyphenation: false,
            comments: None,
            track_changes: None,
            track_formatting: None,
            track_moves: None,
            media_part_names: Vec::new(),
        }
    }

    fn multi_page_input() -> LayoutDocumentInput {
        let blocks = (0..4)
            .map(|index| {
                json!({
                    "kind": "paragraph",
                    "id": format!("p{index}"),
                    "pageBreakBefore": index > 0,
                    "widowControl": false,
                    "runs": [{ "text": format!("Paragraph {index} on its own page") }]
                })
            })
            .collect();
        LayoutDocumentInput {
            sections: vec![LayoutSectionSerde {
                page: sample_page(),
                columns: None,
                blocks,
                section_break: None,
                vertical_align: None,
            }],
        }
    }

    fn mid_page_input() -> LayoutDocumentInput {
        let blocks = vec![
            json!({
                "kind": "paragraph",
                "id": "p0",
                "widowControl": false,
                "runs": [{ "text": "AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA " }]
            }),
            json!({
                "kind": "paragraph",
                "id": "p1",
                "widowControl": false,
                "runs": [{ "text": "Second paragraph after the tall one." }]
            }),
        ];
        LayoutDocumentInput {
            sections: vec![LayoutSectionSerde {
                page: sample_page(),
                columns: None,
                blocks,
                section_break: None,
                vertical_align: None,
            }],
        }
    }

    #[test]
    fn incremental_matches_full_on_page_break_document() {
        // Catches: incremental relayout diverges from full layout after a page-0 text dirty hint.
        let measurer = create_fake_measurer();
        let input = multi_page_input();
        let full = layout_document(&input, &measurer);
        let dirty = DirtyHint {
            touched_blocks: BTreeSet::from([0]),
            structural: false,
        };
        let inc = relayout_incremental(&full, &input, &measurer, dirty);
        assert_eq!(normalize_layout(&inc), normalize_layout(&full));
    }

    #[test]
    fn incremental_matches_full_after_mid_document_dirty() {
        // Catches: incremental relayout diverges from full layout when a later page is dirty.
        let measurer = create_fake_measurer();
        let input = multi_page_input();
        let full = layout_document(&input, &measurer);
        let dirty = DirtyHint {
            touched_blocks: BTreeSet::from([2]),
            structural: false,
        };
        let inc = relayout_incremental(&full, &input, &measurer, dirty);
        assert_eq!(normalize_layout(&inc), normalize_layout(&full));
    }

    #[test]
    fn incremental_matches_full_on_same_page_height_change() {
        // Catches: incremental relayout diverges when a height-changing edit reflows the same page.
        let measurer = create_fake_measurer();
        let input_before = mid_page_input();
        let full_before = layout_document(&input_before, &measurer);
        let mut input_after = input_before.clone();
        input_after.sections[0].blocks[0] = json!({
            "kind": "paragraph",
            "id": "p0",
            "widowControl": false,
            "runs": [{
                "text": "AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA "
            }]
        });
        let full_after = layout_document(&input_after, &measurer);
        let dirty = DirtyHint {
            touched_blocks: BTreeSet::from([0]),
            structural: false,
        };
        let inc = relayout_incremental(&full_before, &input_after, &measurer, dirty);
        assert_eq!(normalize_layout(&inc), normalize_layout(&full_after));
    }

    #[test]
    fn freeze_suffix_diverges_on_height_changing_edit() {
        // Catches: a prefix-only incremental shortcut incorrectly passes parity on same-page reflow.
        let measurer = create_fake_measurer();
        let input_before = mid_page_input();
        let full_before = layout_document(&input_before, &measurer);
        let mut input_after = input_before.clone();
        input_after.sections[0].blocks[0] = json!({
            "kind": "paragraph",
            "id": "p0",
            "widowControl": false,
            "runs": [{
                "text": "AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA AAAA "
            }]
        });
        let full_after = layout_document(&input_after, &measurer);
        let dirty = DirtyHint {
            touched_blocks: BTreeSet::from([0]),
            structural: false,
        };
        let broken =
            relayout_incremental_freeze_suffix(&full_before, &input_after, &measurer, dirty);
        assert_ne!(
            normalize_layout(&broken),
            normalize_layout(&full_after),
            "prefix-only relayout must not match full relayout"
        );
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    fn fixture_bytes(id: &str) -> Vec<u8> {
        let manifest = repo_root().join("fixtures/manifest.json");
        let text = fs::read_to_string(&manifest).expect("manifest");
        let v: serde_json::Value = serde_json::from_str(&text).expect("manifest json");
        let row = v["fixtures"]
            .as_array()
            .expect("fixtures")
            .iter()
            .find(|f| f["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("fixture {id}"));
        fs::read(repo_root().join(row["path"].as_str().expect("path"))).expect("fixture bytes")
    }

    #[test]
    fn incremental_matches_full_on_edit_scripts() {
        // Catches: incremental relayout diverges from full layout after representative edit scripts.
        let scripts = [
            "docx-set-para0-text",
            "docx-set-para-bold",
            "docx-pages10-para-text",
            "docx-pages10-table-cell",
        ];
        let measurer = create_fake_measurer();
        for script_id in scripts {
            let path = repo_root().join(format!("edits/docx/{script_id}.json"));
            let script = fs::read_to_string(&path).expect("script");
            let v: serde_json::Value = serde_json::from_str(&script).expect("script json");
            let fixture_id = v["fixture"].as_str().expect("fixture id");
            let bytes = fixture_bytes(fixture_id);
            let (_report, session) = replay_docx_session(&bytes, &script).expect("replay");
            let model = model_from_host(&session.host);
            let dirty = DirtyHint::from_host(&session.host);
            let full = layout_document_model(&model, &measurer, LayoutReviewOptions::default());
            let prev = layout_document_model(
                &model_from_host(&DocxHost::open(&bytes).expect("open")),
                &measurer,
                LayoutReviewOptions::default(),
            );
            let inc = relayout_incremental_model(
                &prev,
                &model,
                &measurer,
                dirty,
                LayoutReviewOptions::default(),
            );
            assert_eq!(
                normalize_layout(&inc),
                normalize_layout(&full),
                "script {script_id}"
            );
        }
    }

    #[test]
    fn structural_dirty_hint_uses_full_layout() {
        // Catches: structural edits skip incremental relayout and still match full layout.
        let measurer = create_fake_measurer();
        let input = multi_page_input();
        let full = layout_document(&input, &measurer);
        let dirty = DirtyHint::structural_edit(BTreeSet::from([1]));
        let inc = relayout_incremental(&full, &input, &measurer, dirty);
        assert_eq!(normalize_layout(&inc), normalize_layout(&full));
    }
}
