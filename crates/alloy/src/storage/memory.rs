use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::traits::{ScriptPage, ScriptQuery, ScriptRegistry};
use crate::error::{ScriptError, ScriptResult};
use crate::model::{
    AlloyImportedDraftCommand, AlloyImportedDraftResult, ReviewCommand, ReviewDecision, Script,
    ScriptDeletionCommand, ScriptDeletionError, ScriptEvidenceRetentionCommand,
    ScriptEvidenceRetentionError, ScriptEvidenceRetentionState, ScriptId, ScriptSourceRevision,
    ScriptStatus, ScriptTrigger, TestCommand, TestRun, TestRunClaim, TestRunCompletion,
    TestRunLease, TestRunStatus, validate_transition,
};

#[derive(Clone)]
struct ReleaseImportReceipt {
    request_digest: String,
    script_id: ScriptId,
    parent_release: rustok_modules::ArtifactReleaseRef,
}

#[derive(Clone)]
struct DeletionReceipt {
    tenant_id: uuid::Uuid,
    idempotency_key: uuid::Uuid,
    request_digest: String,
    retention_policy: rustok_core::RetentionPolicy,
    retain_until: Option<chrono::DateTime<chrono::Utc>>,
    retention_revision: u32,
}

#[derive(Clone)]
struct RetentionReceipt {
    request_digest: String,
    state: ScriptEvidenceRetentionState,
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct PurgeReceipt {
    script_id: ScriptId,
    tenant_id: uuid::Uuid,
    retention_policy: rustok_core::RetentionPolicy,
    retain_until: chrono::DateTime<chrono::Utc>,
    source_revision_count: u32,
    review_count: u32,
    test_run_count: u32,
    deletion_request_digest: String,
}

#[derive(Clone)]
pub struct InMemoryStorage {
    scripts: Arc<RwLock<HashMap<ScriptId, Script>>>,
    retired_script_ids: Arc<RwLock<HashSet<ScriptId>>>,
    deletion_receipts: Arc<RwLock<HashMap<ScriptId, DeletionReceipt>>>,
    retention_receipts: Arc<RwLock<HashMap<(ScriptId, String, uuid::Uuid), RetentionReceipt>>>,
    #[cfg(test)]
    purge_receipts: Arc<RwLock<Vec<PurgeReceipt>>>,
    source_revisions: Arc<RwLock<HashMap<(ScriptId, u32), ScriptSourceRevision>>>,
    release_imports: Arc<RwLock<HashMap<(uuid::Uuid, uuid::Uuid), ReleaseImportReceipt>>>,
    reviews: Arc<RwLock<HashMap<(ScriptId, u32), Vec<ReviewDecision>>>>,
    test_runs: Arc<RwLock<HashMap<(ScriptId, u32, uuid::Uuid), TestRun>>>,
    test_leases: Arc<RwLock<HashMap<uuid::Uuid, (uuid::Uuid, chrono::DateTime<chrono::Utc>)>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            scripts: Arc::new(RwLock::new(HashMap::new())),
            retired_script_ids: Arc::new(RwLock::new(HashSet::new())),
            deletion_receipts: Arc::new(RwLock::new(HashMap::new())),
            retention_receipts: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(test)]
            purge_receipts: Arc::new(RwLock::new(Vec::new())),
            source_revisions: Arc::new(RwLock::new(HashMap::new())),
            release_imports: Arc::new(RwLock::new(HashMap::new())),
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
            source_provenance: script.source_provenance.clone(),
            parent_release: script.parent_release.clone(),
            created_at: script.updated_at,
        }
    }

    fn retention_state(
        script_id: ScriptId,
        receipt: &DeletionReceipt,
    ) -> ScriptResult<ScriptEvidenceRetentionState> {
        ScriptEvidenceRetentionState::new(
            script_id,
            receipt.tenant_id,
            receipt.request_digest.clone(),
            receipt.retention_policy,
            receipt.retain_until,
            receipt.retention_revision,
        )
        .map_err(ScriptError::from)
    }

    #[cfg(test)]
    pub(crate) async fn set_deleted_evidence_deadline(
        &self,
        script_id: ScriptId,
        retain_until: chrono::DateTime<chrono::Utc>,
    ) -> ScriptResult<()> {
        let mut receipts = self.deletion_receipts.write().await;
        let receipt = receipts
            .get_mut(&script_id)
            .ok_or_else(|| ScriptError::NotFound {
                name: script_id.to_string(),
            })?;
        receipt.retain_until = Some(retain_until);
        Ok(())
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
        // Keep the owning draft read lock while reading immutable evidence so
        // deletion cannot make an orphaned snapshot observable mid-request.
        let owner = self.scripts.read().await;
        if !owner.contains_key(&id) {
            return Err(ScriptError::NotFound {
                name: id.to_string(),
            });
        }
        let guard = self.source_revisions.read().await;
        let source = guard
            .get(&(id, revision))
            .cloned()
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{id}@{revision}"),
            })?;
        drop(guard);
        drop(owner);
        Ok(source)
    }

    async fn list_source_revisions(&self, id: ScriptId) -> ScriptResult<Vec<ScriptSourceRevision>> {
        let owner = self.scripts.read().await;
        if !owner.contains_key(&id) {
            return Err(ScriptError::NotFound {
                name: id.to_string(),
            });
        }
        let guard = self.source_revisions.read().await;
        let mut revisions = guard
            .values()
            .filter(|revision| revision.script_id == id)
            .cloned()
            .collect::<Vec<_>>();
        revisions.sort_by_key(|revision| revision.revision);
        drop(guard);
        drop(owner);
        Ok(revisions)
    }

    async fn review(&self, command: ReviewCommand) -> ScriptResult<ReviewDecision> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let scripts = self.scripts.write().await;
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
        // `scripts` is intentionally held with a write lock through the
        // revision check above so a concurrent save cannot change the review
        // subject. Do not call `get_source_revision` here: that public owner
        // method re-checks the script with a read lock and would self-deadlock.
        let revision = self
            .source_revisions
            .read()
            .await
            .get(&(command.script_id, command.expected_revision))
            .cloned()
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{}@{}", command.script_id, command.expected_revision),
            })?;
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
        let owner = self.scripts.read().await;
        if !owner.contains_key(&id) {
            return Err(ScriptError::NotFound {
                name: id.to_string(),
            });
        }
        let reviews = self
            .reviews
            .read()
            .await
            .get(&(id, revision))
            .cloned()
            .unwrap_or_default();
        drop(owner);
        Ok(reviews)
    }

    async fn claim_test_run(&self, command: TestCommand) -> ScriptResult<TestRunClaim> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let scripts = self.scripts.read().await;
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
            if leases
                .get(&existing.id)
                .is_some_and(|(_, expires_at)| *expires_at > now)
            {
                return Ok(TestRunClaim::InProgress(existing));
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
            let terminal = run.clone();
            drop(runs);
            // The durable row remains available to retention and GC, but a
            // terminal idempotency completion cannot read it after deletion.
            self.get(terminal.script_id).await?;
            return Ok(terminal);
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
        let completed = run.clone();
        drop(leases);
        drop(runs);
        // Keep the settled row for retention, but deny a completion response
        // if the owner draft was deleted while the sandbox was executing.
        self.get(completed.script_id).await?;
        Ok(completed)
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

    async fn import_published_release(
        &self,
        mut command: AlloyImportedDraftCommand,
    ) -> ScriptResult<AlloyImportedDraftResult> {
        command
            .validate()
            .map_err(|error| ScriptError::InvalidLineage(error.to_string()))?;
        let parent_release = command
            .script
            .parent_release
            .clone()
            .expect("validated imported draft must have a parent release");
        let key = (command.script.tenant_id, command.idempotency_key);
        let mut receipts = self.release_imports.write().await;
        let mut scripts = self.scripts.write().await;
        let mut revisions = self.source_revisions.write().await;

        if let Some(receipt) = receipts.get(&key) {
            if receipt.request_digest != command.request_digest
                || receipt.parent_release != parent_release
            {
                return Err(ScriptError::ImportIdempotencyConflict);
            }
            let script = scripts.get(&receipt.script_id).cloned().ok_or_else(|| {
                ScriptError::Storage(
                    "Alloy release import receipt references a missing draft".to_string(),
                )
            })?;
            return Ok(AlloyImportedDraftResult {
                script,
                created: false,
            });
        }

        if scripts.values().any(|script| {
            script.tenant_id == command.script.tenant_id && script.name == command.script.name
        }) {
            return Err(ScriptError::ImportDraftNameConflict);
        }
        if self
            .retired_script_ids
            .read()
            .await
            .contains(&command.script.id)
        {
            return Err(ScriptError::InvalidLineage(
                "a deleted draft ID cannot be reused while immutable evidence is retained".into(),
            ));
        }

        let now = chrono::Utc::now();
        command.script.version = 1;
        command.script.created_at = now;
        command.script.updated_at = now;
        let script = command.script;
        let revision = Self::source_revision(&script);
        scripts.insert(script.id, script.clone());
        revisions.insert((revision.script_id, revision.revision), revision);
        receipts.insert(
            key,
            ReleaseImportReceipt {
                request_digest: command.request_digest,
                script_id: script.id,
                parent_release,
            },
        );
        Ok(AlloyImportedDraftResult {
            script,
            created: true,
        })
    }

    async fn save(&self, mut script: Script) -> ScriptResult<Script> {
        script.workspace.validate().map_err(ScriptError::from)?;
        script
            .source_provenance
            .validate()
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if let Some(parent_release) = &script.parent_release {
            parent_release
                .validate()
                .map_err(|error| ScriptError::InvalidLineage(error.to_string()))?;
        }
        let mut guard = self.scripts.write().await;
        if let Some(existing) = guard.get(&script.id) {
            if existing.parent_release != script.parent_release {
                return Err(ScriptError::InvalidLineage(
                    "a draft cannot replace or remove its imported parent release".to_string(),
                ));
            }
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
            if self.retired_script_ids.read().await.contains(&script.id) {
                return Err(ScriptError::InvalidLineage(
                    "a deleted draft ID cannot be reused while immutable evidence is retained"
                        .into(),
                ));
            }
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

    async fn delete(&self, command: ScriptDeletionCommand) -> ScriptResult<()> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let id = command.script_id;
        let mut guard = self.scripts.write().await;
        let mut retired = self.retired_script_ids.write().await;
        let mut receipts = self.deletion_receipts.write().await;
        if let Some(existing) = receipts.get(&id) {
            if existing.request_digest == request_digest {
                return Ok(());
            }
            return if existing.idempotency_key == command.idempotency_key {
                Err(ScriptDeletionError::IdempotencyConflict.into())
            } else {
                Err(ScriptError::NotFound {
                    name: id.to_string(),
                })
            };
        }
        let script = guard.get(&id).ok_or(ScriptError::NotFound {
            name: id.to_string(),
        })?;
        if script.version != command.expected_revision {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }
        let deleted_at = chrono::Utc::now();
        let (retention_policy, retain_until) = crate::model::deleted_evidence_retention(deleted_at);
        let tenant_id = script.tenant_id;
        guard.remove(&id);
        retired.insert(id);
        receipts.insert(
            id,
            DeletionReceipt {
                tenant_id,
                idempotency_key: command.idempotency_key,
                request_digest,
                retention_policy,
                retain_until: Some(retain_until),
                retention_revision: 1,
            },
        );
        Ok(())
    }

    async fn get_deleted_evidence_retention(
        &self,
        id: ScriptId,
    ) -> ScriptResult<ScriptEvidenceRetentionState> {
        let receipts = self.deletion_receipts.read().await;
        let receipt = receipts.get(&id).ok_or_else(|| ScriptError::NotFound {
            name: id.to_string(),
        })?;
        Self::retention_state(id, receipt)
    }

    async fn update_deleted_evidence_retention(
        &self,
        command: ScriptEvidenceRetentionCommand,
    ) -> ScriptResult<ScriptEvidenceRetentionState> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let mut receipts = self.deletion_receipts.write().await;
        let mut retention_receipts = self.retention_receipts.write().await;
        let receipt_key = (
            command.script_id,
            command.deletion_request_digest.clone(),
            command.idempotency_key,
        );
        let deletion = match receipts.get_mut(&command.script_id) {
            Some(deletion) => deletion,
            None => {
                return retention_receipts
                    .get(&receipt_key)
                    .filter(|receipt| receipt.request_digest == request_digest)
                    .map(|receipt| receipt.state.clone())
                    .ok_or_else(|| ScriptError::NotFound {
                        name: command.script_id.to_string(),
                    });
            }
        };
        let current = Self::retention_state(command.script_id, deletion)?;
        if current.deletion_request_digest != command.deletion_request_digest {
            return Err(ScriptError::NotFound {
                name: command.script_id.to_string(),
            });
        }
        if let Some(receipt) = retention_receipts.get(&receipt_key) {
            return if receipt.request_digest == request_digest {
                Ok(receipt.state.clone())
            } else {
                Err(ScriptEvidenceRetentionError::IdempotencyConflict.into())
            };
        }
        if current.retention_revision != command.expected_retention_revision {
            return Err(ScriptError::RetentionRevisionConflict {
                expected: command.expected_retention_revision,
            });
        }
        let state = current.transition(command.action, chrono::Utc::now())?;
        deletion.retention_policy = state.policy;
        deletion.retain_until = state.retain_until;
        deletion.retention_revision = state.retention_revision;
        retention_receipts.insert(
            receipt_key,
            RetentionReceipt {
                request_digest,
                state: state.clone(),
            },
        );
        Ok(state)
    }

    async fn purge_expired_evidence(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        limit: u16,
    ) -> ScriptResult<u64> {
        if limit == 0 {
            return Ok(0);
        }

        // Acquire locks in the same owner-first order as delete/save so a
        // retention sweep cannot expose or race a partially deleted draft.
        let _scripts = self.scripts.write().await;
        let mut retired = self.retired_script_ids.write().await;
        let mut deletions = self.deletion_receipts.write().await;
        let candidate_ids = deletions
            .iter()
            .filter(|(_, receipt)| {
                receipt.retention_policy == rustok_core::RetentionPolicy::RetainUntil
                    && receipt
                        .retain_until
                        .is_some_and(|retain_until| retain_until <= now)
            })
            .map(|(script_id, _)| *script_id)
            .take(usize::from(limit))
            .collect::<Vec<_>>();
        if candidate_ids.is_empty() {
            return Ok(0);
        }
        let candidates = candidate_ids
            .iter()
            .filter_map(|script_id| {
                deletions
                    .get(script_id)
                    .cloned()
                    .map(|receipt| (*script_id, receipt))
            })
            .collect::<Vec<_>>();

        let candidate_set = candidate_ids.into_iter().collect::<HashSet<_>>();
        let mut source_revisions = self.source_revisions.write().await;
        let mut reviews = self.reviews.write().await;
        let mut test_runs = self.test_runs.write().await;
        #[cfg(test)]
        let mut test_leases = self.test_leases.write().await;
        #[cfg(test)]
        let mut purge_receipts = self.purge_receipts.write().await;

        for (script_id, deletion) in candidates {
            let retain_until = deletion.retain_until.ok_or_else(|| {
                ScriptError::Storage("retain_until evidence has no deadline".into())
            })?;
            #[cfg(not(test))]
            let _ = retain_until;
            #[cfg(test)]
            let source_revision_count: u32 = source_revisions
                .keys()
                .filter(|(id, _)| *id == script_id)
                .count()
                .try_into()
                .map_err(|_| ScriptError::Storage("source revision count exceeds u32".into()))?;
            source_revisions.retain(|(id, _), _| *id != script_id);

            #[cfg(test)]
            let review_count: u32 = reviews
                .iter()
                .filter(|((id, _), _)| *id == script_id)
                .map(|(_, decisions)| decisions.len())
                .sum::<usize>()
                .try_into()
                .map_err(|_| ScriptError::Storage("review count exceeds u32".into()))?;
            reviews.retain(|(id, _), _| *id != script_id);

            #[cfg(test)]
            let purged_run_ids = test_runs
                .iter()
                .filter(|((id, _, _), _)| *id == script_id)
                .map(|(_, run)| run.id)
                .collect::<HashSet<_>>();
            #[cfg(test)]
            let test_run_count: u32 = purged_run_ids
                .len()
                .try_into()
                .map_err(|_| ScriptError::Storage("test run count exceeds u32".into()))?;
            test_runs.retain(|(id, _, _), _| *id != script_id);
            #[cfg(test)]
            test_leases.retain(|run_id, _| !purged_run_ids.contains(run_id));

            #[cfg(test)]
            purge_receipts.push(PurgeReceipt {
                script_id,
                tenant_id: deletion.tenant_id,
                retention_policy: deletion.retention_policy,
                retain_until,
                source_revision_count,
                review_count,
                test_run_count,
                deletion_request_digest: deletion.request_digest,
            });
            deletions.remove(&script_id);
            retired.remove(&script_id);
        }
        debug_assert!(candidate_set.iter().all(|id| !retired.contains(id)));
        Ok(u64::try_from(candidate_set.len())
            .map_err(|_| ScriptError::Storage("purge count exceeds u64".into()))?)
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
        ReviewCommand, ReviewStatus, RhaiWorkspace, RhaiWorkspaceFile, RhaiWorkspaceFileKind,
        TestCommand, TestRunClaim, TestRunCompletion,
    };
    use uuid::Uuid;

    fn named_script(name: &str, status: ScriptStatus) -> Script {
        let mut script = Script::new(
            name,
            RhaiWorkspace::single_source("40 + 2"),
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
        current.workspace = RhaiWorkspace::single_source("43");
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
    async fn delete_rejects_a_stale_script_revision() {
        let storage = InMemoryStorage::new();
        let saved = storage
            .save(named_script("deletable", ScriptStatus::Draft))
            .await
            .expect("initial script should save");
        let mut current = saved.clone();
        current.workspace = RhaiWorkspace::single_source("43");
        let updated = storage
            .save(current)
            .await
            .expect("current revision should save");

        assert!(matches!(
            storage
                .delete(ScriptDeletionCommand {
                    script_id: saved.id,
                    expected_revision: saved.version,
                    actor_id: "operator:delete".into(),
                    reason: "The superseded draft is no longer needed.".into(),
                    idempotency_key: Uuid::new_v4(),
                })
                .await,
            Err(ScriptError::RevisionConflict { expected: 1 })
        ));
        let review = ReviewCommand {
            script_id: updated.id,
            expected_revision: updated.version,
            status: ReviewStatus::ChangesRequested,
            policy_revision: "policy:current".into(),
            actor_id: "operator:reviewer".into(),
            reason: None,
            idempotency_key: Uuid::new_v4(),
        };
        storage
            .review(review.clone())
            .await
            .expect("review should be recorded before deletion");
        let deletion = ScriptDeletionCommand {
            script_id: updated.id,
            expected_revision: updated.version,
            actor_id: "operator:delete".into(),
            reason: "The draft was intentionally removed.".into(),
            idempotency_key: Uuid::new_v4(),
        };
        storage
            .delete(deletion.clone())
            .await
            .expect("current revision should delete");
        storage
            .delete(deletion.clone())
            .await
            .expect("the exact deletion command should replay after removal");
        let mut conflicting_replay = deletion;
        conflicting_replay.reason = "A different retention reason.".into();
        assert!(matches!(
            storage.delete(conflicting_replay).await,
            Err(ScriptError::Deletion(
                ScriptDeletionError::IdempotencyConflict
            ))
        ));
        assert!(matches!(
            storage.get(updated.id).await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(matches!(
            storage
                .get_source_revision(updated.id, updated.version)
                .await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(matches!(
            storage.list_source_revisions(updated.id).await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(matches!(
            storage.list_reviews(updated.id, updated.version).await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(matches!(
            storage.review(review).await,
            Err(ScriptError::NotFound { .. })
        ));
        let mut replacement = named_script("retired_id", ScriptStatus::Draft);
        replacement.id = updated.id;
        assert!(matches!(
            storage.save(replacement).await,
            Err(ScriptError::InvalidLineage(_))
        ));
    }

    #[tokio::test]
    async fn expired_deleted_evidence_is_collected_with_a_content_free_receipt() {
        let storage = InMemoryStorage::new();
        let review_reason = "Review reason that must be erased after expiry.";
        let test_diagnostic = "Test diagnostic that must be erased after expiry.";
        let mut script = named_script("retention_subject", ScriptStatus::Draft);
        script.workspace.files.push(RhaiWorkspaceFile {
            path: "tests/smoke.rhai".into(),
            kind: RhaiWorkspaceFileKind::Test,
            contents: "true".into(),
        });
        let script = storage
            .save(script)
            .await
            .expect("retention subject should save");
        storage
            .review(ReviewCommand {
                script_id: script.id,
                expected_revision: script.version,
                status: ReviewStatus::ChangesRequested,
                policy_revision: "policy:retention".into(),
                actor_id: "operator:reviewer".into(),
                reason: Some(review_reason.into()),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("review should persist");
        let TestRunClaim::Claimed(lease) = storage
            .claim_test_run(TestCommand {
                script_id: script.id,
                expected_revision: script.version,
                test_path: "tests/smoke.rhai".into(),
                actor_id: "operator:tester".into(),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("test evidence should reserve")
        else {
            panic!("new test command must claim a lease");
        };
        storage
            .complete_test_run(
                lease.run.id,
                lease.lease_token,
                TestRunCompletion::failed(Some(test_diagnostic.into()))
                    .expect("diagnostic should be valid test evidence"),
            )
            .await
            .expect("terminal diagnostic should persist until expiry");
        storage
            .delete(ScriptDeletionCommand {
                script_id: script.id,
                expected_revision: script.version,
                actor_id: "operator:delete".into(),
                reason: "Retention period elapsed for this draft.".into(),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("deletion should retain evidence first");

        storage
            .set_deleted_evidence_deadline(
                script.id,
                chrono::Utc::now() - chrono::Duration::seconds(1),
            )
            .await
            .expect("deletion receipt should exist");

        assert_eq!(
            storage
                .purge_expired_evidence(chrono::Utc::now(), 1)
                .await
                .expect("expired retention should purge"),
            1
        );
        assert!(
            !storage
                .source_revisions
                .read()
                .await
                .keys()
                .any(|(id, _)| *id == script.id)
        );
        assert!(
            !storage
                .reviews
                .read()
                .await
                .keys()
                .any(|(id, _)| *id == script.id)
        );
        assert!(
            !storage
                .test_runs
                .read()
                .await
                .keys()
                .any(|(id, _, _)| *id == script.id)
        );
        let receipt = storage
            .purge_receipts
            .read()
            .await
            .last()
            .cloned()
            .expect("content-free purge receipt should persist");
        assert_eq!(receipt.script_id, script.id);
        assert_eq!(receipt.tenant_id, script.tenant_id);
        assert_eq!(
            receipt.retention_policy,
            rustok_core::RetentionPolicy::RetainUntil
        );
        assert!(receipt.retain_until <= chrono::Utc::now());
        assert_eq!(receipt.source_revision_count, 1);
        assert_eq!(receipt.review_count, 1);
        assert_eq!(receipt.test_run_count, 1);
        assert!(receipt.deletion_request_digest.starts_with("sha256:"));
        let receipt_debug = format!("{receipt:?}");
        assert!(!receipt_debug.contains(review_reason));
        assert!(!receipt_debug.contains(test_diagnostic));

        let mut replacement = named_script("reused_after_purge", ScriptStatus::Draft);
        replacement.id = script.id;
        storage
            .save(replacement)
            .await
            .expect("purged draft ID may be reused without prior evidence");
    }

    #[tokio::test]
    async fn legal_hold_requires_a_retention_revision_and_blocks_collection_until_release() {
        let storage = InMemoryStorage::new();
        let script = storage
            .save(named_script("legal_hold_subject", ScriptStatus::Draft))
            .await
            .expect("script should save");
        storage
            .delete(ScriptDeletionCommand {
                script_id: script.id,
                expected_revision: script.version,
                actor_id: "operator:retention".into(),
                reason: "The draft must be retained before a legal review.".into(),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("script deletion should create an initial retention state");
        let initial = storage
            .get_deleted_evidence_retention(script.id)
            .await
            .expect("initial retention state should be visible to the owner");
        let hold = ScriptEvidenceRetentionCommand {
            script_id: script.id,
            deletion_request_digest: initial.deletion_request_digest.clone(),
            expected_retention_revision: initial.retention_revision,
            action: crate::ScriptEvidenceRetentionAction::ApplyLegalHold,
            actor_id: "operator:retention".into(),
            reason: "A legal investigation requires preservation.".into(),
            idempotency_key: Uuid::new_v4(),
        };
        let held = storage
            .update_deleted_evidence_retention(hold.clone())
            .await
            .expect("owner should be able to apply a legal hold");
        assert_eq!(held.policy, rustok_core::RetentionPolicy::LegalHold);
        assert_eq!(held.retain_until, None);
        assert_eq!(held.retention_revision, 2);
        assert_eq!(
            storage
                .purge_expired_evidence(chrono::Utc::now() + chrono::Duration::days(90), 8)
                .await
                .expect("legal-hold sweep should complete"),
            0
        );
        assert_eq!(
            storage
                .update_deleted_evidence_retention(hold.clone())
                .await
                .expect("exact legal-hold retry should replay"),
            held
        );
        let mut conflicting_hold = hold;
        conflicting_hold.reason = "A different reason must not replay.".into();
        assert!(matches!(
            storage
                .update_deleted_evidence_retention(conflicting_hold)
                .await,
            Err(ScriptError::EvidenceRetention(
                ScriptEvidenceRetentionError::IdempotencyConflict
            ))
        ));

        let released = storage
            .update_deleted_evidence_retention(ScriptEvidenceRetentionCommand {
                script_id: script.id,
                deletion_request_digest: held.deletion_request_digest.clone(),
                expected_retention_revision: held.retention_revision,
                action: crate::ScriptEvidenceRetentionAction::ReleaseLegalHold,
                actor_id: "operator:retention".into(),
                reason: "The legal hold was released by its owner.".into(),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("owner should be able to release a legal hold");
        assert_eq!(released.policy, rustok_core::RetentionPolicy::RetainUntil);
        assert!(released.retain_until > Some(chrono::Utc::now()));
        assert_eq!(released.retention_revision, 3);
    }

    #[tokio::test]
    async fn source_revision_history_preserves_immutable_source_snapshots() {
        let storage = InMemoryStorage::new();
        let saved = storage
            .save(named_script("revisioned", ScriptStatus::Draft))
            .await
            .expect("initial script should save");
        let mut updated = saved.clone();
        updated.workspace = RhaiWorkspace::single_source("41 + 2");
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
        script.workspace.files.push(RhaiWorkspaceFile {
            path: "tests/smoke.rhai".into(),
            kind: RhaiWorkspaceFileKind::Test,
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
        let mut conflicting = command.clone();
        conflicting.actor_id = "operator:2".into();
        assert!(matches!(
            storage.claim_test_run(conflicting).await,
            Err(ScriptError::TestRun(
                crate::TestRunError::IdempotencyConflict
            ))
        ));

        let mut next = saved;
        next.workspace = RhaiWorkspace::single_source("43");
        let next = storage.save(next).await.expect("next revision should save");
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
        storage
            .delete(ScriptDeletionCommand {
                script_id: next.id,
                expected_revision: next.version,
                actor_id: "operator:delete".into(),
                reason: "The draft was intentionally removed.".into(),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("current script should delete");
        assert!(matches!(
            storage
                .complete_test_run(lease.run.id, lease.lease_token, TestRunCompletion::passed())
                .await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(matches!(
            storage.claim_test_run(command).await,
            Err(ScriptError::NotFound { .. })
        ));
    }
}
