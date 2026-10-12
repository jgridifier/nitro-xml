// SPDX-License-Identifier: MIT
//! Style-aware insertion and deletion for the rich surface (#61).
//!
//! Empty selection → insert inherits the style of the run at the caret
//! (upstream PM stored-marks / pptx segment style behaviour).

use super::model::{RichBlock, RichDocument, RichParagraph, RichRun, RunStyle};
use super::position::{clamp_pos, paragraph_at, paragraph_at_mut, ParaPath, RichPos};
use super::selection::{selected_plain, RichSelection};

/// Style at the caret (run containing `offset`, or previous run if at a boundary).
pub fn style_at(doc: &RichDocument, pos: &RichPos) -> RunStyle {
    let Some(p) = paragraph_at(doc, &pos.path) else {
        return RunStyle::default();
    };
    let mut at = 0usize;
    let mut last = RunStyle::default();
    for run in &p.runs {
        let end = at + run.text.len();
        if pos.offset <= end {
            // Prefer the run to the left when sitting on a boundary after content.
            if pos.offset == at && at > 0 {
                return last;
            }
            return run.style.clone();
        }
        last = run.style.clone();
        at = end;
    }
    last
}

fn split_paragraph_at(p: &RichParagraph, offset: usize) -> (RichParagraph, RichParagraph) {
    let mut left_runs = Vec::new();
    let mut right_runs = Vec::new();
    let mut at = 0usize;
    let mut on_right = false;
    for run in &p.runs {
        let end = at + run.text.len();
        if on_right {
            right_runs.push(run.clone());
        } else if offset <= at {
            on_right = true;
            right_runs.push(run.clone());
        } else if offset >= end {
            left_runs.push(run.clone());
        } else {
            let local = offset - at;
            let (a, b) = run.text.split_at(local);
            if !a.is_empty() {
                left_runs.push(RichRun::styled(a, run.style.clone()));
            }
            if !b.is_empty() {
                right_runs.push(RichRun::styled(b, run.style.clone()));
            }
            on_right = true;
        }
        at = end;
    }
    if left_runs.is_empty() {
        left_runs.push(RichRun::plain(""));
    }
    if right_runs.is_empty() {
        right_runs.push(RichRun::plain(""));
    }
    (
        RichParagraph { runs: left_runs },
        RichParagraph { runs: right_runs },
    )
}

fn insert_into_paragraph(p: &mut RichParagraph, offset: usize, text: &str, style: &RunStyle) {
    if text.is_empty() {
        return;
    }
    // Fast path: insert into a run with matching style.
    let mut at = 0usize;
    for run in &mut p.runs {
        let end = at + run.text.len();
        if offset <= end && run.style == *style {
            let local = offset.saturating_sub(at).min(run.text.len());
            run.text.insert_str(local, text);
            return;
        }
        at = end;
    }
    // Rebuild: split the run at offset and inject a styled run.
    let mut new_runs = Vec::new();
    let mut at = 0usize;
    let mut inserted = false;
    for run in &p.runs {
        let end = at + run.text.len();
        if !inserted && offset <= end {
            let local = offset.saturating_sub(at).min(run.text.len());
            let (a, b) = run.text.split_at(local);
            if !a.is_empty() {
                new_runs.push(RichRun::styled(a, run.style.clone()));
            }
            new_runs.push(RichRun::styled(text, style.clone()));
            if !b.is_empty() {
                new_runs.push(RichRun::styled(b, run.style.clone()));
            }
            inserted = true;
        } else {
            new_runs.push(run.clone());
        }
        at = end;
    }
    if !inserted {
        new_runs.push(RichRun::styled(text, style.clone()));
    }
    if new_runs.is_empty() {
        new_runs.push(RichRun::plain(""));
    }
    p.runs = new_runs;
}

/// Delete the current selection (or nothing if collapsed).
pub fn delete_selection(doc: &mut RichDocument, sel: &mut RichSelection) {
    if sel.is_collapsed() {
        return;
    }
    let start = sel.start().clone();
    let end = sel.end().clone();
    if start.path == end.path {
        let style = style_at(doc, &start);
        if let Some(p) = paragraph_at_mut(doc, &start.path) {
            let text = p.plain_text();
            let a = start.offset.min(text.len());
            let b = end.offset.min(text.len()).max(a);
            let mut new_text = String::new();
            new_text.push_str(&text[..a]);
            new_text.push_str(&text[b..]);
            *p = RichParagraph {
                runs: vec![RichRun::styled(new_text, style)],
            };
        }
        *sel = RichSelection::collapsed(start);
        return;
    }
    // Cross-paragraph delete: keep start prefix + end suffix in start para; drop middles.
    let start_path = start.path.clone();
    let end_path = end.path.clone();
    let start_off = start.offset;
    let end_off = end.offset;
    let start_prefix = paragraph_at(doc, &start_path)
        .map(|p| p.plain_text()[..start_off.min(p.plain_text().len())].to_string())
        .unwrap_or_default();
    let end_suffix = paragraph_at(doc, &end_path)
        .map(|p| p.plain_text()[end_off.min(p.plain_text().len())..].to_string())
        .unwrap_or_default();
    let style = style_at(doc, &start);
    // Only support top-level paragraph blocks for multi-delete in this slice.
    if let (ParaPath::Block { block: sb }, ParaPath::Block { block: eb }) = (&start_path, &end_path)
    {
        if let Some(p) = paragraph_at_mut(doc, &start_path) {
            *p = RichParagraph {
                runs: vec![RichRun::styled(
                    format!("{start_prefix}{end_suffix}"),
                    style,
                )],
            };
        }
        let remove_from = (*sb + 1).min(doc.blocks.len());
        let remove_to = (*eb + 1).min(doc.blocks.len());
        if remove_from < remove_to {
            doc.blocks.drain(remove_from..remove_to);
        }
        *sel = RichSelection::collapsed(RichPos {
            path: ParaPath::Block { block: *sb },
            offset: start_prefix.len(),
        });
    } else {
        // Cell paths: collapse to deleting plain selected text within each — keep simple.
        let _ = selected_plain(doc, sel);
        *sel = RichSelection::collapsed(start);
    }
}

/// Insert `text` at the selection (replacing it), inheriting style at the caret.
pub fn insert_text(doc: &mut RichDocument, sel: &mut RichSelection, text: &str) {
    if !sel.is_collapsed() {
        delete_selection(doc, sel);
    }
    let at = clamp_pos(doc, &sel.focus);
    let style = style_at(doc, &at);
    if let Some(p) = paragraph_at_mut(doc, &at.path) {
        insert_into_paragraph(p, at.offset, text, &style);
        *sel = RichSelection::collapsed(RichPos {
            path: at.path,
            offset: at.offset + text.len(),
        });
    }
}

/// Split paragraph at caret (Enter).
pub fn split_paragraph(doc: &mut RichDocument, sel: &mut RichSelection) {
    if !sel.is_collapsed() {
        delete_selection(doc, sel);
    }
    let at = clamp_pos(doc, &sel.focus);
    let ParaPath::Block { block } = at.path else {
        // Table cell: split within cell paragraphs.
        if let Some(p) = paragraph_at(doc, &at.path).cloned() {
            let (left, right) = split_paragraph_at(&p, at.offset);
            if let ParaPath::Cell {
                block,
                row,
                col,
                para,
            } = &at.path
            {
                if let RichBlock::Table(t) = &mut doc.blocks[*block] {
                    let cell = &mut t.rows[*row][*col];
                    cell.paragraphs[*para] = left;
                    cell.paragraphs.insert(*para + 1, right);
                    *sel = RichSelection::collapsed(RichPos {
                        path: ParaPath::Cell {
                            block: *block,
                            row: *row,
                            col: *col,
                            para: *para + 1,
                        },
                        offset: 0,
                    });
                }
            }
        }
        return;
    };
    let Some(p) = paragraph_at(doc, &at.path).cloned() else {
        return;
    };
    let (left, right) = split_paragraph_at(&p, at.offset);
    doc.blocks[block] = RichBlock::Paragraph(left);
    doc.blocks.insert(block + 1, RichBlock::Paragraph(right));
    *sel = RichSelection::collapsed(RichPos {
        path: ParaPath::Block { block: block + 1 },
        offset: 0,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::rich::model::{RichBlock, RichDocument, RichParagraph, RichRun, RunStyle};

    #[test]
    fn catches_insert_inherits_bold_from_run_at_caret() {
        // Catches: typing in a bold run must produce bold text (style-aware insert).
        let mut doc = RichDocument {
            blocks: vec![RichBlock::Paragraph(RichParagraph {
                runs: vec![RichRun::styled(
                    "Hi",
                    RunStyle {
                        bold: true,
                        ..RunStyle::default()
                    },
                )],
            })],
        };
        let mut sel = RichSelection::collapsed(RichPos {
            path: ParaPath::Block { block: 0 },
            offset: 2,
        });
        insert_text(&mut doc, &mut sel, "!");
        let RichBlock::Paragraph(p) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(p.runs.iter().any(|r| r.text.contains('!') && r.style.bold));
    }
}
