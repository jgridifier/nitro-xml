// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/dialog/focus.ts and Tab-trap
// logic in dialog/dialog.ts; changes listed in PROVENANCE.md.
//! Focus trapping and restoration for modal dialogs and menus (#54).

/// Token identifying the element that should regain focus when an overlay closes.
/// Hosts map this id back to a DOM node (grid sink, ribbon button, …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocusRestore {
    pub target_id: String,
}

/// Result of a Tab key inside a focus trap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapTab {
    /// Focus should move to `index` in the focusable list.
    MoveTo(usize),
    /// No focusables — leave focus on the panel itself.
    StayOnPanel,
}

/// Compute the next focusable index for Tab / Shift+Tab inside a trap.
///
/// `current` is the index of the focused item, or `None` if focus is outside
/// the list (then Tab jumps to first, Shift+Tab to last).
pub fn trap_tab(current: Option<usize>, count: usize, shift: bool) -> TrapTab {
    if count == 0 {
        return TrapTab::StayOnPanel;
    }
    let last = count - 1;
    match current {
        None => {
            if shift {
                TrapTab::MoveTo(last)
            } else {
                TrapTab::MoveTo(0)
            }
        }
        Some(at) if !shift && at >= last => TrapTab::MoveTo(0),
        Some(0) if shift => TrapTab::MoveTo(last),
        Some(at) if shift => TrapTab::MoveTo(at.saturating_sub(1)),
        Some(at) => TrapTab::MoveTo(at + 1),
    }
}

/// Whether Tab at the edge of the trap should be prevented (cycle).
pub fn should_prevent_tab(current: Option<usize>, count: usize, shift: bool) -> bool {
    if count == 0 {
        return true;
    }
    match current {
        None => true,
        Some(at) if !shift && at + 1 >= count => true,
        Some(0) if shift => true,
        _ => false,
    }
}

/// Stack of restore targets (nested overlays). Closing pops and returns the target.
#[derive(Clone, Debug, Default)]
pub struct FocusStack {
    stack: Vec<FocusRestore>,
}

impl FocusStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, target_id: impl Into<String>) {
        self.stack.push(FocusRestore {
            target_id: target_id.into(),
        });
    }

    pub fn pop(&mut self) -> Option<FocusRestore> {
        self.stack.pop()
    }

    pub fn peek(&self) -> Option<&FocusRestore> {
        self.stack.last()
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_tab_at_last_focusable_cycles_to_first() {
        // Catches: Tab on the last focusable must cycle to index 0 (dialog trap).
        assert_eq!(trap_tab(Some(2), 3, false), TrapTab::MoveTo(0));
        assert!(should_prevent_tab(Some(2), 3, false));
    }

    #[test]
    fn catches_focus_stack_restores_opener_on_close() {
        // Catches: closing an overlay must restore the opener id, not leave a stale token.
        let mut stack = FocusStack::new();
        stack.push("grid-sink");
        stack.push("ribbon-bold");
        assert_eq!(stack.pop().unwrap().target_id, "ribbon-bold");
        assert_eq!(stack.pop().unwrap().target_id, "grid-sink");
        assert!(stack.pop().is_none());
    }
}
