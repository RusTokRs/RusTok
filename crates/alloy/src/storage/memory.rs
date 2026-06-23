use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::traits::{ScriptPage, ScriptQuery, ScriptRegistry};
use crate::error::{ScriptError, ScriptResult};
use crate::model::{Script, ScriptId, ScriptStatus, ScriptTrigger};

#[derive(Clone)]
pub struct InMemoryStorage {
    scripts: Arc<RwLock<HashMap<ScriptId, Script>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            scripts: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ScriptRegistry for InMemoryStorage {
    async fn find(&self, query: ScriptQuery) -> ScriptResult<Vec<Script>> {
        let guard = self.scripts.read().await;

        let mut result: Vec<Script> = match query {
            ScriptQuery::ById(id) => guard.get(&id).cloned().into_iter().collect(),
            ScriptQuery::ByName(name) => guard
                .values()
                .filter(|script| script.name == name)
                .cloned()
                .collect(),
            ScriptQuery::ByEvent { entity_type, event } => guard
                .values()
                .filter(|script| script.is_executable())
                .filter(|script| {
                    matches!(
                        &script.trigger,
                        ScriptTrigger::Event {
                            entity_type: stored_entity,
                            event: stored_event,
                        } if stored_entity == &entity_type && stored_event == &event
                    )
                })
                .cloned()
                .collect(),
            ScriptQuery::ByApiPath(path) => guard
                .values()
                .filter(|script| script.is_executable())
                .filter(|script| {
                    matches!(
                        &script.trigger,
                        ScriptTrigger::Api { path: stored_path, .. }
                            if stored_path == &path
                    )
                })
                .cloned()
                .collect(),
            ScriptQuery::Scheduled => guard
                .values()
                .filter(|script| script.is_executable())
                .filter(|script| matches!(script.trigger, ScriptTrigger::Cron { .. }))
                .cloned()
                .collect(),
            ScriptQuery::ByStatus(status) => guard
                .values()
                .filter(|script| script.status == status)
                .cloned()
                .collect(),
            ScriptQuery::All => guard.values().cloned().collect(),
        };

        result.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.cmp(&right.id))
        });

        Ok(result)
    }

    async fn find_paginated(
        &self,
        query: ScriptQuery,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<ScriptPage> {
        let all = self.find(query).await?;
        let total = all.len() as u64;
        let items = all
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok(ScriptPage { items, total })
    }

    async fn get(&self, id: ScriptId) -> ScriptResult<Script> {
        let guard = self.scripts.read().await;
        guard.get(&id).cloned().ok_or(ScriptError::NotFound {
            name: id.to_string(),
        })
    }

    async fn get_by_name(&self, name: &str) -> ScriptResult<Script> {
        let guard = self.scripts.read().await;
        guard
            .values()
            .find(|script| script.name == name)
            .cloned()
            .ok_or(ScriptError::NotFound {
                name: name.to_string(),
            })
    }

    async fn save(&self, mut script: Script) -> ScriptResult<Script> {
        let mut guard = self.scripts.write().await;

        if guard.contains_key(&script.id) {
            script.version += 1;
            script.updated_at = chrono::Utc::now();
        }

        guard.insert(script.id, script.clone());
        Ok(script)
    }

    async fn delete(&self, id: ScriptId) -> ScriptResult<()> {
        let mut guard = self.scripts.write().await;
        guard.remove(&id).ok_or(ScriptError::NotFound {
            name: id.to_string(),
        })?;
        Ok(())
    }

    async fn set_status(&self, id: ScriptId, status: ScriptStatus) -> ScriptResult<()> {
        let mut guard = self.scripts.write().await;
        let script = guard.get_mut(&id).ok_or(ScriptError::NotFound {
            name: id.to_string(),
        })?;
        script.status = status;
        script.updated_at = chrono::Utc::now();
        Ok(())
    }

    async fn record_error(&self, id: ScriptId) -> ScriptResult<bool> {
        let mut guard = self.scripts.write().await;
        let script = guard.get_mut(&id).ok_or(ScriptError::NotFound {
            name: id.to_string(),
        })?;

        let should_disable = script.register_error();
        if should_disable {
            script.status = ScriptStatus::Disabled;
        }

        Ok(should_disable)
    }

    async fn reset_errors(&self, id: ScriptId) -> ScriptResult<()> {
        let mut guard = self.scripts.write().await;
        let script = guard.get_mut(&id).ok_or(ScriptError::NotFound {
            name: id.to_string(),
        })?;
        script.reset_errors();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named_script(name: &str, status: ScriptStatus) -> Script {
        let mut script = Script::new(name, "40 + 2", ScriptTrigger::Manual);
        script.status = status;
        script
    }

    #[tokio::test]
    async fn find_returns_scripts_in_sea_orm_compatible_name_order() {
        let storage = InMemoryStorage::new();
        storage
            .save(named_script("zeta", ScriptStatus::Draft))
            .await
            .unwrap();
        storage
            .save(named_script("alpha", ScriptStatus::Active))
            .await
            .unwrap();
        storage
            .save(named_script("middle", ScriptStatus::Paused))
            .await
            .unwrap();

        let names = storage
            .find(ScriptQuery::All)
            .await
            .unwrap()
            .into_iter()
            .map(|script| script.name)
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["alpha", "middle", "zeta"]);
    }

    #[tokio::test]
    async fn paginated_status_query_keeps_total_and_name_order_after_filtering() {
        let storage = InMemoryStorage::new();
        storage
            .save(named_script("gamma_active", ScriptStatus::Active))
            .await
            .unwrap();
        storage
            .save(named_script("beta_draft", ScriptStatus::Draft))
            .await
            .unwrap();
        storage
            .save(named_script("alpha_active", ScriptStatus::Active))
            .await
            .unwrap();

        let page = storage
            .find_paginated(ScriptQuery::ByStatus(ScriptStatus::Active), 1, 1)
            .await
            .unwrap();

        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "gamma_active");
    }
}
