use crate::AdminCanvasController;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SsrDraftSessionSnapshot {
    pub token: String,
    pub generation: u64,
    pub controller: AdminCanvasController,
    pub runtime_context: Value,
}

pub trait SsrDraftSessionStore: Send + Sync {
    fn load(
        &self,
        token: &str,
        page_id: &str,
    ) -> Result<Option<SsrDraftSessionSnapshot>, SsrDraftSessionError>;

    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError>;

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        let _ = runtime_context;
        self.commit(token, expected_generation, controller)
    }

    fn remove(&self, token: &str) -> Result<(), SsrDraftSessionError>;

    fn prune(&self) -> Result<usize, SsrDraftSessionError>;
}

#[derive(Clone)]
pub struct InMemorySsrDraftSessionStore {
    entries: Arc<RwLock<HashMap<String, DraftEntry>>>,
    ttl: Duration,
    maximum_entries: usize,
}

#[derive(Debug, Clone)]
struct DraftEntry {
    controller: AdminCanvasController,
    runtime_context: Value,
    generation: u64,
    expires_at: Instant,
}

impl InMemorySsrDraftSessionStore {
    pub fn new(ttl: Duration, maximum_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            ttl: ttl.max(Duration::from_secs(30)),
            maximum_entries: maximum_entries.max(1),
        }
    }

    pub fn editor_default() -> Self {
        Self::new(Duration::from_secs(60 * 60 * 8), 10_000)
    }

    pub fn len(&self) -> Result<usize, SsrDraftSessionError> {
        self.entries
            .read()
            .map(|entries| entries.len())
            .map_err(|_| SsrDraftSessionError::Poisoned)
    }

    fn fresh_token(entries: &HashMap<String, DraftEntry>) -> String {
        loop {
            let token = Uuid::new_v4().simple().to_string();
            if !entries.contains_key(&token) {
                return token;
            }
        }
    }

    fn empty_context() -> Value {
        Value::Object(Map::new())
    }

    fn prune_locked(entries: &mut HashMap<String, DraftEntry>, now: Instant) -> usize {
        let before = entries.len();
        entries.retain(|_, entry| entry.expires_at > now);
        before.saturating_sub(entries.len())
    }

    fn evict_oldest(entries: &mut HashMap<String, DraftEntry>, maximum_entries: usize) {
        while entries.len() >= maximum_entries {
            let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.expires_at)
                .map(|(token, _)| token.clone())
            else {
                break;
            };
            entries.remove(&oldest);
        }
    }

    fn commit_internal(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Option<Value>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = Instant::now();
        Self::prune_locked(&mut entries, now);

        if let Some(token) = token.map(str::trim).filter(|token| !token.is_empty()) {
            if let Some(entry) = entries.get_mut(token) {
                if entry.controller.page_id() != controller.page_id() {
                    return Err(SsrDraftSessionError::PageMismatch {
                        expected: entry.controller.page_id().to_string(),
                        actual: controller.page_id().to_string(),
                    });
                }
                if expected_generation.is_some_and(|generation| generation != entry.generation) {
                    return Err(SsrDraftSessionError::GenerationConflict {
                        expected: entry.generation,
                        actual: expected_generation.unwrap_or_default(),
                    });
                }
                entry.controller = controller;
                if let Some(runtime_context) = runtime_context {
                    entry.runtime_context = runtime_context;
                }
                entry.generation = entry.generation.saturating_add(1);
                entry.expires_at = now + self.ttl;
                return Ok(SsrDraftSessionSnapshot {
                    token: token.to_string(),
                    generation: entry.generation,
                    controller: entry.controller.clone(),
                    runtime_context: entry.runtime_context.clone(),
                });
            }
        }

        Self::evict_oldest(&mut entries, self.maximum_entries);
        let token = Self::fresh_token(&entries);
        let generation = 1;
        let runtime_context = runtime_context.unwrap_or_else(Self::empty_context);
        entries.insert(
            token.clone(),
            DraftEntry {
                controller: controller.clone(),
                runtime_context: runtime_context.clone(),
                generation,
                expires_at: now + self.ttl,
            },
        );
        Ok(SsrDraftSessionSnapshot {
            token,
            generation,
            controller,
            runtime_context,
        })
    }
}

impl Default for InMemorySsrDraftSessionStore {
    fn default() -> Self {
        Self::editor_default()
    }
}

impl SsrDraftSessionStore for InMemorySsrDraftSessionStore {
    fn load(
        &self,
        token: &str,
        page_id: &str,
    ) -> Result<Option<SsrDraftSessionSnapshot>, SsrDraftSessionError> {
        let token = token.trim();
        if token.is_empty() {
            return Ok(None);
        }
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = Instant::now();
        Self::prune_locked(&mut entries, now);
        let Some(entry) = entries.get_mut(token) else {
            return Ok(None);
        };
        if entry.controller.page_id() != page_id {
            return Err(SsrDraftSessionError::PageMismatch {
                expected: entry.controller.page_id().to_string(),
                actual: page_id.to_string(),
            });
        }
        entry.expires_at = now + self.ttl;
        Ok(Some(SsrDraftSessionSnapshot {
            token: token.to_string(),
            generation: entry.generation,
            controller: entry.controller.clone(),
            runtime_context: entry.runtime_context.clone(),
        }))
    }

    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_internal(token, expected_generation, controller, None)
    }

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_internal(
            token,
            expected_generation,
            controller,
            Some(runtime_context),
        )
    }

    fn remove(&self, token: &str) -> Result<(), SsrDraftSessionError> {
        self.entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?
            .remove(token.trim());
        Ok(())
    }

    fn prune(&self) -> Result<usize, SsrDraftSessionError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        Ok(Self::prune_locked(&mut entries, Instant::now()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SsrDraftSessionError {
    #[error("SSR draft session storage lock is poisoned")]
    Poisoned,
    #[error("SSR draft session belongs to page `{expected}`, not `{actual}`")]
    PageMismatch { expected: String, actual: String },
    #[error("SSR draft session generation conflict: expected `{expected}`, received `{actual}`")]
    GenerationConflict { expected: u64, actual: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_ui::UiIntent;
    use serde_json::json;

    fn controller(page_id: &str) -> AdminCanvasController {
        AdminCanvasController::new(
            page_id,
            "rev-1",
            json!({
                "pages": [{
                    "id": page_id,
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{ "id": "hero", "type": "section" }]
                    }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn session_preserves_editor_history_clipboard_and_runtime_context() {
        let store = InMemorySsrDraftSessionStore::default();
        let mut controller = controller("home");
        controller
            .dispatch(UiIntent::Select(Some("hero".to_string())))
            .unwrap();
        controller.dispatch(UiIntent::CopySelection).unwrap();
        let first = store
            .commit_with_context(
                None,
                None,
                controller,
                json!({ "customer": { "name": "Ada" } }),
            )
            .expect("create");
        assert!(first.controller.has_clipboard());

        let loaded = store
            .load(&first.token, "home")
            .expect("load")
            .expect("session");
        assert_eq!(loaded.generation, 1);
        assert!(loaded.controller.has_clipboard());
        assert_eq!(loaded.runtime_context["customer"]["name"], "Ada");
    }

    #[test]
    fn ordinary_commit_preserves_existing_runtime_context() {
        let store = InMemorySsrDraftSessionStore::default();
        let first = store
            .commit_with_context(None, None, controller("home"), json!({ "value": 42 }))
            .expect("create");
        let second = store
            .commit(Some(&first.token), Some(first.generation), first.controller)
            .expect("update controller");
        assert_eq!(second.runtime_context["value"], 42);
    }

    #[test]
    fn optimistic_generation_rejects_parallel_commit() {
        let store = InMemorySsrDraftSessionStore::default();
        let first = store
            .commit(None, None, controller("home"))
            .expect("create");
        let second = store
            .commit(
                Some(&first.token),
                Some(first.generation),
                first.controller.clone(),
            )
            .expect("update");
        assert_eq!(second.generation, 2);
        assert!(matches!(
            store.commit(Some(&first.token), Some(first.generation), first.controller,),
            Err(SsrDraftSessionError::GenerationConflict { .. })
        ));
    }

    #[test]
    fn session_token_cannot_cross_page_boundary() {
        let store = InMemorySsrDraftSessionStore::default();
        let first = store
            .commit(None, None, controller("home"))
            .expect("create");
        assert!(matches!(
            store.load(&first.token, "about"),
            Err(SsrDraftSessionError::PageMismatch { .. })
        ));
    }
}
