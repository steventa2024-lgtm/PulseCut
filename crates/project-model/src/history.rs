use crate::commands::{apply_command, EditCommand, EditError, EditOutcome};
use crate::model::Project;
use serde::Serialize;

pub const MAX_HISTORY: usize = 500;

#[derive(Clone, Debug)]
struct Entry {
    label: String,
    snapshot: Project,
}

/// Owns the authoritative project plus snapshot-based undo/redo. Snapshots make undo
/// restore the exact prior model (no inverse-operation drift); MVP projects are small
/// enough that this costs kilobytes per step.
#[derive(Clone, Debug)]
pub struct Editor {
    project: Project,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    revision: u64,
    saved_revision: Option<u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
    pub revision: u64,
    pub dirty: bool,
}

impl Editor {
    pub fn new(project: Project) -> Self {
        Editor {
            project,
            undo: vec![],
            redo: vec![],
            revision: 0,
            saved_revision: None,
        }
    }

    pub fn opened(project: Project) -> Self {
        Editor {
            saved_revision: Some(0),
            ..Editor::new(project)
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn apply(&mut self, cmd: &EditCommand) -> Result<EditOutcome, EditError> {
        let before = self.project.clone();
        let outcome = apply_command(&mut self.project, cmd)?;
        self.push_undo(outcome.label.clone(), before);
        Ok(outcome)
    }

    /// Non-command mutations (import, relink) that should still be undoable.
    pub fn mutate<T>(
        &mut self,
        label: &str,
        f: impl FnOnce(&mut Project) -> Result<T, EditError>,
    ) -> Result<T, EditError> {
        let before = self.project.clone();
        let mut work = before.clone();
        let out = f(&mut work)?;
        crate::validate::validate_project(&work).map_err(EditError::Invalid)?;
        self.project = work;
        self.push_undo(label.into(), before);
        Ok(out)
    }

    fn push_undo(&mut self, label: String, snapshot: Project) {
        self.undo.push(Entry { label, snapshot });
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision += 1;
    }

    pub fn undo(&mut self) -> Option<String> {
        let e = self.undo.pop()?;
        let current = std::mem::replace(&mut self.project, e.snapshot);
        self.redo.push(Entry {
            label: e.label.clone(),
            snapshot: current,
        });
        self.revision += 1;
        Some(e.label)
    }

    pub fn redo(&mut self) -> Option<String> {
        let e = self.redo.pop()?;
        let current = std::mem::replace(&mut self.project, e.snapshot);
        self.undo.push(Entry {
            label: e.label.clone(),
            snapshot: current,
        });
        self.revision += 1;
        Some(e.label)
    }

    pub fn mark_saved(&mut self) {
        self.saved_revision = Some(self.revision);
    }

    pub fn history(&self) -> HistoryState {
        HistoryState {
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            undo_label: self.undo.last().map(|e| e.label.clone()),
            redo_label: self.redo.last().map(|e| e.label.clone()),
            revision: self.revision,
            dirty: self.saved_revision != Some(self.revision),
        }
    }
}
