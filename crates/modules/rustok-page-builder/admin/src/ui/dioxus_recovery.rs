use crate::model::AdminCanvasController;
use fly::GrapesJsCodec;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use std::{cell::Cell, rc::Rc};

const RECOVERY_FORMAT: u8 = 1;
const MAX_RECOVERY_RECORD_BYTES: usize = 2 * 1024 * 1024;
const MAX_RECOVERY_SUPERSEDED_KEYS: usize = 256;
const MAX_RECOVERY_RECORDS_PER_SCOPE: usize = 256;
const RECOVERY_KEY_PREFIX: &str = "rustok:fly:draft:v1:";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftRecoveryBase {
    pub revision_id: String,
    pub project_data: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DraftRecoveryStatus {
    Ready,
    Restored { draft_count: usize },
    Merged { draft_count: usize },
    Conflict { paths: Vec<String> },
    Disabled(String),
    Error(String),
    SaveError(String),
}

#[derive(Clone)]
pub(crate) struct RecoveryRecordRef {
    key: String,
    raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DraftRecoveryRecord {
    format: u8,
    scope_hash: String,
    page_hash: String,
    writer_id: String,
    /// Empty only for records written by the original fixed-key format.
    #[serde(default)]
    record_id: String,
    /// Monotonic within one mounted writer; timestamps alone can tie within a millisecond.
    #[serde(default)]
    sequence: u64,
    /// Exact older storage keys consumed by this recovery lineage.
    #[serde(default)]
    supersedes: Vec<String>,
    base_revision_id: String,
    base_project_data: Value,
    draft_project_data: Value,
    updated_at_ms: u64,
}

#[derive(Clone)]
pub(crate) struct DraftRecoveryBootstrap {
    pub store: Option<DraftRecoveryStore>,
    pub base: DraftRecoveryBase,
    pub merged_document: Option<Value>,
    pub status: DraftRecoveryStatus,
    pub conflict_json: Option<String>,
    pub consumed_records: Vec<RecoveryRecordRef>,
    pub persisted: bool,
}

struct LoadedRecoveryRecord {
    record: DraftRecoveryRecord,
}

struct LoadedRecoveryRecords {
    records: Vec<LoadedRecoveryRecord>,
    references: Vec<RecoveryRecordRef>,
    supersedes_keys: BTreeSet<String>,
}

fn recovery_record_storage_key(prefix: &str, writer_id: &str, record_id: &str) -> String {
    if record_id.is_empty() {
        // Read compatibility for the original one-key-per-writer format.
        format!("{prefix}{writer_id}")
    } else {
        format!("{prefix}{writer_id}:{record_id}")
    }
}

fn valid_recovery_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_recovery_storage_key(prefix: &str, key: &str) -> bool {
    let Some(suffix) = key.strip_prefix(prefix) else {
        return false;
    };
    match suffix.split_once(':') {
        Some((writer_id, record_id)) => {
            valid_recovery_token(writer_id) && valid_recovery_token(record_id)
        }
        None => valid_recovery_token(suffix),
    }
}

fn recovery_record_is_newer(
    candidate: &DraftRecoveryRecord,
    current: &DraftRecoveryRecord,
) -> bool {
    (
        candidate.sequence,
        candidate.updated_at_ms,
        candidate.record_id.as_str(),
    ) > (
        current.sequence,
        current.updated_at_ms,
        current.record_id.as_str(),
    )
}

/// Validate the complete persisted lineage before projecting one record per writer.
/// Superseded records can still participate in a cycle, and an unrelated live
/// record must not hide that corruption.
fn recovery_lineage_has_cycle(entries: &[(RecoveryRecordRef, DraftRecoveryRecord)]) -> bool {
    let keys = entries
        .iter()
        .map(|(reference, _)| reference.key.clone())
        .collect::<BTreeSet<_>>();
    let mut outgoing = BTreeMap::<String, BTreeSet<String>>::new();
    let mut indegree = keys
        .iter()
        .cloned()
        .map(|key| (key, 0usize))
        .collect::<BTreeMap<_, _>>();

    for (reference, record) in entries {
        let edges = outgoing.entry(reference.key.clone()).or_default();
        for superseded_key in &record.supersedes {
            if !keys.contains(superseded_key) || !edges.insert(superseded_key.clone()) {
                continue;
            }
            let Some(degree) = indegree.get_mut(superseded_key) else {
                return true;
            };
            let Some(next) = degree.checked_add(1) else {
                return true;
            };
            *degree = next;
        }
    }

    // Kahn's algorithm examines every connected component. A disconnected
    // cycle is still fatal even when another writer has a valid latest record.
    let mut ready = indegree
        .iter()
        .filter_map(|(key, degree)| (*degree == 0).then_some(key.clone()))
        .collect::<BTreeSet<_>>();
    let mut visited = 0usize;

    while let Some(key) = ready.iter().next().cloned() {
        ready.remove(&key);
        let Some(next_visited) = visited.checked_add(1) else {
            return true;
        };
        visited = next_visited;

        if let Some(superseded_keys) = outgoing.get(&key) {
            for superseded_key in superseded_keys {
                let Some(degree) = indegree.get_mut(superseded_key) else {
                    return true;
                };
                let Some(next) = degree.checked_sub(1) else {
                    return true;
                };
                *degree = next;
                if next == 0 {
                    ready.insert(superseded_key.clone());
                }
            }
        }
    }

    visited != keys.len()
}

fn latest_recovery_records(
    entries: Vec<(RecoveryRecordRef, DraftRecoveryRecord)>,
) -> Result<LoadedRecoveryRecords, String> {
    if recovery_lineage_has_cycle(&entries) {
        return Err(
            "Local recovery records contain a cyclic supersession chain; all records were left untouched."
                .to_string(),
        );
    }

    let mut references = Vec::with_capacity(entries.len());
    let mut by_writer = BTreeMap::<String, (String, DraftRecoveryRecord)>::new();

    for (reference, record) in entries {
        let key = reference.key.clone();
        references.push(reference);
        match by_writer.entry(record.writer_id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert((key, record));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if recovery_record_is_newer(&record, &entry.get().1) {
                    entry.insert((key, record));
                }
            }
        }
    }

    let supersedes_keys = by_writer
        .values()
        .flat_map(|(_, record)| record.supersedes.iter().cloned())
        .collect::<BTreeSet<_>>();
    let writer_count = by_writer.len();
    let mut records = by_writer
        .into_values()
        .filter_map(|(key, record)| {
            (!supersedes_keys.contains(&key)).then_some(LoadedRecoveryRecord { record })
        })
        .collect::<Vec<_>>();

    if writer_count > 0 && records.is_empty() {
        return Err(
            "Local recovery records contain a cyclic supersession chain; all records were left untouched."
                .to_string(),
        );
    }

    records.sort_by(|left, right| {
        left.record
            .updated_at_ms
            .cmp(&right.record.updated_at_ms)
            .then_with(|| left.record.writer_id.cmp(&right.record.writer_id))
    });
    Ok(LoadedRecoveryRecords {
        records,
        references,
        supersedes_keys,
    })
}

#[derive(Clone)]
pub(crate) struct DraftRecoveryStore {
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    prefix: String,
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    writer_id: String,
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    sequence: Rc<Cell<u64>>,
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    supersedes_keys: Rc<RefCell<BTreeSet<String>>>,
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    consumed_records: Rc<RefCell<Vec<RecoveryRecordRef>>>,
}

impl DraftRecoveryStore {
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    fn new(scope: &str, page_id: &str) -> Result<Self, String> {
        if scope.trim().is_empty() || scope.len() > 256 {
            return Err("The recovery scope is empty or too long.".to_string());
        }
        if page_id.trim().is_empty() {
            return Err("The page id is empty.".to_string());
        }
        let scope_hash = fly::sha256_hex(scope.as_bytes());
        let page_hash = fly::sha256_hex(page_id.as_bytes());
        let prefix = format!("{RECOVERY_KEY_PREFIX}{scope_hash}:{page_hash}:");
        let writer_id = format!(
            "{:x}-{:x}-{:x}",
            js_sys::Date::now() as u64,
            (js_sys::Math::random() * u64::MAX as f64) as u64,
            (js_sys::Math::random() * u64::MAX as f64) as u64
        );
        let store = Self {
            prefix,
            writer_id,
            sequence: Rc::new(Cell::new(0)),
            supersedes_keys: Rc::new(RefCell::new(BTreeSet::new())),
            consumed_records: Rc::new(RefCell::new(Vec::new())),
        };
        // Probe storage during initialization so quota/security failures are surfaced before edits.
        let _ = store.storage()?;
        Ok(store)
    }

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    fn set_recovery_lineage(
        &self,
        supersedes_keys: BTreeSet<String>,
        consumed_records: Vec<RecoveryRecordRef>,
    ) -> Result<(), String> {
        if supersedes_keys.len() > MAX_RECOVERY_SUPERSEDED_KEYS {
            return Err(format!(
                "The local recovery lineage exceeds the {MAX_RECOVERY_SUPERSEDED_KEYS}-record safety limit; existing records were left untouched."
            ));
        }
        *self.supersedes_keys.borrow_mut() = supersedes_keys;
        *self.consumed_records.borrow_mut() = consumed_records;
        Ok(())
    }

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    fn storage(&self) -> Result<web_sys::Storage, String> {
        let window =
            web_sys::window().ok_or_else(|| "Browser window is unavailable.".to_string())?;
        window
            .local_storage()
            .map_err(|error| format!("Cannot access browser local storage: {error:?}"))?
            .ok_or_else(|| "Browser local storage is unavailable.".to_string())
    }

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    fn load_records(&self) -> Result<LoadedRecoveryRecords, String> {
        let storage = self.storage()?;
        let length = storage
            .length()
            .map_err(|error| format!("Cannot enumerate browser recovery storage: {error:?}"))?;
        let scope_hash = self
            .prefix
            .strip_prefix(RECOVERY_KEY_PREFIX)
            .and_then(|value| value.split(':').next())
            .unwrap_or_default();
        let page_hash = self.prefix.rsplit(':').nth(1).unwrap_or_default();
        let mut entries = Vec::<(RecoveryRecordRef, DraftRecoveryRecord)>::new();

        for index in 0..length {
            let Some(key) = storage
                .key(index)
                .map_err(|error| format!("Cannot read browser recovery key: {error:?}"))?
            else {
                continue;
            };
            if !key.starts_with(&self.prefix) {
                continue;
            }
            let Some(raw) = storage
                .get_item(&key)
                .map_err(|error| format!("Cannot read a local draft: {error:?}"))?
            else {
                continue;
            };
            if raw.len() > MAX_RECOVERY_RECORD_BYTES {
                return Err(format!(
                    "A local draft exceeds the {} MiB recovery limit and was left untouched.",
                    MAX_RECOVERY_RECORD_BYTES / 1024 / 1024
                ));
            }
            let record: DraftRecoveryRecord = serde_json::from_str(&raw)
                .map_err(|error| format!("A local recovery record is invalid: {error}"))?;
            if record.format != RECOVERY_FORMAT
                || record.scope_hash.as_str() != scope_hash
                || record.page_hash.as_str() != page_hash
                || !valid_recovery_token(&record.writer_id)
                || (!record.record_id.is_empty() && !valid_recovery_token(&record.record_id))
                || record.supersedes.len() > MAX_RECOVERY_SUPERSEDED_KEYS
                || record.supersedes.iter().any(|superseded_key| {
                    superseded_key.len() > 512
                        || !valid_recovery_storage_key(&self.prefix, superseded_key)
                })
                || key
                    != recovery_record_storage_key(
                        &self.prefix,
                        &record.writer_id,
                        &record.record_id,
                    )
            {
                return Err(
                    "A local recovery record has an unsupported identity or format.".to_string(),
                );
            }

            if entries.len() >= MAX_RECOVERY_RECORDS_PER_SCOPE {
                return Err(
                    "The local recovery scope contains too many records; existing records were left untouched."
                        .to_string(),
                );
            }
            entries.push((RecoveryRecordRef { key, raw }, record));
        }

        latest_recovery_records(entries)
    }

    pub(crate) fn save(
        &self,
        base: &DraftRecoveryBase,
        draft_project_data: Value,
    ) -> Result<(), String> {
        #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
        {
            let scope_hash = self
                .prefix
                .strip_prefix(RECOVERY_KEY_PREFIX)
                .and_then(|value| value.split(':').next())
                .unwrap_or_default()
                .to_string();
            let page_hash = self
                .prefix
                .rsplit(':')
                .nth(1)
                .unwrap_or_default()
                .to_string();
            let sequence =
                self.sequence.get().checked_add(1).ok_or_else(|| {
                    "The local recovery writer sequence is exhausted.".to_string()
                })?;
            let record_id = format!(
                "{sequence:x}-{:x}-{:x}",
                js_sys::Date::now().max(0.0) as u64,
                (js_sys::Math::random() * u64::MAX as f64) as u64
            );
            let record = DraftRecoveryRecord {
                format: RECOVERY_FORMAT,
                scope_hash,
                page_hash,
                writer_id: self.writer_id.clone(),
                record_id: record_id.clone(),
                sequence,
                supersedes: self.supersedes_keys.borrow().iter().cloned().collect(),
                base_revision_id: base.revision_id.clone(),
                base_project_data: base.project_data.clone(),
                draft_project_data,
                updated_at_ms: js_sys::Date::now().max(0.0) as u64,
            };
            let raw = serde_json::to_string(&record)
                .map_err(|error| format!("Cannot serialize local draft recovery: {error}"))?;
            if raw.len() > MAX_RECOVERY_RECORD_BYTES {
                return Err(format!(
                    "This draft is larger than the {} MiB browser recovery limit. Copy its JSON before reloading.",
                    MAX_RECOVERY_RECORD_BYTES / 1024 / 1024
                ));
            }

            let storage = self.storage()?;
            let key = recovery_record_storage_key(&self.prefix, &self.writer_id, &record_id);
            storage.set_item(&key, &raw).map_err(|error| {
                format!("Cannot persist local draft in browser storage: {error:?}")
            })?;
            self.sequence.set(sequence);

            // Never overwrite a key another tab may have read. The new immutable record is
            // committed first; older versions for this writer can then be retired safely.
            match self.writer_record_keys(&storage) {
                Ok(keys) => {
                    for old_key in keys.into_iter().filter(|old_key| old_key != &key) {
                        if let Err(error) = storage.remove_item(&old_key) {
                            tracing::warn!(
                                error = ?error,
                                "Local draft was saved but an older immutable recovery record could not be retired"
                            );
                        }
                    }
                }
                Err(error) => tracing::warn!(
                    error = %error,
                    "Local draft was saved but older immutable recovery records could not be enumerated"
                ),
            }
            Ok(())
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
        {
            let _ = (self, base, draft_project_data);
            Err("Browser-local recovery is available only in a Dioxus web build.".to_string())
        }
    }

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    fn writer_record_keys(&self, storage: &web_sys::Storage) -> Result<Vec<String>, String> {
        let length = storage
            .length()
            .map_err(|error| format!("Cannot enumerate local drafts: {error:?}"))?;
        let legacy_key = recovery_record_storage_key(&self.prefix, &self.writer_id, "");
        let versioned_prefix = format!("{}{}:", self.prefix, self.writer_id);
        let mut keys = Vec::new();
        for index in 0..length {
            if let Some(key) = storage
                .key(index)
                .map_err(|error| format!("Cannot enumerate local drafts: {error:?}"))?
                && (key == legacy_key || key.starts_with(&versioned_prefix))
            {
                keys.push(key);
            }
        }
        Ok(keys)
    }

    pub(crate) fn clear_current(&self) -> Result<(), String> {
        #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
        {
            let consumed_records = self.consumed_records.borrow().clone();
            self.clear_if_unchanged(&consumed_records)?;
            let storage = self.storage()?;
            for key in self.writer_record_keys(&storage)? {
                storage
                    .remove_item(&key)
                    .map_err(|error| format!("Cannot clear the saved local draft: {error:?}"))?;
            }
            Ok(())
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
        {
            let _ = self;
            Ok(())
        }
    }

    pub(crate) fn clear_if_unchanged(
        &self,
        references: &[RecoveryRecordRef],
    ) -> Result<(), String> {
        #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
        {
            let storage = self.storage()?;
            // Each write has an immutable key. A peer can add a newer key, but cannot
            // replace the bytes at a key we observed, so this comparison cannot race a newer save.
            for reference in references {
                if storage
                    .get_item(&reference.key)
                    .map_err(|error| format!("Cannot verify a recovered draft: {error:?}"))?
                    .as_deref()
                    == Some(reference.raw.as_str())
                {
                    storage
                        .remove_item(&reference.key)
                        .map_err(|error| format!("Cannot retire a recovered draft: {error:?}"))?;
                }
            }
            Ok(())
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
        {
            let _ = (self, references);
            Ok(())
        }
    }

    pub(crate) fn clear_all(&self) -> Result<(), String> {
        #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
        {
            let storage = self.storage()?;
            let length = storage
                .length()
                .map_err(|error| format!("Cannot enumerate local drafts: {error:?}"))?;
            let mut keys = Vec::new();
            for index in 0..length {
                if let Some(key) = storage
                    .key(index)
                    .map_err(|error| format!("Cannot enumerate local drafts: {error:?}"))?
                    && key.starts_with(&self.prefix)
                {
                    keys.push(key);
                }
            }
            for key in keys {
                storage
                    .remove_item(&key)
                    .map_err(|error| format!("Cannot discard a local draft: {error:?}"))?;
            }
            Ok(())
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
        {
            let _ = self;
            Ok(())
        }
    }
}

pub(crate) fn initialize_draft_recovery(
    scope: Option<&str>,
    controller: &AdminCanvasController,
) -> DraftRecoveryBootstrap {
    let base = DraftRecoveryBase {
        revision_id: controller.revision_id().to_string(),
        project_data: match GrapesJsCodec::encode_value(controller.editor().document()) {
            Ok(value) => value,
            Err(error) => {
                return DraftRecoveryBootstrap {
                    store: None,
                    base: DraftRecoveryBase {
                        revision_id: controller.revision_id().to_string(),
                        project_data: Value::Null,
                    },
                    merged_document: None,
                    status: DraftRecoveryStatus::Error(format!(
                        "Cannot snapshot the server document for local recovery: {error}"
                    )),
                    conflict_json: None,
                    consumed_records: Vec::new(),
                    persisted: false,
                };
            }
        },
    };

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    {
        let Some(scope) = scope else {
            return DraftRecoveryBootstrap {
                store: None,
                base,
                merged_document: None,
                status: DraftRecoveryStatus::Disabled(
                    "The host did not provide an opaque tenant/user recovery scope.".to_string(),
                ),
                conflict_json: None,
                consumed_records: Vec::new(),
                persisted: false,
            };
        };
        let store = match DraftRecoveryStore::new(scope, controller.page_id()) {
            Ok(store) => store,
            Err(error) => {
                return DraftRecoveryBootstrap {
                    store: None,
                    base,
                    merged_document: None,
                    status: DraftRecoveryStatus::Error(error),
                    conflict_json: None,
                    consumed_records: Vec::new(),
                    persisted: false,
                };
            }
        };
        let loaded = match store.load_records() {
            Ok(records) => records,
            Err(error) => {
                return DraftRecoveryBootstrap {
                    store: Some(store),
                    base,
                    merged_document: None,
                    status: DraftRecoveryStatus::Error(error),
                    conflict_json: None,
                    consumed_records: Vec::new(),
                    persisted: false,
                };
            }
        };
        let mut supersedes_keys = loaded.supersedes_keys.clone();
        supersedes_keys.extend(
            loaded
                .references
                .iter()
                .map(|reference| reference.key.clone()),
        );
        let consumed_records = loaded.references;
        if let Err(error) = store.set_recovery_lineage(supersedes_keys, consumed_records.clone()) {
            return DraftRecoveryBootstrap {
                store: Some(store),
                base,
                merged_document: None,
                status: DraftRecoveryStatus::Error(error),
                conflict_json: None,
                consumed_records,
                persisted: false,
            };
        }
        let records = loaded.records;
        if records.is_empty() {
            return DraftRecoveryBootstrap {
                store: Some(store),
                base,
                merged_document: None,
                status: DraftRecoveryStatus::Ready,
                conflict_json: None,
                consumed_records,
                persisted: !controller.editor().revision().dirty,
            };
        }

        let mut merged = base.project_data.clone();
        let mut conflicts = BTreeSet::new();
        for loaded in &records {
            let record = &loaded.record;
            if record.base_revision_id.trim().is_empty()
                || record.base_project_data.is_null()
                || record.draft_project_data.is_null()
            {
                conflicts.insert("$".to_string());
                continue;
            }
            let mut record_conflicts = Vec::new();
            let candidate = merge_optional(
                Some(&record.base_project_data),
                Some(&merged),
                Some(&record.draft_project_data),
                "$",
                &mut record_conflicts,
            );
            conflicts.extend(record_conflicts);
            if let Some(Some(value)) = candidate {
                merged = value;
            }
        }
        if !conflicts.is_empty() {
            let conflict_json = serde_json::to_string_pretty(
                &records
                    .iter()
                    .map(|loaded| &loaded.record)
                    .collect::<Vec<_>>(),
            )
            .ok();
            return DraftRecoveryBootstrap {
                store: Some(store),
                base,
                merged_document: None,
                status: DraftRecoveryStatus::Conflict {
                    paths: conflicts.into_iter().collect(),
                },
                conflict_json,
                consumed_records,
                persisted: true,
            };
        }

        let changed = merged != base.project_data;
        let (status, persisted) = if changed {
            (
                if records.len() > 1 {
                    DraftRecoveryStatus::Merged {
                        draft_count: records.len(),
                    }
                } else {
                    DraftRecoveryStatus::Restored { draft_count: 1 }
                },
                true,
            )
        } else {
            match store.clear_if_unchanged(&consumed_records) {
                Ok(()) => (DraftRecoveryStatus::Ready, true),
                Err(error) => (DraftRecoveryStatus::Error(error), false),
            }
        };
        DraftRecoveryBootstrap {
            store: Some(store),
            base,
            merged_document: changed.then_some(merged),
            status,
            conflict_json: None,
            consumed_records,
            persisted,
        }
    }

    #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
    {
        let _ = scope;
        DraftRecoveryBootstrap {
            store: None,
            base,
            merged_document: None,
            status: DraftRecoveryStatus::Disabled(
                "Browser-local recovery is available only in a Dioxus web build.".to_string(),
            ),
            conflict_json: None,
            consumed_records: Vec::new(),
            persisted: false,
        }
    }
}

/// A conservative three-way merge. Objects merge recursively; arrays of uniquely identified
/// objects merge by id; all other simultaneous edits to the same value are reported as conflicts.
fn merge_optional(
    base: Option<&Value>,
    server: Option<&Value>,
    local: Option<&Value>,
    path: &str,
    conflicts: &mut Vec<String>,
) -> Option<Option<Value>> {
    if local == base {
        return Some(server.cloned());
    }
    if server == base || server == local {
        return Some(local.cloned());
    }

    match (base, server, local) {
        (Some(Value::Object(base)), Some(Value::Object(server)), Some(Value::Object(local))) => {
            let keys = base
                .keys()
                .chain(server.keys())
                .chain(local.keys())
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut merged = Map::new();
            for key in keys {
                let child_path = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                match merge_optional(
                    base.get(&key),
                    server.get(&key),
                    local.get(&key),
                    &child_path,
                    conflicts,
                ) {
                    Some(Some(value)) => {
                        merged.insert(key, value);
                    }
                    Some(None) => {}
                    None => {}
                }
            }
            Some(Some(Value::Object(merged)))
        }
        (Some(Value::Array(base)), Some(Value::Array(server)), Some(Value::Array(local))) => {
            merge_arrays_by_id(base, server, local, path, conflicts).map(Some)
        }
        _ => {
            conflicts.push(path.to_string());
            None
        }
    }
}

fn merge_arrays_by_id(
    base: &[Value],
    server: &[Value],
    local: &[Value],
    path: &str,
    conflicts: &mut Vec<String>,
) -> Option<Value> {
    fn indexed<'a>(values: &'a [Value]) -> Option<(Vec<String>, BTreeMap<String, &'a Value>)> {
        let mut order = Vec::with_capacity(values.len());
        let mut items = BTreeMap::new();
        for value in values {
            // Identity is exact. Trimming here aliases different authored IDs
            // and can turn a delete/insert into an edit of the previous item.
            let id = value.get("id")?.as_str()?;
            if id.trim().is_empty() || items.contains_key(id) {
                return None;
            }
            order.push(id.to_string());
            items.insert(id.to_string(), value);
        }
        Some((order, items))
    }

    let Some((base_order, base_items)) = indexed(base) else {
        conflicts.push(path.to_string());
        return None;
    };
    let Some((server_order, server_items)) = indexed(server) else {
        conflicts.push(path.to_string());
        return None;
    };
    let Some((local_order, local_items)) = indexed(local) else {
        conflicts.push(path.to_string());
        return None;
    };

    let base_ids = base_order.iter().cloned().collect::<BTreeSet<_>>();
    let server_ids = server_order.iter().cloned().collect::<BTreeSet<_>>();
    let local_ids = local_order.iter().cloned().collect::<BTreeSet<_>>();
    let keys = base_ids
        .union(&server_ids)
        .cloned()
        .chain(local_ids.difference(&base_ids).cloned())
        .collect::<BTreeSet<_>>();
    let mut merged_items = BTreeMap::<String, Value>::new();
    for id in keys {
        let child_path = format!("{path}[id={id}]");
        match merge_optional(
            base_items.get(&id).copied(),
            server_items.get(&id).copied(),
            local_items.get(&id).copied(),
            &child_path,
            conflicts,
        ) {
            Some(Some(value)) => {
                merged_items.insert(id, value);
            }
            Some(None) | None => {}
        }
    }

    // Compare relative order only among base items surviving the merge. A move
    // involving a node deleted by the other writer imposes no ordering constraint
    // on the final array and must not mask a real reorder of the remaining items.
    let surviving_base = |order: &[String]| {
        order
            .iter()
            .filter(|id| base_ids.contains(*id) && merged_items.contains_key(*id))
            .cloned()
            .collect::<Vec<_>>()
    };
    let merged_original_base_order = surviving_base(&base_order);
    let merged_server_base_order = surviving_base(&server_order);
    let merged_local_base_order = surviving_base(&local_order);
    let server_reordered = merged_server_base_order != merged_original_base_order;
    let local_reordered = merged_local_base_order != merged_original_base_order;
    if server_reordered && local_reordered && merged_server_base_order != merged_local_base_order {
        conflicts.push(format!("{path}/$order"));
        return None;
    }

    let merged_base_order = if local_reordered && !server_reordered {
        merged_local_base_order
    } else if server_reordered {
        merged_server_base_order
    } else {
        merged_original_base_order
    };
    let surviving_base_ids = merged_base_order.iter().cloned().collect::<BTreeSet<_>>();
    let merged_item_ids = merged_items.keys().cloned().collect::<BTreeSet<_>>();

    type Gap = (Option<String>, Option<String>);

    fn insertion_groups(
        order: &[String],
        base_ids: &BTreeSet<String>,
        surviving_base_ids: &BTreeSet<String>,
        merged_item_ids: &BTreeSet<String>,
    ) -> (BTreeMap<Gap, Vec<String>>, BTreeMap<String, Gap>) {
        let mut next_base_at = vec![None; order.len()];
        let mut next_base = None;
        for index in (0..order.len()).rev() {
            next_base_at[index] = next_base.clone();
            if surviving_base_ids.contains(&order[index]) {
                next_base = Some(order[index].clone());
            }
        }

        let mut previous_base = None;
        let mut groups = BTreeMap::<Gap, Vec<String>>::new();
        let mut positions = BTreeMap::<String, Gap>::new();
        for (index, id) in order.iter().enumerate() {
            if base_ids.contains(id) {
                if surviving_base_ids.contains(id) {
                    previous_base = Some(id.clone());
                }
                continue;
            }
            if !merged_item_ids.contains(id) {
                continue;
            }
            let gap = (previous_base.clone(), next_base_at[index].clone());
            groups.entry(gap.clone()).or_default().push(id.clone());
            positions.insert(id.clone(), gap);
        }
        (groups, positions)
    }

    fn merge_insertion_sequences(server: &[String], local: &[String]) -> Option<Vec<String>> {
        let all_ids = server
            .iter()
            .chain(local.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let server_positions = server
            .iter()
            .enumerate()
            .map(|(index, id)| (id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let local_positions = local
            .iter()
            .enumerate()
            .map(|(index, id)| (id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let priority = |id: &String| {
            (
                server_positions.get(id).copied().unwrap_or(usize::MAX),
                local_positions.get(id).copied().unwrap_or(usize::MAX),
                id.clone(),
            )
        };

        let mut outgoing = BTreeMap::<String, BTreeSet<String>>::new();
        let mut indegree = all_ids
            .iter()
            .cloned()
            .map(|id| (id, 0usize))
            .collect::<BTreeMap<_, _>>();
        for sequence in [server, local] {
            for pair in sequence.windows(2) {
                if outgoing
                    .entry(pair[0].clone())
                    .or_default()
                    .insert(pair[1].clone())
                {
                    *indegree.get_mut(&pair[1])? += 1;
                }
            }
        }

        let mut ready = BTreeSet::new();
        for id in &all_ids {
            if indegree.get(id).copied()? == 0 {
                ready.insert(priority(id));
            }
        }

        let mut merged = Vec::with_capacity(all_ids.len());
        while let Some(next_ready) = ready.iter().next().cloned() {
            ready.remove(&next_ready);
            let (_, _, id) = next_ready;
            merged.push(id.clone());
            if let Some(next_ids) = outgoing.get(&id) {
                for next_id in next_ids {
                    let degree = indegree.get_mut(next_id)?;
                    *degree = degree.checked_sub(1)?;
                    if *degree == 0 {
                        ready.insert(priority(next_id));
                    }
                }
            }
        }

        (merged.len() == all_ids.len()).then_some(merged)
    }

    fn gap_is_valid(gap: &Gap, positions: &BTreeMap<String, usize>, base_len: usize) -> bool {
        let previous = match gap.0.as_ref() {
            Some(id) => match positions.get(id) {
                Some(index) => Some(*index),
                None => return false,
            },
            None => None,
        };
        let next = match gap.1.as_ref() {
            Some(id) => match positions.get(id) {
                Some(index) => Some(*index),
                None => return false,
            },
            None => None,
        };

        match (previous, next) {
            (None, None) => base_len == 0,
            (None, Some(next)) => next == 0,
            (Some(previous), None) => previous + 1 == base_len,
            (Some(previous), Some(next)) => previous + 1 == next,
        }
    }

    let (server_groups, server_positions) = insertion_groups(
        &server_order,
        &base_ids,
        &surviving_base_ids,
        &merged_item_ids,
    );
    let (local_groups, local_positions) = insertion_groups(
        &local_order,
        &base_ids,
        &surviving_base_ids,
        &merged_item_ids,
    );

    // The same newly-created id cannot be placed at two different gaps.
    for (id, server_gap) in &server_positions {
        if local_positions
            .get(id)
            .is_some_and(|local_gap| local_gap != server_gap)
        {
            conflicts.push(format!("{path}/$order"));
            return None;
        }
    }

    let base_positions = merged_base_order
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    if server_groups
        .keys()
        .chain(local_groups.keys())
        .any(|gap| !gap_is_valid(gap, &base_positions, merged_base_order.len()))
    {
        // A side inserted between anchors that the other side reordered or separated.
        // Do not silently move that component to a different position.
        conflicts.push(format!("{path}/$order"));
        return None;
    }

    let mut ordered_ids = Vec::with_capacity(merged_items.len());
    for gap_index in 0..=merged_base_order.len() {
        let gap = (
            gap_index
                .checked_sub(1)
                .and_then(|index| merged_base_order.get(index).cloned()),
            merged_base_order.get(gap_index).cloned(),
        );
        let server_insertions = server_groups.get(&gap).map(Vec::as_slice).unwrap_or(&[]);
        let local_insertions = local_groups.get(&gap).map(Vec::as_slice).unwrap_or(&[]);
        let Some(insertions) = merge_insertion_sequences(server_insertions, local_insertions)
        else {
            conflicts.push(format!("{path}/$order"));
            return None;
        };
        ordered_ids.extend(insertions);

        if let Some(base_id) = merged_base_order.get(gap_index) {
            ordered_ids.push(base_id.clone());
        }
    }

    if ordered_ids.len() != merged_items.len() {
        conflicts.push(format!("{path}/$order"));
        return None;
    }

    Some(Value::Array(
        ordered_ids
            .into_iter()
            .filter_map(|id| merged_items.remove(&id))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn three_way_merge_keeps_non_overlapping_object_edits() {
        let base = json!({"title":"old","theme":{"color":"blue"}});
        let server = json!({"title":"server","theme":{"color":"blue"}});
        let local = json!({"title":"old","theme":{"color":"red"}});
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        )
        .expect("merge");
        assert!(conflicts.is_empty());
        assert_eq!(
            merged,
            Some(json!({"title":"server","theme":{"color":"red"}}))
        );
    }

    #[test]
    fn three_way_merge_combines_changes_to_different_component_ids() {
        let base = json!({"components":[
            {"id":"a","content":"one"},
            {"id":"b","content":"two"}
        ]});
        let server = json!({"components":[
            {"id":"a","content":"server"},
            {"id":"b","content":"two"}
        ]});
        let local = json!({"components":[
            {"id":"a","content":"one"},
            {"id":"b","content":"local"}
        ]});
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        )
        .expect("merge");
        assert!(conflicts.is_empty());
        assert_eq!(
            merged,
            Some(json!({"components":[
                {"id":"a","content":"server"},
                {"id":"b","content":"local"}
            ]}))
        );
    }

    #[test]
    fn three_way_merge_never_aliases_ids_that_differ_by_whitespace() {
        let base = json!({"items":[{"id":"a","content":"original"}]});
        let server = json!({"items":[{"id":" a ","content":"original"}]});
        let local = json!({"items":[{"id":"a","content":"local edit"}]});
        let mut conflicts = Vec::new();
        let _ = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        );
        assert!(conflicts.iter().any(|path| path.contains("id=a")));

        // Independent insertions with distinct exact IDs must not be collapsed.
        let base = json!({"items":[]});
        let server = json!({"items":[{"id":"a","content":"server"}]});
        let local = json!({"items":[{"id":" a ","content":"local"}]});
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        )
        .expect("merge");
        assert!(conflicts.is_empty());
        assert_eq!(
            merged,
            Some(json!({"items":[
                {"id":"a","content":"server"},
                {"id":" a ","content":"local"}
            ]}))
        );
    }

    #[test]
    fn three_way_merge_reports_overlapping_edits_without_overwriting_either_side() {
        let base = json!({"components":[{"id":"a","content":"one"}]});
        let server = json!({"components":[{"id":"a","content":"server"}]});
        let local = json!({"components":[{"id":"a","content":"local"}]});
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        );
        assert!(merged.is_some());
        assert!(!conflicts.is_empty());
        assert!(conflicts.iter().any(|path| path.contains("id=a")));
    }

    #[test]
    fn recovery_record_keys_are_immutable_per_write_and_keep_legacy_reads() {
        let prefix = "rustok:fly:draft:v1:scope:page:";
        let writer = "writer-1";
        let legacy = recovery_record_storage_key(prefix, writer, "");
        let first = recovery_record_storage_key(prefix, writer, "1-a");
        let second = recovery_record_storage_key(prefix, writer, "2-b");

        assert_eq!(legacy, format!("{prefix}{writer}"));
        assert_ne!(first, second);
        assert_ne!(legacy, first);
        assert!(valid_recovery_storage_key(prefix, &legacy));
        assert!(valid_recovery_storage_key(prefix, &first));
        assert!(!valid_recovery_storage_key(
            prefix,
            &format!("{prefix}bad:token:extra")
        ));
        assert!(!valid_recovery_storage_key(
            prefix,
            "other-scope:writer:1-a"
        ));
    }

    #[test]
    fn legacy_recovery_records_default_to_the_original_fixed_key_identity() {
        let record: DraftRecoveryRecord = serde_json::from_value(json!({
            "format": 1,
            "scope_hash": "scope",
            "page_hash": "page",
            "writer_id": "writer-1",
            "base_revision_id": "rev-1",
            "base_project_data": { "pages": [] },
            "draft_project_data": { "pages": [] },
            "updated_at_ms": 100
        }))
        .expect("legacy recovery record");
        assert_eq!(record.record_id, "");
        assert_eq!(record.sequence, 0);
        assert_eq!(
            recovery_record_storage_key("prefix:", &record.writer_id, &record.record_id),
            "prefix:writer-1"
        );
    }

    #[test]
    fn recovery_writer_sequence_wins_over_clock_skew_when_selecting_latest_record() {
        let base = DraftRecoveryRecord {
            format: RECOVERY_FORMAT,
            scope_hash: "scope".to_string(),
            page_hash: "page".to_string(),
            writer_id: "writer-1".to_string(),
            record_id: "1-a".to_string(),
            sequence: 1,
            supersedes: Vec::new(),
            base_revision_id: "rev-1".to_string(),
            base_project_data: json!({ "pages": [] }),
            draft_project_data: json!({ "pages": [] }),
            updated_at_ms: 200,
        };
        let mut newer = base.clone();
        newer.record_id = "2-b".to_string();
        newer.sequence = 2;
        newer.updated_at_ms = 100;
        assert!(recovery_record_is_newer(&newer, &base));
    }

    #[test]
    fn three_way_merge_preserves_independent_insertions_at_their_shared_gap() {
        let base = json!([
            { "id": "a", "content": "A" },
            { "id": "b", "content": "B" }
        ]);
        let server = json!([
            { "id": "a", "content": "A" },
            { "id": "server", "content": "S" },
            { "id": "b", "content": "B" }
        ]);
        let local = json!([
            { "id": "a", "content": "A" },
            { "id": "local", "content": "L" },
            { "id": "b", "content": "B" }
        ]);
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        )
        .expect("merge");
        assert!(conflicts.is_empty());
        assert_eq!(
            merged,
            Some(json!([
                { "id": "a", "content": "A" },
                { "id": "server", "content": "S" },
                { "id": "local", "content": "L" },
                { "id": "b", "content": "B" }
            ]))
        );
    }

    #[test]
    fn three_way_merge_preserves_front_and_end_insertions() {
        let base = json!([
            { "id": "a", "content": "A" },
            { "id": "b", "content": "B" }
        ]);
        let server = json!([
            { "id": "front", "content": "F" },
            { "id": "a", "content": "A" },
            { "id": "b", "content": "B" }
        ]);
        let local = json!([
            { "id": "a", "content": "A" },
            { "id": "b", "content": "B" },
            { "id": "end", "content": "E" }
        ]);
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        )
        .expect("merge");
        assert!(conflicts.is_empty());
        assert_eq!(
            merged,
            Some(json!([
                { "id": "front", "content": "F" },
                { "id": "a", "content": "A" },
                { "id": "b", "content": "B" },
                { "id": "end", "content": "E" }
            ]))
        );
    }

    #[test]
    fn three_way_merge_rejects_insertions_whose_anchors_were_reordered() {
        let base = json!([
            { "id": "a", "content": "A" },
            { "id": "b", "content": "B" }
        ]);
        let server = json!([
            { "id": "b", "content": "B" },
            { "id": "a", "content": "A" }
        ]);
        let local = json!([
            { "id": "a", "content": "A" },
            { "id": "local", "content": "L" },
            { "id": "b", "content": "B" }
        ]);
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        );
        assert!(merged.is_none());
        assert!(conflicts.iter().any(|path| path == "$/$order"));
    }

    #[test]
    fn three_way_merge_rejects_the_same_new_id_at_different_positions() {
        let base = json!([
            { "id": "a", "content": "A" },
            { "id": "b", "content": "B" }
        ]);
        let server = json!([
            { "id": "a", "content": "A" },
            { "id": "shared", "content": "S" },
            { "id": "b", "content": "B" }
        ]);
        let local = json!([
            { "id": "shared", "content": "S" },
            { "id": "a", "content": "A" },
            { "id": "b", "content": "B" }
        ]);
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        );
        assert!(merged.is_none());
        assert!(conflicts.iter().any(|path| path == "$/$order"));
    }

    #[test]
    fn three_way_merge_ignores_reorders_of_items_deleted_by_the_other_side() {
        let base = json!([
            { "id": "a", "content": "A" },
            { "id": "b", "content": "B" },
            { "id": "c", "content": "C" }
        ]);
        let server = json!([
            { "id": "b", "content": "B" },
            { "id": "a", "content": "A" }
        ]);
        let local = json!([
            { "id": "c", "content": "C" },
            { "id": "b", "content": "B" }
        ]);
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        )
        .expect("merge");
        assert!(conflicts.is_empty());
        assert_eq!(merged, Some(json!([{ "id": "b", "content": "B" }])));
    }

    #[test]
    fn three_way_merge_preserves_surviving_reorder_when_other_writer_only_moved_deleted_item() {
        let base = json!([
            { "id": "a" }, { "id": "b" }, { "id": "c" }, { "id": "d" }
        ]);
        let moved_deleted = json!([
            { "id": "d" }, { "id": "a" }, { "id": "b" }, { "id": "c" }
        ]);
        let surviving_reorder = json!([
            { "id": "b" }, { "id": "a" }, { "id": "c" }
        ]);
        for (server, local) in [
            (&moved_deleted, &surviving_reorder),
            (&surviving_reorder, &moved_deleted),
        ] {
            let mut conflicts = Vec::new();
            let merged =
                merge_optional(Some(&base), Some(server), Some(local), "$", &mut conflicts);
            assert!(conflicts.is_empty(), "{conflicts:?}");
            assert_eq!(merged, Some(Some(surviving_reorder.clone())));
        }
    }

    #[test]
    fn three_way_merge_still_rejects_incompatible_surviving_reorders() {
        let base = json!([
            { "id": "a" }, { "id": "b" }, { "id": "c" }, { "id": "d" }
        ]);
        let server = json!([
            { "id": "b" }, { "id": "a" }, { "id": "c" }, { "id": "d" }
        ]);
        let local = json!([
            { "id": "a" }, { "id": "c" }, { "id": "b" }
        ]);
        let mut conflicts = Vec::new();
        assert!(
            merge_optional(
                Some(&base),
                Some(&server),
                Some(&local),
                "$",
                &mut conflicts
            )
            .is_none()
        );
        assert_eq!(conflicts, vec!["$/$order"]);
    }

    fn test_recovery_record(
        writer_id: &str,
        record_id: &str,
        sequence: u64,
        supersedes: &[&str],
    ) -> DraftRecoveryRecord {
        DraftRecoveryRecord {
            format: RECOVERY_FORMAT,
            scope_hash: "scope".to_string(),
            page_hash: "page".to_string(),
            writer_id: writer_id.to_string(),
            record_id: record_id.to_string(),
            sequence,
            supersedes: supersedes.iter().map(|key| (*key).to_string()).collect(),
            base_revision_id: "rev-1".to_string(),
            base_project_data: json!({ "pages": [] }),
            draft_project_data: json!({ "pages": [] }),
            updated_at_ms: sequence,
        }
    }

    fn test_record_ref(key: &str) -> RecoveryRecordRef {
        RecoveryRecordRef {
            key: key.to_string(),
            raw: key.to_string(),
        }
    }

    #[test]
    fn recovery_lineage_supersedes_pre_reload_writer_without_merging_it_twice() {
        let loaded = latest_recovery_records(vec![
            (
                test_record_ref("a1"),
                test_recovery_record("writer-a", "a1", 1, &[]),
            ),
            (
                test_record_ref("a2"),
                test_recovery_record("writer-a", "a2", 2, &["a1"]),
            ),
            (
                test_record_ref("b1"),
                test_recovery_record("writer-b", "b1", 1, &["a1", "a2"]),
            ),
        ])
        .expect("valid recovery lineage");

        assert_eq!(loaded.records.len(), 1);
        assert_eq!(loaded.records[0].record.writer_id, "writer-b");
        assert_eq!(loaded.references.len(), 3);
        assert!(loaded.supersedes_keys.contains("a1"));
        assert!(loaded.supersedes_keys.contains("a2"));
    }

    #[test]
    fn recovery_lineage_keeps_a_new_peer_write_after_the_superseded_snapshot() {
        let loaded = latest_recovery_records(vec![
            (
                test_record_ref("a1"),
                test_recovery_record("writer-a", "a1", 1, &[]),
            ),
            (
                test_record_ref("a2"),
                test_recovery_record("writer-a", "a2", 2, &["a1"]),
            ),
            (
                test_record_ref("b1"),
                test_recovery_record("writer-b", "b1", 1, &["a1", "a2"]),
            ),
            (
                test_record_ref("a3"),
                test_recovery_record("writer-a", "a3", 3, &["a1"]),
            ),
        ])
        .expect("concurrent latest writer snapshots");

        assert_eq!(loaded.records.len(), 2);
        assert!(
            loaded
                .records
                .iter()
                .any(|loaded| loaded.record.writer_id == "writer-a"
                    && loaded.record.record_id == "a3")
        );
        assert!(
            loaded
                .records
                .iter()
                .any(|loaded| loaded.record.writer_id == "writer-b"
                    && loaded.record.record_id == "b1")
        );
    }

    #[test]
    fn cyclic_recovery_lineage_fails_closed_without_discarding_records() {
        let result = latest_recovery_records(vec![
            (
                test_record_ref("a1"),
                test_recovery_record("writer-a", "a1", 1, &["b1"]),
            ),
            (
                test_record_ref("b1"),
                test_recovery_record("writer-b", "b1", 1, &["a1"]),
            ),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn recovery_lineage_rejects_cycles_hidden_by_newer_and_unrelated_records() {
        let result = latest_recovery_records(vec![
            (
                test_record_ref("a1"),
                test_recovery_record("writer-a", "a1", 1, &["b1"]),
            ),
            (
                test_record_ref("a2"),
                test_recovery_record("writer-a", "a2", 2, &["a1"]),
            ),
            (
                test_record_ref("b1"),
                test_recovery_record("writer-b", "b1", 1, &["a1"]),
            ),
            (
                test_record_ref("c1"),
                test_recovery_record("writer-c", "c1", 1, &[]),
            ),
        ]);

        assert!(
            result.is_err(),
            "a cycle in superseded history must not be hidden by a newer record or an unrelated writer"
        );
    }

    #[test]
    fn three_way_merge_rejects_reordered_shared_insertions_in_one_gap() {
        let base = json!([
            { "id": "a", "content": "A" },
            { "id": "b", "content": "B" }
        ]);
        let server = json!([
            { "id": "a", "content": "A" },
            { "id": "x", "content": "X" },
            { "id": "y", "content": "Y" },
            { "id": "b", "content": "B" }
        ]);
        let local = json!([
            { "id": "a", "content": "A" },
            { "id": "y", "content": "Y" },
            { "id": "x", "content": "X" },
            { "id": "b", "content": "B" }
        ]);
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        );
        assert!(merged.is_none());
        assert!(conflicts.iter().any(|path| path == "$/$order"));
    }

    #[test]
    fn three_way_merge_preserves_both_writers_order_constraints_for_same_gap_insertions() {
        let base = json!([
            { "id": "a", "content": "A" },
            { "id": "b", "content": "B" }
        ]);
        let server = json!([
            { "id": "a", "content": "A" },
            { "id": "x", "content": "X" },
            { "id": "y", "content": "Y" },
            { "id": "b", "content": "B" }
        ]);
        let local = json!([
            { "id": "a", "content": "A" },
            { "id": "x", "content": "X" },
            { "id": "z", "content": "Z" },
            { "id": "y", "content": "Y" },
            { "id": "b", "content": "B" }
        ]);
        let mut conflicts = Vec::new();
        let merged = merge_optional(
            Some(&base),
            Some(&server),
            Some(&local),
            "$",
            &mut conflicts,
        )
        .expect("merge");
        assert!(conflicts.is_empty());
        assert_eq!(
            merged,
            Some(json!([
                { "id": "a", "content": "A" },
                { "id": "x", "content": "X" },
                { "id": "z", "content": "Z" },
                { "id": "y", "content": "Y" },
                { "id": "b", "content": "B" }
            ]))
        );
    }
}
