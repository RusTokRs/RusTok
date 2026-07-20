use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::traits::{ScriptPage, ScriptQuery, ScriptRegistry};
use crate::error::{ScriptError, ScriptResult};
use crate::model::{
    validate_transition, ReviewCommand, ReviewDecision, Script, ScriptId, ScriptSourceRevision,
    ScriptStatus, ScriptTrigger, TestCommand, TestRun, TestRunClaim, TestRunCompletion,
    TestRunLease, TestRunStatus,
};

#[derive(Clone)]
pub struct InMemoryStorage {
    scripts: Arc<RwLock<HashMap<ScriptId, Script>>>,
    source_revisions: Arc<RwLock<HashMap<(ScriptId, u32), ScriptSourceRevision>>>,
    reviews: Arc<RwLock<HashMap<(ScriptId, u32), Vec<ReviewDecision>>>>,
    test_runs: Arc<RwLock<HashMap<(ScriptId, u32, uuid::Uuid), TestRun>>>,
    test_leases: Arc<RwLock<HashMap<uuid::Uuid, (uuid::Uuid, chrono::DateTime<chrono::Utc>)>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            scripts: Arc::new(RwLock::new(HashMap::new())),
            source_revisions: Arc::new(RwLock::new(HashMap::new())),
            reviews: Arc::new(RwLock::new(HashMap::new())),
            test_runs: Arc::new(RwLock::new(HashMap::new())),
            test_leases: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn source_revision(script: &Script) -> ScriptSourceRevision {
        ScriptSourceRevision {
            script_id: script.id,
            tenant_id: script.tenant_id,
            revision: script.version,
            parent_revision: script.version.checked_sub(1).filter(|parent| *parent > 0),
            source_digest: script
                .workspace
                .digest()
                .expect("saved workspace must have been validated"),
            workspace: script.workspace.clone(),
            author_id: script.author_id.clone(),
            created_at: script.updated_at,
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

    async fn get_source_revision(
        &self,
        id: ScriptId,
        revision: u32,
    ) -> ScriptResult<ScriptSourceRevision> {
        let guard = self.source_revisions.read().await;
        guard
            .get(&(id, revision))
            .cloned()
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{id}@{revision}"),
            })
    }

    async fn list_source_revisions(&self, id: ScriptId) -> ScriptResult<Vec<ScriptSourceRevision>> {
        let guard = self.source_revisions.read().await;
        let mut revisions = guard
            .values()
            .filter(|revision| revision.script_id == id)
            .cloned()
            .collect::<Vec<_>>();
        revisions.sort_by_key(|revision| revision.revision);
        Ok(revisions)
    }

    async fn review(&self, command: ReviewCommand) -> ScriptResult<ReviewDecision> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let scripts = self.scripts.write().await;
        let key = (command.script_id, command.expected_revision);
        let mut reviews = self.reviews.write().await;
        let history = reviews.entry(key).or_default();
        if let Some(existing) = history
            .iter()
            .find(|decision| decision.idempotency_key == command.idempotency_key)
        {
            if existing.request_digest == request_digest {
                return Ok(existing.clone());
            }
            return Err(crate::model::ReviewError::IdempotencyConflict.into());
        }
        let script =
            scripts
                .get(&command.script_id)
                .cloned()
                .ok_or_else(|| ScriptError::NotFound {
                    name: command.script_id.to_string(),
                })?;
        if script.version != command.expected_revision {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }
        let revision = self
            .get_source_revision(command.script_id, command.expected_revision)
            .await?;
        validate_transition(
            history.last().map(|decision| decision.status),
            command.status,
        )?;
        let decision = ReviewDecision {
            id: uuid::Uuid::new_v4(),
            script_id: command.script_id,
            tenant_id: script.tenant_id,
            revision: command.expected_revision,
            source_digest: revision.source_digest,
            status: command.status,
            policy_revision: command.policy_revision,
            actor_id: command.actor_id,
            reason: command.reason,
            idempotency_key: command.idempotency_key,
            request_digest,
            created_at: chrono::Utc::now(),
        };
        history.push(decision.clone());
        drop(scripts);
        Ok(decision)
    }

    async fn list_reviews(&self, id: ScriptId, revision: u32) -> ScriptResult<Vec<ReviewDecision>> {
        Ok(self
            .reviews
            .read()
            .await
            .get(&(id, revision))
            .cloned()
            .unwrap_or_default())
    }

    async fn claim_test_run(&self, command: TestCommand) -> ScriptResult<TestRunClaim> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let key = (
            command.script_id,
            command.expected_revision,
            command.idempotency_key,
        );
        let now = chrono::Utc::now();
        let mut runs = self.test_runs.write().await;
        if let Some(existing) = runs.get(&key).cloned() {
            if existing.request_digest != request_digest {
                return Err(crate::model::TestRunError::IdempotencyConflict.into());
            }
            if existing.status.is_terminal() {
                return Ok(TestRunClaim::Replay(existing));
            }
            let mut leases = self.test_leases.write().await;
            if let Some((_, expires_at)) = leases.get(&existing.id) {
                if *expires_at > now {
                    return Ok(TestRunClaim::InProgress(existing));
                }
            }
            let source = self
                .source_revisions
                .read()
                .await
                .get(&(command.script_id, command.expected_revision))
                .cloned()
                .ok_or_else(|| ScriptError::NotFound {
                    name: format!("{}@{}", command.script_id, command.expected_revision),
                })?;
            if source.source_digest != existing.source_digest {
                return Err(ScriptError::Storage(
                    "test run source digest does not match its immutable revision".into(),
                ));
            }
            source.workspace.validate_rhai_test(&command.test_path)?;
            let lease_token = uuid::Uuid::new_v4();
            leases.insert(
                existing.id,
                (lease_token, crate::model::test_run_lease_expires_at(now)),
            );
            return Ok(TestRunClaim::Claimed(TestRunLease {
                run: existing,
                lease_token,
                source,
            }));
        }

        let script = self
            .scripts
            .read()
            .await
            .get(&command.script_id)
            .cloned()
            .ok_or_else(|| ScriptError::NotFound {
                name: command.script_id.to_string(),
            })?;
        if script.version != command.expected_revision {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }
        let source = self
            .source_revisions
            .read()
            .await
            .get(&(command.script_id, command.expected_revision))
            .cloned()
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{}@{}", command.script_id, command.expected_revision),
            })?;
        source.workspace.validate_rhai_test(&command.test_path)?;
        let run = TestRun {
            id: uuid::Uuid::new_v4(),
            script_id: command.script_id,
            tenant_id: script.tenant_id,
            revision: command.expected_revision,
            source_digest: source.source_digest.clone(),
            test_path: command.test_path,
            actor_id: command.actor_id,
            idempotency_key: command.idempotency_key,
            request_digest,
            status: TestRunStatus::Pending,
            passed: None,
            error: None,
            created_at: now,
            completed_at: None,
        };
        let lease_token = uuid::Uuid::new_v4();
        self.test_leases.write().await.insert(
            run.id,
            (lease_token, crate::model::test_run_lease_expires_at(now)),
        );
        runs.insert(key, run.clone());
        Ok(TestRunClaim::Claimed(TestRunLease {
            run,
            lease_token,
            source,
        }))
    }

    async fn complete_test_run(
        &self,
        run_id: uuid::Uuid,
        lease_token: uuid::Uuid,
        completion: TestRunCompletion,
    ) -> ScriptResult<TestRun> {
        completion.validate()?;
        let now = chrono::Utc::now();
        let mut runs = self.test_runs.write().await;
        let key = runs
            .iter()
            .find_map(|(key, run)| (run.id == run_id).then_some(*key))
            .ok_or_else(|| ScriptError::NotFound {
                name: run_id.to_string(),
            })?;
        let run = runs.get_mut(&key).expect("test run key was found");
        if run.status.is_terminal() {
            return Ok(run.clone());
        }
        let mut leases = self.test_leases.write().await;
        let Some((stored_token, expires_at)) = leases.get(&run_id) else {
            return Err(crate::model::TestRunError::LeaseLost.into());
        };
        if *stored_token != lease_token || *expires_at <= now {
            return Err(crate::model::TestRunError::LeaseLost.into());
        }
        run.status = if completion.passed {
            TestRunStatus::Passed
        } else {
            TestRunStatus::Failed
        };
        run.passed = Some(completion.passed);
        run.error = completion.error;
        run.completed_at = Some(now);
        leases.remove(&run_id);
        Ok(run.clone())
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
        script.workspace.validate().map_err(ScriptError::from)?;
        let mut guard = self.scripts.write().await;
        if let Some(existing) = guard.get(&script.id) {
            if script.version != existing.version {
                return Err(ScriptError::RevisionConflict {
                    expected: script.version,
                });
            }
            script.version = existing
                .version
                .checked_add(1)
                .ok_or_else(|| ScriptError::Storage("script version overflow".into()))?;
            script.updated_at = chrono::Utc::now();
        } else {
            script.version = 1;
            script.created_at = chrono::Utc::now();
            script.updated_at = script.created_at;
        }

        guard.insert(script.id, script.clone());
        drop(guard);
        let revision = Self::source_revision(&script);
        self.source_revisions
            .write()
            .await
            .insert((revision.script_id, revision.revision), revision);
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
        let mut script = self.get(id).await?;
        script.status = status;
        self.save(script).await?;
        Ok(())
    }

    async fn record_error(&self, id: ScriptId) -> ScriptResult<bool> {
        let mut script = self.get(id).await?;
        let should_disable = script.register_error();
        if should_disable {
            script.status = ScriptStatus::Disabled;
        }
        self.save(script).await?;

        Ok(should_disable)
    }

    async fn reset_errors(&self, id: ScriptId) -> ScriptResult<()> {
        let mut script = self.get(id).await?;
        script.reset_errors();
        self.save(script).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AlloyWorkspace, TestCommand, TestRunClaim, TestRunCompletion, WorkspaceFile,
        WorkspaceFileKind,
    };
    use uuid::Uuid;

    fn named_script(name: &str, status: ScriptStatus) -> Script {
        let mut script = Script::new(
            name,
            AlloyWorkspace::single_source("40 + 2"),
            ScriptTrigger::Manual,
        );
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

    #[tokio::test]
    async fn save_rejects_a_stale_script_revision() {
        let storage = InMemoryStorage::new();
        let saved = storage
            .save(named_script("revisioned", ScriptStatus::Draft))
            .await
            .expect("initial script should save");
        let stale = saved.clone();

        let mut current = saved;
        current.workspace = AlloyWorkspace::single_source("43");
        let updated = storage
            .save(current)
            .await
            .expect("current revision should save");

        assert_eq!(updated.version, 2);
        assert!(matches!(
            storage.save(stale).await,
            Err(ScriptError::RevisionConflict { expected: 1 })
        ));
        assert_eq!(
            storage
                .get(updated.id)
                .await
                .expect("updated script should remain available")
                .workspace
                .entrypoint_source()
                .expect("workspace source"),
            "43"
        );
    }

    #[tokio::test]
    async fn source_revision_history_preserves_immutable_source_snapshots() {
        let storage = InMemoryStorage::new();
        let saved = storage
            .save(named_script("revisioned", ScriptStatus::Draft))
            .await
            .expect("initial script should save");
        let mut updated = saved.clone();
        updated.workspace = AlloyWorkspace::single_source("41 + 2");
        updated.author_id = Some("author:next".into());
        storage
            .save(updated)
            .await
            .expect("updated script should save");

        let revisions = storage
            .list_source_revisions(saved.id)
            .await
            .expect("source revision history should load");

        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].revision, 1);
        assert_eq!(revisions[0].parent_revision, None);
        assert_eq!(
            revisions[0]
                .workspace
                .entrypoint_source()
                .expect("workspace source"),
            "40 + 2"
        );
        assert_eq!(revisions[1].revision, 2);
        assert_eq!(revisions[1].parent_revision, Some(1));
        assert_eq!(
            revisions[1]
                .workspace
                .entrypoint_source()
                .expect("workspace source"),
            "41 + 2"
        );
        assert_eq!(revisions[1].author_id.as_deref(), Some("author:next"));
    }

    #[tokio::test]
    async fn test_run_claim_replays_only_the_same_revision_pinned_command() {
        let storage = InMemoryStorage::new();
        let mut script = named_script("tested", ScriptStatus::Draft);
        script.workspace.files.push(WorkspaceFile {
            path: "tests/smoke.rhai".into(),
            kind: WorkspaceFileKind::Test,
            contents: "true".into(),
        });
        let saved = storage.save(script).await.expect("script should save");
        let command = TestCommand {
            script_id: saved.id,
            expected_revision: saved.version,
            test_path: "tests/smoke.rhai".into(),
            actor_id: "operator:1".into(),
            idempotency_key: Uuid::new_v4(),
        };
        let TestRunClaim::Claimed(lease) = storage
            .claim_test_run(command.clone())
            .await
            .expect("test claim should reserve the immutable revision")
        else {
            panic!("new test command must be claimed");
        };
        assert_eq!(lease.source.revision, saved.version);
        assert_eq!(lease.run.source_digest, lease.source.source_digest);
        let completed = storage
            .complete_test_run(lease.run.id, lease.lease_token, TestRunCompletion::passed())
            .await
            .expect("claimed test should complete");
        assert!(completed.status.is_terminal());
        assert_eq!(completed.passed, Some(true));
        assert!(matches!(
            storage
                .claim_test_run(command.clone())
                .await
                .expect("identical command should replay"),
            TestRunClaim::Replay(run) if run.id == completed.id
        ));
        let mut conflicting = command;
        conflicting.actor_id = "operator:2".into();
        assert!(matches!(
            storage.claim_test_run(conflicting).await,
            Err(ScriptError::TestRun(
                crate::TestRunError::IdempotencyConflict
            ))
        ));

        let mut next = saved;
        next.workspace = AlloyWorkspace::single_source("43");
        storage.save(next).await.expect("next revision should save");
        assert!(matches!(
            storage
                .claim_test_run(TestCommand {
                    script_id: completed.script_id,
                    expected_revision: completed.revision,
                    test_path: "tests/smoke.rhai".into(),
                    actor_id: "operator:1".into(),
                    idempotency_key: Uuid::new_v4(),
                })
                .await,
            Err(ScriptError::RevisionConflict { .. })
        ));
    }
}
