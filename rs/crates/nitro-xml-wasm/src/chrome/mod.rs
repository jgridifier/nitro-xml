// SPDX-License-Identifier: MIT
//! Xlsx editor chrome: command registry facade, Home ribbon, shortcuts (#53).

#[allow(dead_code)]
mod commands;
mod ribbon;
mod shortcuts;

pub use commands::ChromeCommands;
pub use shortcuts::{find_shortcut, KeyEventLike, SHORTCUTS};

/// Lazy-loaded editor chrome state shared by ribbon HTML and command dispatch.
pub struct EditorChrome {
    pub commands: ChromeCommands,
    active_tab: String,
    read_only: bool,
    loaded: bool,
}

impl EditorChrome {
    pub fn new() -> Self {
        Self {
            commands: ChromeCommands::new(),
            active_tab: "home".to_string(),
            read_only: true,
            loaded: false,
        }
    }

    pub fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.commands.register_chrome_stubs();
        self.loaded = true;
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.commands.set_read_only(read_only);
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn loaded(&self) -> bool {
        self.loaded
    }

    pub fn active_tab(&self) -> &str {
        &self.active_tab
    }

    pub fn ribbon_html(&self) -> String {
        ribbon::ribbon_html(self)
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.commands.is_enabled(id)
    }

    pub fn is_checked(&self, id: &str) -> bool {
        self.commands.is_checked(id)
    }

    pub fn dispatch_command(&mut self, id: &str, host: &mut dyn std::any::Any) -> bool {
        match self.commands.dispatch(id, host) {
            Ok(()) => true,
            Err(_) => false,
        }
    }

    pub fn match_shortcut(
        &self,
        event: &KeyEventLike<'_>,
        mac: bool,
    ) -> Option<shortcut::ShortcutMatch> {
        if !self.loaded || self.read_only {
            return None;
        }
        find_shortcut(event, mac).map(|s| shortcut::ShortcutMatch {
            command: s.command,
            action: s.action,
        })
    }
}

pub mod shortcut {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ShortcutMatch {
        pub command: Option<&'static str>,
        pub action: Option<&'static str>,
    }
}
