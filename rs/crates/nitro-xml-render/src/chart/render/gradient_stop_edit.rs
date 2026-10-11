// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/gradient-stop-edit.ts

pub fn sort_gradient_stops<T: Clone>(stops: &[T], position: fn(&T) -> f64) -> Vec<T> {
    let mut out = stops.to_vec();
    out.sort_by(|a, b| {
        position(a)
            .partial_cmp(&position(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}
