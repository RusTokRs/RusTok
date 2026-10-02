use super::patch::ComponentPatch;
use crate::{
    BindingCommand, ComponentNode, ContextCommand, DynamicCommand, FlyError, FlyResult,
    GrapesJsCodec, PageCommand, ProjectDocument, ProjectSnapshot, StyleRuleCommand,
    TranslationCommand,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;

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

impl HistoryEntry {
    /// Approximate retained size, measured as the serialized JSON length of both documents.
    ///
    /// Serializing to measure is not free, which is itself an argument for moving history to
    /// inverse commands; it is still far cheaper than the clones the entry already paid for.
    pub fn approximate_bytes(&self) -> usize {
        serde_json::to_vec(&self.before).map_or(0, |bytes| bytes.len())
            + serde_json::to_vec(&self.after).map_or(0, |bytes| bytes.len())
    }
}

/// Default ceiling on how much serialized document state undo history may retain.
///
/// Each entry currently stores a full before/after document pair, so an entry-count limit alone
/// lets a large project pin an unbounded amount of memory: 100 entries of a 5 MB project is a
/// gigabyte. The byte budget is the backstop until history moves to inverse commands.
pub const DEFAULT_HISTORY_MEMORY_BUDGET_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct History {
    limit: usize,
    #[serde(default = "default_history_memory_budget")]
    memory_budget_bytes: usize,
    /// Running total of `undo_sizes`, kept incrementally so eviction never re-measures.
    #[serde(default)]
    retained_bytes: usize,
    #[serde(default)]
    undo_sizes: VecDeque<usize>,
    undo: VecDeque<HistoryEntry>,
    redo: VecDeque<HistoryEntry>,
}

fn default_history_memory_budget() -> usize {
    DEFAULT_HISTORY_MEMORY_BUDGET_BYTES
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            memory_budget_bytes: DEFAULT_HISTORY_MEMORY_BUDGET_BYTES,
            retained_bytes: 0,
            undo_sizes: VecDeque::new(),
            undo: VecDeque::new(),
            redo: VecDeque::new(),
        }
    }

    /// Override how many bytes of retained document state history may hold.
    pub fn with_memory_budget(mut self, memory_budget_bytes: usize) -> Self {
        self.memory_budget_bytes = memory_budget_bytes.max(1);
        self
    }

    /// Approximate retained size of the undo stack, in bytes of serialized JSON.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    /// Configured memory ceiling for the undo stack.
    pub fn memory_budget_bytes(&self) -> usize {
        self.memory_budget_bytes
    }

    /// Rebuild the size index when it does not line up with the entries.
    ///
    /// Only reachable after deserializing a `History` written before the byte budget existed, in
    /// which case `undo_sizes` arrives empty while `undo` is populated.
    fn resync_sizes(&mut self) {
        self.undo_sizes = self
            .undo
            .iter()
            .map(HistoryEntry::approximate_bytes)
            .collect();
        self.retained_bytes = self.undo_sizes.iter().sum();
    }

    fn evict_oldest(&mut self) -> bool {
        if self.undo.pop_front().is_none() {
            return false;
        }
        let evicted = self.undo_sizes.pop_front().unwrap_or_default();
        self.retained_bytes = self.retained_bytes.saturating_sub(evicted);
        true
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
        if self.undo_sizes.len() != self.undo.len() {
            self.resync_sizes();
        }
        let size = entry.approximate_bytes();
        self.undo.push_back(entry);
        self.undo_sizes.push_back(size);
        self.retained_bytes = self.retained_bytes.saturating_add(size);
        self.redo.clear();

        // Oldest-first eviction. `VecDeque` keeps this O(1); the previous `Vec::remove(0)` shifted
        // every retained document on every command once the limit was reached.
        while self.undo.len() > self.limit && self.evict_oldest() {}
        while self.undo.len() > 1
            && self.retained_bytes > self.memory_budget_bytes
            && self.evict_oldest()
        {}
    }

    pub(super) fn pop_undo(&mut self) -> FlyResult<HistoryEntry> {
        if self.undo_sizes.len() != self.undo.len() {
            self.resync_sizes();
        }
        let entry = self.undo.pop_back().ok_or(FlyError::UndoHistoryEmpty)?;
        let size = self.undo_sizes.pop_back().unwrap_or_default();
        self.retained_bytes = self.retained_bytes.saturating_sub(size);
        Ok(entry)
    }

    pub(super) fn push_undo(&mut self, entry: HistoryEntry) {
        if self.undo_sizes.len() != self.undo.len() {
            self.resync_sizes();
        }
        let size = entry.approximate_bytes();
        self.undo.push_back(entry);
        self.undo_sizes.push_back(size);
        self.retained_bytes = self.retained_bytes.saturating_add(size);
    }

    pub(super) fn push_redo(&mut self, entry: HistoryEntry) {
        self.redo.push_back(entry);
    }

    pub(super) fn pop_redo(&mut self) -> FlyResult<HistoryEntry> {
        self.redo.pop_back().ok_or(FlyError::RedoHistoryEmpty)
    }
}

/// Cheap, non-cryptographic fingerprint of a project (FNV-1a 64).
///
/// # This is not an integrity primitive
///
/// `ProjectHash` exists for change detection: dirty flags, ETag-style comparison and optimistic
/// concurrency, where both sides are trusted and the only question is "did this change?".
///
/// It is 64-bit and non-cryptographic, so an attacker who controls a payload can produce a
/// collision on demand. Anything answering "is this payload the one that was approved?" must use
/// [`crate::ContentDigest`] (SHA-256) instead — see `ProjectSnapshot::content_digest` and
/// `ProjectBundle::content_digest`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectHash(pub u64);

impl ProjectHash {
    /// Fingerprint a document, falling back to the raw project encoding.
    ///
    /// Returns `None` when the document cannot be serialized at all. The previous version
    /// swallowed that case into `unwrap_or_default()`, which hashed an empty byte slice — so every
    /// unserializable document shared one fingerprint and compared equal to the others.
    pub fn try_from_document(document: &ProjectDocument) -> Option<Self> {
        let bytes = GrapesJsCodec::encode_vec(document)
            .ok()
            .or_else(|| serde_json::to_vec(&document.project).ok())?;
        Some(Self::from_bytes(&bytes))
    }

    pub fn from_document(document: &ProjectDocument) -> Self {
        // A `ProjectDocument` is built from `serde_json::Value`, so encoding it cannot fail in
        // practice; the sentinel keeps the infallible signature without silently colliding with
        // the hash of an empty document.
        Self::try_from_document(document).unwrap_or(Self(u64::MAX))
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
