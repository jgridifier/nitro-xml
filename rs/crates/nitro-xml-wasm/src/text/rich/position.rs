// SPDX-License-Identifier: MIT
//! Positions inside a [`super::model::RichDocument`] (#61).
//!
//! A position names a paragraph path plus a UTF-8 byte offset into that
//! paragraph's concatenated run text (snapped to grapheme boundaries).

use super::model::{RichBlock, RichDocument, RichParagraph};
use crate::text::selection::snap_grapheme;

/// Where a paragraph lives in the document tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParaPath {
    /// Top-level paragraph block.
    Block { block: usize },
    /// Paragraph inside a table cell.
    Cell {
        block: usize,
        row: usize,
        col: usize,
        para: usize,
    },
}

/// Caret / selection endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichPos {
    pub path: ParaPath,
    /// Byte offset into the paragraph's concatenated plain text.
    pub offset: usize,
}

impl RichPos {
    pub fn block_start(block: usize) -> Self {
        Self {
            path: ParaPath::Block { block },
            offset: 0,
        }
    }
}

pub fn paragraph_at<'a>(doc: &'a RichDocument, path: &ParaPath) -> Option<&'a RichParagraph> {
    match path {
        ParaPath::Block { block } => match doc.blocks.get(*block)? {
            RichBlock::Paragraph(p) => Some(p),
            RichBlock::Table(_) => None,
        },
        ParaPath::Cell {
            block,
            row,
            col,
            para,
        } => match doc.blocks.get(*block)? {
            RichBlock::Table(t) => t.rows.get(*row)?.get(*col)?.paragraphs.get(*para),
            RichBlock::Paragraph(_) => None,
        },
    }
}

pub fn paragraph_at_mut<'a>(
    doc: &'a mut RichDocument,
    path: &ParaPath,
) -> Option<&'a mut RichParagraph> {
    match path {
        ParaPath::Block { block } => match doc.blocks.get_mut(*block)? {
            RichBlock::Paragraph(p) => Some(p),
            RichBlock::Table(_) => None,
        },
        ParaPath::Cell {
            block,
            row,
            col,
            para,
        } => match doc.blocks.get_mut(*block)? {
            RichBlock::Table(t) => t
                .rows
                .get_mut(*row)?
                .get_mut(*col)?
                .paragraphs
                .get_mut(*para),
            RichBlock::Paragraph(_) => None,
        },
    }
}

pub fn clamp_pos(doc: &RichDocument, pos: &RichPos) -> RichPos {
    let Some(p) = paragraph_at(doc, &pos.path) else {
        return RichPos::block_start(0);
    };
    let text = p.plain_text();
    let offset = snap_grapheme(&text, pos.offset.min(text.len()));
    RichPos {
        path: pos.path.clone(),
        offset,
    }
}

/// Total order for comparing positions (document order).
pub fn cmp_pos(a: &RichPos, b: &RichPos) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (&a.path, &b.path) {
        (ParaPath::Block { block: ab }, ParaPath::Block { block: bb }) => {
            ab.cmp(bb).then(a.offset.cmp(&b.offset))
        }
        (
            ParaPath::Cell {
                block: ab,
                row: ar,
                col: ac,
                para: ap,
            },
            ParaPath::Cell {
                block: bb,
                row: br,
                col: bc,
                para: bp,
            },
        ) => ab
            .cmp(bb)
            .then(ar.cmp(br))
            .then(ac.cmp(bc))
            .then(ap.cmp(bp))
            .then(a.offset.cmp(&b.offset)),
        (ParaPath::Block { block: ab }, ParaPath::Cell { block: bb, .. }) => match ab.cmp(bb) {
            Ordering::Equal => Ordering::Less,
            o => o,
        },
        (ParaPath::Cell { block: ab, .. }, ParaPath::Block { block: bb }) => match ab.cmp(bb) {
            Ordering::Equal => Ordering::Greater,
            o => o,
        },
    }
}

/// Enumerate paragraph paths in document order.
pub fn iter_para_paths(doc: &RichDocument) -> Vec<ParaPath> {
    let mut out = Vec::new();
    for (bi, block) in doc.blocks.iter().enumerate() {
        match block {
            RichBlock::Paragraph(_) => out.push(ParaPath::Block { block: bi }),
            RichBlock::Table(t) => {
                for (ri, row) in t.rows.iter().enumerate() {
                    for (ci, cell) in row.iter().enumerate() {
                        for pi in 0..cell.paragraphs.len() {
                            out.push(ParaPath::Cell {
                                block: bi,
                                row: ri,
                                col: ci,
                                para: pi,
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

pub fn next_para_path(doc: &RichDocument, path: &ParaPath) -> Option<ParaPath> {
    let paths = iter_para_paths(doc);
    let i = paths.iter().position(|p| p == path)?;
    paths.get(i + 1).cloned()
}

pub fn prev_para_path(doc: &RichDocument, path: &ParaPath) -> Option<ParaPath> {
    let paths = iter_para_paths(doc);
    let i = paths.iter().position(|p| p == path)?;
    if i == 0 {
        None
    } else {
        paths.get(i - 1).cloned()
    }
}
