// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/axis-metrics.ts; changes listed in PROVENANCE.md.

#[derive(Debug, Clone)]
pub struct AxisMetrics {
    default_size: i32,
    count: u32,
    indices: Vec<u32>,
    sizes: Vec<i32>,
    prefix: Vec<i32>,
    total_delta: i32,
}

impl AxisMetrics {
    pub fn new(
        default_size: i32,
        count: u32,
        overrides: impl IntoIterator<Item = (u32, i32)>,
    ) -> Self {
        let mut sorted: Vec<(u32, i32)> = overrides
            .into_iter()
            .filter(|(index, size)| *index < count && *size != default_size)
            .collect();
        sorted.sort_by_key(|(a, _)| *a);
        let mut indices = Vec::new();
        let mut sizes = Vec::new();
        let mut prefix = Vec::new();
        let mut delta = 0i32;
        for (index, size) in sorted {
            if indices.last() == Some(&index) {
                continue;
            }
            let size = size.max(0);
            indices.push(index);
            sizes.push(size);
            prefix.push(delta);
            delta += size - default_size;
        }
        Self {
            default_size,
            count,
            indices,
            sizes,
            prefix,
            total_delta: delta,
        }
    }

    fn rank(&self, index: u32) -> usize {
        let mut lo = 0usize;
        let mut hi = self.indices.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.indices[mid] < index {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    pub fn size(&self, index: u32) -> i32 {
        if index >= self.count {
            return 0;
        }
        let k = self.rank(index);
        if k < self.indices.len() && self.indices[k] == index {
            self.sizes[k]
        } else {
            self.default_size
        }
    }

    pub fn start(&self, index: u32) -> i32 {
        let i = index.min(self.count);
        let k = self.rank(i);
        let before = if k < self.indices.len() {
            self.prefix[k]
        } else {
            self.total_delta
        };
        (i as i32) * self.default_size + before
    }

    pub fn total(&self) -> i32 {
        self.start(self.count)
    }

    pub fn at(&self, offset: i32) -> u32 {
        if offset <= 0 {
            return self.first_visible(0);
        }
        if offset >= self.total() {
            return self.last_visible(self.count - 1);
        }
        let mut lo = 0usize;
        let mut hi = self.indices.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let idx = self.indices[mid];
            let top = (idx as i32) * self.default_size + self.prefix[mid];
            if top <= offset {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            return if self.default_size > 0 {
                (offset / self.default_size) as u32
            } else {
                0
            };
        }
        let k = lo - 1;
        let idx = self.indices[k];
        let top = (idx as i32) * self.default_size + self.prefix[k];
        let size = self.sizes[k];
        if offset < top + size {
            return idx;
        }
        if self.default_size <= 0 {
            return self.first_visible(idx + 1);
        }
        let line = idx + 1 + ((offset - top - size) / self.default_size) as u32;
        line.min(self.count - 1)
    }

    pub fn first_visible(&self, index: u32) -> u32 {
        for i in index..self.count {
            if self.size(i) > 0 {
                return i;
            }
        }
        self.count.saturating_sub(1)
    }

    pub fn last_visible(&self, index: u32) -> u32 {
        for i in (0..=index.min(self.count - 1)).rev() {
            if self.size(i) > 0 {
                return i;
            }
        }
        0
    }
}
