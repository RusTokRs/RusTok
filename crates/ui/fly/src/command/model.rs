use super::patch::ComponentPatch;
use crate::{
    BindingCommand, ComponentNode, ContextCommand, DynamicCommand, FlyError, FlyResult,
    GrapesJsCodec, PageCommand, ProjectDocument, ProjectSnapshot, StyleRuleCommand,
    TranslationCommand,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AssetCommand {
    Upsert { asset: Value },
    Remove { asset_id: String },
}

/// Project-level symbol definition operations.
///
/// Instances are ordinary `Insert` commands of a `symbol`-type component that
/// carries `symbolId`; these commands only manage definitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SymbolCommand {
    Upsert { symbol: Value },
    Remove { symbol_id: String },
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
    Symbol {
        command: SymbolCommand,
    },
    StyleRule {
        command: StyleRuleCommand,
    },
    Theme {
        command: crate::ThemeCommand,
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
    pub before: Arc<ProjectDocument>,
    pub after: Arc<ProjectDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse: Option<EditorCommand>,
}

impl HistoryEntry {
    pub fn inverse(&self) -> Option<&EditorCommand> {
        self.inverse.as_ref()
    }

    /// Conservative serialized-size estimate of what this entry really retains.
    ///
    /// Inverse commands are not used to reconstruct undo: both full snapshots remain in memory.
    /// Count them even when an inverse exists, and include commands as well. Adjacent entries may
    /// share an `Arc`, so summing entries can overestimate but must not pretend snapshots are free.
    /// JSON size is a resource proxy, not an exact measure of allocator/heap overhead.
    pub fn approximate_bytes(&self) -> usize {
        serialized_size(&self.before)
            .saturating_add(serialized_size(&self.after))
            .saturating_add(serialized_size(&self.command))
            .saturating_add(self.inverse.as_ref().map_or(0, serialized_size))
            .saturating_add(128)
    }
}

/// Count JSON bytes without allocating another full serialized document per command.
fn serialized_size(value: &impl Serialize) -> usize {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    if serde_json::to_writer(&mut counter, value).is_ok() {
        counter.0
    } else {
        // An unmeasurable snapshot must never silently bypass the memory budget.
        usize::MAX
    }
}

/// Default ceiling on how much serialized document state undo history may retain.
///
/// Each entry currently stores a full before/after document pair, so an entry-count limit alone
/// lets a large project pin an unbounded amount of memory: 100 entries of a 5 MB project is a
/// gigabyte. The byte budget is the backstop until history moves to inverse commands.
pub const DEFAULT_HISTORY_MEMORY_BUDGET_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(from = "StoredHistory")]
pub struct History {
    limit: usize,
    #[serde(default = "default_history_memory_budget")]
    memory_budget_bytes: usize,
    /// Running total of both stacks; moving an entry between them retains its snapshots.
    #[serde(default)]
    retained_bytes: usize,
    #[serde(default)]
    undo_sizes: VecDeque<usize>,
    #[serde(default)]
    redo_sizes: VecDeque<usize>,
    undo: VecDeque<HistoryEntry>,
    redo: VecDeque<HistoryEntry>,
}

// Older serialized histories may contain the old delta-only accounting. Never trust that index on
// load: serde also recreates the Arcs independently, so rebuild from the actual stored entries.
#[derive(Deserialize)]
struct StoredHistory {
    limit: usize,
    #[serde(default = "default_history_memory_budget")]
    memory_budget_bytes: usize,
    #[serde(default)]
    undo: VecDeque<HistoryEntry>,
    #[serde(default)]
    redo: VecDeque<HistoryEntry>,
}

impl From<StoredHistory> for History {
    fn from(stored: StoredHistory) -> Self {
        let mut history = Self {
            limit: stored.limit.max(1),
            memory_budget_bytes: stored.memory_budget_bytes.max(1),
            retained_bytes: 0,
            undo_sizes: VecDeque::new(),
            redo_sizes: VecDeque::new(),
            undo: stored.undo,
            redo: stored.redo,
        };
        history.resync_sizes();
        history.enforce_budget();
        history
    }
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
            redo_sizes: VecDeque::new(),
            undo: VecDeque::new(),
            redo: VecDeque::new(),
        }
    }

    /// Override the budget and evict old entries immediately if it was lowered.
    pub fn with_memory_budget(mut self, memory_budget_bytes: usize) -> Self {
        self.memory_budget_bytes = memory_budget_bytes.max(1);
        self.enforce_budget();
        self
    }

    /// Conservative serialized-size estimate for both undo and redo stacks.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    /// Configured serialized-size budget for both history stacks.
    pub fn memory_budget_bytes(&self) -> usize {
        self.memory_budget_bytes
    }

    fn resync_sizes(&mut self) {
        self.undo_sizes = self
            .undo
            .iter()
            .map(HistoryEntry::approximate_bytes)
            .collect();
        self.redo_sizes = self
            .redo
            .iter()
            .map(HistoryEntry::approximate_bytes)
            .collect();
        self.retained_bytes = self
            .undo_sizes
            .iter()
            .chain(&self.redo_sizes)
            .fold(0usize, |total, size| total.saturating_add(*size));
    }

    fn account_removed(&mut self, size: usize) {
        if self.retained_bytes == usize::MAX {
            // Saturated sums cannot be subtracted safely; recompute after the removal.
            self.resync_sizes();
        } else {
            self.retained_bytes = self.retained_bytes.saturating_sub(size);
        }
    }

    fn evict_oldest_undo(&mut self) -> bool {
        if self.undo.pop_front().is_none() {
            return false;
        }
        let size = self.undo_sizes.pop_front().unwrap_or_default();
        self.account_removed(size);
        true
    }

    fn evict_farthest_redo(&mut self) -> bool {
        if self.redo.pop_front().is_none() {
            return false;
        }
        let size = self.redo_sizes.pop_front().unwrap_or_default();
        self.account_removed(size);
        true
    }

    fn enforce_budget(&mut self) {
        while self.undo.len() + self.redo.len() > self.limit {
            if !self.evict_oldest_undo() && !self.evict_farthest_redo() {
                break;
            }
        }
        // Keep one undo or redo entry even when a single document exceeds the budget. Redo's
        // front is farthest in the future; dropping it preserves a contiguous redo sequence.
        while self.undo.len() + self.redo.len() > 1
            && self.retained_bytes > self.memory_budget_bytes
        {
            if !self.evict_oldest_undo() && !self.evict_farthest_redo() {
                break;
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Undo entries, oldest first.
    pub fn undo_entries(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.undo.iter()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub(super) fn push(&mut self, entry: HistoryEntry) {
        // A new edit invalidates redo and releases every future snapshot it held.
        self.redo.clear();
        let removed = self
            .redo_sizes
            .drain(..)
            .fold(0usize, |total, size| total.saturating_add(size));
        self.account_removed(removed);
        let size = entry.approximate_bytes();
        self.undo.push_back(entry);
        self.undo_sizes.push_back(size);
        self.retained_bytes = self.retained_bytes.saturating_add(size);
        self.enforce_budget();
    }

    pub(super) fn pop_undo(&mut self) -> FlyResult<(HistoryEntry, usize)> {
        let entry = self.undo.pop_back().ok_or(FlyError::UndoHistoryEmpty)?;
        let size = self.undo_sizes.pop_back().unwrap_or_default();
        self.account_removed(size);
        Ok((entry, size))
    }

    pub(super) fn push_undo(&mut self, entry: HistoryEntry, size: usize) {
        self.undo.push_back(entry);
        self.undo_sizes.push_back(size);
        self.retained_bytes = self.retained_bytes.saturating_add(size);
    }

    pub(super) fn push_redo(&mut self, entry: HistoryEntry, size: usize) {
        self.redo.push_back(entry);
        self.redo_sizes.push_back(size);
        self.retained_bytes = self.retained_bytes.saturating_add(size);
    }

    pub(super) fn pop_redo(&mut self) -> FlyResult<(HistoryEntry, usize)> {
        let entry = self.redo.pop_back().ok_or(FlyError::RedoHistoryEmpty)?;
        let size = self.redo_sizes.pop_back().unwrap_or_default();
        self.account_removed(size);
        Ok((entry, size))
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
    /// A save was rejected because the document changed elsewhere (lost update). The
    /// local changes are still unsaved; only a successful acknowledge clears this.
    pub save_conflict: bool,
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
            save_conflict: false,
            command_sequence: 0,
            last_acknowledged_revision: None,
            project_hash: document.hash(),
            save_in_progress: false,
            save_failed: false,
        }
    }

    /// Records a document mutation.
    ///
    /// `save_failed` is deliberately left untouched: after a rejected save the changes are
    /// still not on the server, and a further edit does not change that. Only the next save
    /// attempt ([`Self::begin_save`]) or a successful [`Self::acknowledge`] clears the signal.
    pub(super) fn mark_changed(&mut self, document: &ProjectDocument) {
        self.dirty = true;
        self.command_sequence = self.command_sequence.saturating_add(1);
        self.project_hash = document.hash();
    }

    pub fn begin_save(&mut self) {
        self.save_in_progress = true;
        self.save_failed = false;
    }

    pub fn fail_save(&mut self) {
        self.save_in_progress = false;
        self.save_failed = true;
        self.save_conflict = false;
    }

    /// Record a lost update: the save was rejected because another session saved first.
    /// `dirty` stays set — the local changes exist and are not on the server.
    pub fn mark_save_conflict(&mut self) {
        self.save_in_progress = false;
        self.save_failed = true;
        self.save_conflict = true;
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
        self.save_conflict = false;
        Ok(())
    }
}
