// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/ligatures.ts; changes listed in PROVENANCE.md.

pub fn ligature_css(value: &str) -> String {
    let lower = value.to_lowercase();
    let enabled = |token: &str| value == "all" || lower.contains(token);
    format!(
        "{} {} {} {}",
        if enabled("standard") {
            "common-ligatures"
        } else {
            "no-common-ligatures"
        },
        if enabled("contextual") {
            "contextual"
        } else {
            "no-contextual"
        },
        if enabled("historical") {
            "historical-ligatures"
        } else {
            "no-historical-ligatures"
        },
        if enabled("discretional") {
            "discretionary-ligatures"
        } else {
            "no-discretionary-ligatures"
        }
    )
}
