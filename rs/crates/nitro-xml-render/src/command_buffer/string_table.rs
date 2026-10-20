// SPDX-License-Identifier: MIT
//! String table ids referenced by command-buffer opcodes (e.g. `SetInnerMarkup`).

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// FNV-1a — fixed hasher, no `RandomState` / `getrandom` (wasm-safe).
#[derive(Clone)]
struct FnvHasher(u64);

impl Default for FnvHasher {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for FnvHasher {
    fn write(&mut self, bytes: &[u8]) {
        const FNV_PRIME: u64 = 0x100000001b3;
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type HashToIds = HashMap<u64, Vec<u32>, BuildHasherDefault<FnvHasher>>;

#[derive(Debug, Default)]
pub struct StringTable {
    strings: Vec<String>,
    by_hash: HashToIds,
}

impl StringTable {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn hash_str(s: &str) -> u64 {
        let mut h = FnvHasher::default();
        h.write(s.as_bytes());
        h.finish()
    }

    pub fn intern(&mut self, value: &str) -> u32 {
        let hash = Self::hash_str(value);
        if let Some(ids) = self.by_hash.get(&hash) {
            for &id in ids {
                if self.strings[id as usize] == value {
                    return id;
                }
            }
        }
        let id = self.strings.len() as u32;
        self.strings.push(value.to_string());
        self.by_hash.entry(hash).or_default().push(id);
        id
    }

    pub fn get(&self, id: u32) -> Option<&str> {
        self.strings.get(id as usize).map(|s| s.as_str())
    }

    pub fn as_slice(&self) -> &[String] {
        &self.strings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_hash_index_dedupes() {
        // Catches: render StringTable must reuse ids after HashMap intern (#30 encode).
        let mut t = StringTable::new();
        let a = t.intern("div");
        let b = t.intern("div");
        let c = t.intern("span");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(t.get(a), Some("div"));
    }
}
