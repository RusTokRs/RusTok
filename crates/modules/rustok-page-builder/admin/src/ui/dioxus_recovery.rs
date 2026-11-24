use crate::model::AdminCanvasController;
use fly::GrapesJsCodec;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const RECOVERY_FORMAT: u8 = 1;
const MAX_RECOVERY_RECORD_BYTES: usize = 2 * 1024 * 1024;
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
    base_revision_id: String,
    base_project_data: Value,
    draft_project_data: Value,
    updated_at_ms: u64,
}

pub(crate) struct DraftRecoveryBootstrap {
    pub store: Option<DraftRecoveryStore>,
    pub base: DraftRecoveryBase,
    pub merged_document: Option<Value>,
    pub status: DraftRecoveryStatus,
    pub conflict_json: Option<String>,
    pub consumed_records: Vec<RecoveryRecordRef>,
    pub persisted: bool,
}

#[derive(Clone)]
pub(crate) struct DraftRecoveryStore {
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    prefix: String,
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    current_key: String,
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
            current_key: format!("{prefix}{writer_id}"),
            prefix,
        };
        // Probe storage during initialization so quota/security failures are surfaced before edits.
        let _ = store.storage()?;
        Ok(store)
    }

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    fn storage(&self) -> Result<web_sys::Storage, String> {
        let window = web_sys::window()
            .ok_or_else(|| "Browser window is unavailable.".to_string())?;
        window
            .local_storage()
            .map_err(|error| format!("Cannot access browser local storage: {error:?}"))?
            .ok_or_else(|| "Browser local storage is unavailable.".to_string())
    }

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    fn load_records(&self) -> Result<Vec<(RecoveryRecordRef, DraftRecoveryRecord)>, String> {
        let storage = self.storage()?;
        let length = storage
            .length()
            .map_err(|error| format!("Cannot enumerate browser recovery storage: {error:?}"))?;
        let mut records = Vec::new();
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
            let scope_hash = self
                .prefix
                .strip_prefix(RECOVERY_KEY_PREFIX)
                .and_then(|value| value.split(':').next())
                .unwrap_or_default();
            let page_hash = self.prefix.rsplit(':').nth(1).unwrap_or_default();
            if record.format != RECOVERY_FORMAT
                || record.scope_hash.as_str() != scope_hash
                || record.page_hash.as_str() != page_hash
                || key != format!("{}{}", self.prefix, record.writer_id)
            {
                return Err("A local recovery record has an unsupported identity or format.".to_string());
            }
            records.push((RecoveryRecordRef { key, raw }, record));
        }
        records.sort_by(|left, right| {
            left.1
                .updated_at_ms
                .cmp(&right.1.updated_at_ms)
                .then_with(|| left.1.writer_id.cmp(&right.1.writer_id))
        });
        Ok(records)
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
            let writer_id = self
                .current_key
                .strip_prefix(&self.prefix)
                .unwrap_or_default()
                .to_string();
            let record = DraftRecoveryRecord {
                format: RECOVERY_FORMAT,
                scope_hash,
                page_hash,
                writer_id,
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
            self.storage()?
                .set_item(&self.current_key, &raw)
                .map_err(|error| format!("Cannot persist local draft in browser storage: {error:?}"))
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
        {
            let _ = (self, base, draft_project_data);
            Err("Browser-local recovery is available only in a Dioxus web build.".to_string())
        }
    }

    pub(crate) fn clear_current(&self) -> Result<(), String> {
        #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
        {
            self.storage()?
                .remove_item(&self.current_key)
                .map_err(|error| format!("Cannot clear the saved local draft: {error:?}"))
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
        let records = match store.load_records() {
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
        if records.is_empty() {
            return DraftRecoveryBootstrap {
                store: Some(store),
                base,
                merged_document: None,
                status: DraftRecoveryStatus::Ready,
                conflict_json: None,
                consumed_records: Vec::new(),
                persisted: !controller.editor().revision().dirty,
            };
        }

        let consumed_records = records
            .iter()
            .map(|(reference, _)| reference.clone())
            .collect::<Vec<_>>();
        let mut merged = base.project_data.clone();
        let mut conflicts = BTreeSet::new();
        for (_, record) in &records {
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
                &records.iter().map(|(_, record)| record).collect::<Vec<_>>(),
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
    fn indexed<'a>(
        values: &'a [Value],
    ) -> Option<(Vec<String>, BTreeMap<String, &'a Value>)> {
        let mut order = Vec::with_capacity(values.len());
        let mut items = BTreeMap::new();
        for value in values {
            let id = value.get("id")?.as_str()?.trim();
            if id.is_empty() || items.contains_key(id) {
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

    let surviving_base = |order: &[String], present: &BTreeSet<String>| {
        order
            .iter()
            .filter(|id| base_ids.contains(*id) && present.contains(*id))
            .cloned()
            .collect::<Vec<_>>()
    };
    let server_base_order = surviving_base(&server_order, &server_ids);
    let local_base_order = surviving_base(&local_order, &local_ids);
    let expected_server_order = base_order
        .iter()
        .filter(|id| server_ids.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let expected_local_order = base_order
        .iter()
        .filter(|id| local_ids.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let server_reordered = server_base_order != expected_server_order;
    let local_reordered = local_base_order != expected_local_order;
    if server_reordered && local_reordered && server_base_order != local_base_order {
        conflicts.push(format!("{path}/$order"));
        return None;
    }

    let mut order = if local_reordered && !server_reordered {
        local_order.clone()
    } else {
        server_order.clone()
    };
    for id in &local_order {
        if !order.contains(id) {
            order.push(id.clone());
        }
    }
    for id in &server_order {
        if !order.contains(id) {
            order.push(id.clone());
        }
    }
    for id in merged_items.keys() {
        if !order.contains(id) {
            order.push(id.clone());
        }
    }
    Some(Value::Array(
        order
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
        let merged = merge_optional(Some(&base), Some(&server), Some(&local), "$", &mut conflicts)
            .expect("merge");
        assert!(conflicts.is_empty());
        assert_eq!(merged, Some(json!({"title":"server","theme":{"color":"red"}})));
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
        let merged = merge_optional(Some(&base), Some(&server), Some(&local), "$", &mut conflicts)
            .expect("merge");
        assert!(conflicts.is_empty());
        assert_eq!(merged, Some(json!({"components":[
            {"id":"a","content":"server"},
            {"id":"b","content":"local"}
        ]})));
    }

    #[test]
    fn three_way_merge_reports_overlapping_edits_without_overwriting_either_side() {
        let base = json!({"components":[{"id":"a","content":"one"}]});
        let server = json!({"components":[{"id":"a","content":"server"}]});
        let local = json!({"components":[{"id":"a","content":"local"}]});
        let mut conflicts = Vec::new();
        let merged = merge_optional(Some(&base), Some(&server), Some(&local), "$", &mut conflicts);
        assert!(merged.is_some());
        assert!(!conflicts.is_empty());
        assert!(conflicts.iter().any(|path| path.contains("id=a")));
    }
}
