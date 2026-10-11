// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/data-cache.ts; changes listed in PROVENANCE.md.

use super::model::{ChartDataCache, ChartDataSource};

pub fn chart_cache_values(cache: Option<&ChartDataCache>) -> Vec<serde_json::Value> {
    let cache = match cache {
        Some(c) => c,
        None => return Vec::new(),
    };
    let numeric = cache.cache_type == "number";
    let mut out: Vec<Option<serde_json::Value>> = Vec::new();
    for point in &cache.points {
        let idx = point.index.unwrap_or(out.len() as i64) as usize;
        let text = &point.value;
        let val = if numeric && !text.trim().is_empty() {
            let n = text.parse::<f64>();
            if let Ok(n) = n {
                if n.is_finite() {
                    serde_json::Value::from(n)
                } else {
                    serde_json::Value::String(text.clone())
                }
            } else {
                serde_json::Value::String(text.clone())
            }
        } else {
            serde_json::Value::String(text.clone())
        };
        if idx >= out.len() {
            out.resize(idx + 1, None);
        }
        out[idx] = Some(val);
    }
    let size = std::cmp::max(cache.point_count.unwrap_or(0) as usize, out.len());
    out.resize(size, None);
    out.into_iter()
        .map(|v| v.unwrap_or(serde_json::Value::String(String::new())))
        .collect()
}

pub fn chart_source_values(source: Option<&ChartDataSource>) -> Vec<serde_json::Value> {
    let cache = source
        .and_then(|s| s.cache.as_ref())
        .or_else(|| source.and_then(|s| s.levels.as_ref()?.first()));
    chart_cache_values(cache)
}
