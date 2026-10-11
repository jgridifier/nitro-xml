// SPDX-License-Identifier: MIT
//! Intern strings for command-buffer DOM ops (#19).

/// Stable string-table ids for one document frame.
pub trait StringIntern {
    fn intern(&mut self, s: &str) -> u32;
}

impl<F> StringIntern for F
where
    F: FnMut(&str) -> u32,
{
    fn intern(&mut self, s: &str) -> u32 {
        self(s)
    }
}
