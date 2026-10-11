// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/expect-defined.ts; changes listed in PROVENANCE.md.

pub fn expect_defined<T>(value: Option<T>, context: &str) -> T {
    match value {
        Some(v) => v,
        None => panic!("Internal invariant violated: {} is missing.", context),
    }
}

pub fn clone_pages(
    pages: &[crate::layout::result::LayoutPageBox],
) -> Vec<crate::layout::result::LayoutPageBox> {
    serde_json::from_value(serde_json::to_value(pages).expect("clone pages")).expect("clone pages")
}
