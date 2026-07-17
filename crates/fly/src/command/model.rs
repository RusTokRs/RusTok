use super::patch::ComponentPatch;
use crate::{
    BindingCommand, ComponentNode, ContextCommand, DynamicCommand, FlyError, FlyResult,
    GrapesJsV1Codec, PageCommand, ProjectDocument, ProjectSnapshot, StyleRuleCommand,
    TranslationCommand,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AssetCommand {
    Upsert { asset: Value },
    Remove { asset_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditorCommand {
    Select {
        component_id: Option<String>,
    },
    Insert {
        parent_id: Option<String>,
        index: usize,
        component: ComponentNode,
    },
    Remove {
        component_id: String,
    },
    Move {
        component_id: String,
        new_parent_id: Option<String>,
        index: usize,
    },
    Patch {
        component_id: String,
        patch: ComponentPatch,
    },
    Asset {
        command: AssetCommand,
    },
    StyleRule {
        command: StyleRuleCommand,
    },
    Page {
        command: PageCommand,
    },
    Dynamic {
        command: DynamicCommand,
    },
    Binding {
        command: BindingCommand,
    },
    Context {
        command: ContextCommand,
    },
    Translation {
        command: TranslationCommand,
    },
    RestoreSnapshot {
        snapshot: Box<ProjectSnapshot>,
    },
    Batch {
        commands: Vec<EditorCommand>,
    },
}

impl EditorCommand {
    pub fn batch(commands: impl IntoIterator<Item = EditorCommand>) -> Self {
        Self::Batch {
            commands: commands.into_iter().collect(),
        }
    }

    pub fn restore_snapshot(snapshot: ProjectSnapshot) -> Self {
        Self::RestoreSnapshot {
            snapshot: Box::new(snapshot),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryEntry {
    pub command: EditorCommand,
    pub before: ProjectDocument,
    pub after: ProjectDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct History {
    limit: usize,
    pub(super) undo: Vec<HistoryEntry>,
    pub(super) redo: Vec<HistoryEntry>,
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub(super) fn push(&mut self, entry: HistoryEntry) {
        self.undo.push(entry);
        self.redo.clear();
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    pub(super) fn pop_undo(&mut self) -> FlyResult<HistoryEntry> {
        self.undo.pop().ok_or(FlyError::UndoHistoryEmpty)
    }

    pub(super) fn pop_redo(&mut self) -> FlyResult<HistoryEntry> {
        self.redo.pop().ok_or(FlyError::RedoHistoryEmpty)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectHash(pub u64);

impl ProjectHash {
    pub fn from_document(document: &ProjectDocument) -> Self {
        let bytes = GrapesJsV1Codec::encode_vec(document)
            .unwrap_or_else(|_| serde_json::to_vec(&document.project).unwrap_or_default());
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Self(hash)
    }

    pub fn hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevisionState {
    pub dirty: bool,
    pub command_sequence: u64,
    pub last_acknowledged_revision: Option<String>,
    pub project_hash: ProjectHash,
    pub save_in_progress: bool,
    pub save_failed: bool,
}

impl RevisionState {
    pub fn new(document: &ProjectDocument) -> Self {
        Self {
            dirty: false,
            command_sequence: 0,
            last_acknowledged_revision: None,
            project_hash: document.hash(),
            save_in_progress: false,
            save_failed: false,
        }
    }

    pub(super) fn mark_changed(&mut self, document: &ProjectDocument) {
        self.dirty = true;
        self.command_sequence = self.command_sequence.saturating_add(1);
        self.project_hash = document.hash();
        self.save_failed = false;
    }

    pub fn begin_save(&mut self) {
        self.save_in_progress = true;
        self.save_failed = false;
    }

    pub fn fail_save(&mut self) {
        self.save_in_progress = false;
        self.save_failed = true;
    }

    pub fn acknowledge(
        &mut self,
        expected_hash: ProjectHash,
        revision: impl Into<String>,
    ) -> FlyResult<()> {
        if self.project_hash != expected_hash {
            return Err(FlyError::RevisionConflict {
                expected: expected_hash.hex(),
                actual: self.project_hash.hex(),
            });
        }
        self.last_acknowledged_revision = Some(revision.into());
        self.dirty = false;
        self.save_in_progress = false;
        self.save_failed = false;
        Ok(())
    }
}