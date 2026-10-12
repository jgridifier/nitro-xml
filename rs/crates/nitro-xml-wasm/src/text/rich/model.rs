// SPDX-License-Identifier: MIT
//! Editing IR for the rich multi-paragraph text surface (#61).
//!
//! Mirrors upstream pptx `InlineListParagraph` / `TextSegment` and docx
//! `Paragraph`/`TextRun` shapes without ProseMirror. Layout geometry stays in
//! nitro-xml-docx; this module owns the editable tree only.
//!
//! TODO(#56): IME composition · TODO(#57): bidi · TODO(#59): a11y caret exposure.

/// Character/paragraph style snapshot applied to a run (subset of OOXML run props).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font: Option<String>,
    pub size_half_points: Option<u32>,
}

/// One styled text run inside a paragraph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichRun {
    pub text: String,
    pub style: RunStyle,
}

impl RichRun {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: RunStyle::default(),
        }
    }

    pub fn styled(text: impl Into<String>, style: RunStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// A paragraph is an ordered list of runs (may be empty = empty ¶).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichParagraph {
    pub runs: Vec<RichRun>,
}

impl RichParagraph {
    pub fn from_plain(text: &str) -> Self {
        Self {
            runs: if text.is_empty() {
                vec![RichRun::plain("")]
            } else {
                vec![RichRun::plain(text)]
            },
        }
    }

    pub fn plain_text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// Table cell holding one or more paragraphs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichCell {
    pub paragraphs: Vec<RichParagraph>,
}

impl RichCell {
    pub fn from_plain(text: &str) -> Self {
        Self {
            paragraphs: vec![RichParagraph::from_plain(text)],
        }
    }
}

/// Simple rectangular table (no merges in the editing IR — merges are layout).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichTable {
    pub rows: Vec<Vec<RichCell>>,
}

/// Top-level block in a rich document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RichBlock {
    Paragraph(RichParagraph),
    Table(RichTable),
}

/// Editable rich document (Word body fragment or slide text frame).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichDocument {
    pub blocks: Vec<RichBlock>,
}

impl RichDocument {
    pub fn empty() -> Self {
        Self {
            blocks: vec![RichBlock::Paragraph(RichParagraph::from_plain(""))],
        }
    }

    pub fn from_paragraphs(texts: &[&str]) -> Self {
        Self {
            blocks: texts
                .iter()
                .map(|t| RichBlock::Paragraph(RichParagraph::from_plain(t)))
                .collect(),
        }
    }

    /// Flatten to plain text with `\n` between paragraphs; cells joined by `\t`, rows by `\n`.
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for (bi, block) in self.blocks.iter().enumerate() {
            if bi > 0 {
                out.push('\n');
            }
            match block {
                RichBlock::Paragraph(p) => out.push_str(&p.plain_text()),
                RichBlock::Table(t) => {
                    for (ri, row) in t.rows.iter().enumerate() {
                        if ri > 0 {
                            out.push('\n');
                        }
                        for (ci, cell) in row.iter().enumerate() {
                            if ci > 0 {
                                out.push('\t');
                            }
                            for (pi, p) in cell.paragraphs.iter().enumerate() {
                                if pi > 0 {
                                    out.push('\n');
                                }
                                out.push_str(&p.plain_text());
                            }
                        }
                    }
                }
            }
        }
        out
    }
}
