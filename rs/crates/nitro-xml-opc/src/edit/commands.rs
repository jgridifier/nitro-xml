// SPDX-License-Identifier: MIT
//! Command registry shared by chrome (#53), headless API and edit-script replay (#63).
//!
//! Shape matches upstream `Command` / `CommandRegistry` (id, label, editing, run).
//! Runners receive `&mut dyn Any` (the [`EditHost`]) so format crates (#66+) can downcast.

use std::any::Any;
use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;

use crate::edit::change_set::ChangeSet;
use crate::edit::error::EditError;

type CommandRunner =
    Arc<dyn Fn(&mut dyn Any, &Value) -> Result<ChangeSet, EditError> + Send + Sync>;

/// One registry entry (upstream `Command` without the Lit context).
#[derive(Clone)]
pub struct Command {
    pub id: String,
    pub label: String,
    /// When true, disabled in read-only mode.
    pub editing: bool,
    runner: Option<CommandRunner>,
}

impl Command {
    /// Metadata-only stub — `run` reports unsupported until a runner is attached (#66+).
    pub fn stub(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            label: label.into(),
            id,
            editing: true,
            runner: None,
        }
    }

    pub fn with_runner<F>(mut self, runner: F) -> Self
    where
        F: Fn(&mut dyn Any, &Value) -> Result<ChangeSet, EditError> + Send + Sync + 'static,
    {
        self.runner = Some(Arc::new(runner));
        self
    }

    pub fn run(&self, host: &mut dyn Any, arg: &Value) -> Result<ChangeSet, EditError> {
        match &self.runner {
            Some(runner) => runner(host, arg),
            None => Err(EditError::unsupported(&self.id)),
        }
    }
}

/// Upstream-shaped registry: ribbon, shortcuts and scripts share one map.
#[derive(Default, Clone)]
pub struct CommandRegistry {
    commands: BTreeMap<String, Command>,
    read_only: bool,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers stub entries for every `edits/schema.json` op id (still unsupported until runners attach).
    pub fn with_schema_stubs() -> Self {
        let mut reg = Self::new();
        for id in crate::edit::script::SCHEMA_OP_IDS {
            reg.register(Command::stub(*id, *id));
        }
        reg
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    pub fn register(&mut self, command: Command) {
        self.commands.insert(command.id.clone(), command);
    }

    pub fn register_all(&mut self, commands: Vec<Command>) {
        for command in commands {
            self.register(command);
        }
    }

    pub fn get(&self, id: &str) -> Option<&Command> {
        self.commands.get(id)
    }

    pub fn list(&self) -> Vec<&Command> {
        self.commands.values().collect()
    }

    pub fn ids(&self) -> Vec<&str> {
        self.commands.keys().map(String::as_str).collect()
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        match self.commands.get(id) {
            None => false,
            Some(cmd) if cmd.editing && self.read_only => false,
            Some(_) => true,
        }
    }

    /// Missing command or stub without runner → [`EditError::Unsupported`] (never a silent pass).
    pub fn run(&self, id: &str, host: &mut dyn Any, arg: &Value) -> Result<ChangeSet, EditError> {
        let Some(cmd) = self.commands.get(id) else {
            return Err(EditError::unsupported(id));
        };
        if cmd.editing && self.read_only {
            return Err(EditError::unsupported(id));
        }
        cmd.run(host, arg)
    }
}
