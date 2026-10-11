// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/review-formatting-display.ts; changes listed in PROVENANCE.md.

use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReviewDisplayMode {
    All,
    Simple,
    Final,
    Original,
}

impl ReviewDisplayMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "simple" => Self::Simple,
            "final" => Self::Final,
            "original" => Self::Original,
            _ => Self::All,
        }
    }
}

pub fn is_run_hidden_for_review(run: &Value, mode: ReviewDisplayMode) -> bool {
    let kind = run
        .get("revision")
        .and_then(|r| r.get("kind"))
        .and_then(|v| v.as_str());
    match mode {
        ReviewDisplayMode::Original => kind == Some("insert") || kind == Some("moveTo"),
        ReviewDisplayMode::Final | ReviewDisplayMode::Simple => {
            kind == Some("delete") || kind == Some("moveFrom")
        }
        ReviewDisplayMode::All => false,
    }
}
