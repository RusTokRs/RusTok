//! Framework-neutral, tenant-scoped Alloy authoring operations.
//!
//! HTTP, GraphQL, and remote MCP compose the same [`AlloyAuthoringService`]
//! from an owner-scoped runtime. Remote adapters must never serialize a
//! workspace back to an untrusted caller: every returned script shape in this
//! module is deliberately source-redacted.

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::utils::{json_to_dynamic, validate_cron_expression};
use crate::{
    AlloyDraftRuntime, EntityProxy, ExecutionOutcome, ReviewCommand, ReviewDecision, ReviewStatus,
    RevisionedTestRunner, RhaiWorkspace, ScopedAlloyRuntime, Script, ScriptDeletionCommand,
    ScriptEngine, ScriptError, ScriptEvidenceRetentionAction, ScriptEvidenceRetentionCommand,
    ScriptEvidenceRetentionState, ScriptOrchestrator, ScriptQuery, ScriptRegistry, ScriptStatus,
    ScriptTrigger, SourceProvenance, TestCommand, TestRun, TestRunStatus,
};

/// Authoring operations are always bound to one tenant before a command is
/// parsed or executed. `R` is owner storage, not an MCP transport dependency.
pub struct AlloyAuthoringService<R: ScriptRegistry> {
    engine: Arc<ScriptEngine>,
    sandbox: AlloyDraftRuntime,
    registry: Arc<R>,
    orchestrator: Arc<ScriptOrchestrator<R>>,
    tenant_id: Uuid,
}

impl AlloyAuthoringService<crate::SeaOrmStorage> {
    /// Uses the exact owner-scoped production runtime supplied by the host.
    /// The caller cannot select a registry or tenant independently.
    pub fn from_scoped(runtime: ScopedAlloyRuntime) -> Self {
        Self {
            engine: runtime.engine,
            sandbox: runtime.sandbox,
            registry: runtime.storage,
            orchestrator: runtime.orchestrator,
            tenant_id: runtime.tenant_id,
        }
    }
}

impl<R: ScriptRegistry> AlloyAuthoringService<R> {
    /// Constructs a tenant-bound owner service for tests and non-HTTP owner
    /// adapters. Production remote adapters use [`Self::from_scoped`].
    pub fn new(
        tenant_id: Uuid,
        engine: Arc<ScriptEngine>,
        sandbox: AlloyDraftRuntime,
        registry: Arc<R>,
    ) -> Self {
        let orchestrator = Arc::new(ScriptOrchestrator::new(sandbox.clone(), registry.clone()));
        Self {
            engine,
            sandbox,
            registry,
            orchestrator,
            tenant_id,
        }
    }

    pub async fn list_scripts(
        &self,
        request: ListAlloyScriptsCommand,
    ) -> Result<RedactedAlloyScriptPage, AlloyAuthoringError> {
        let page = request.normalized_page();
        let per_page = request.normalized_per_page();
        let query = request
            .status
            .map(ScriptQuery::ByStatus)
            .unwrap_or(ScriptQuery::All);
        // Owner storage is tenant-scoped in production. Filtering again makes
        // the invariant explicit for every other ScriptRegistry implementation.
        let scripts = self
            .registry
            .find(query)
            .await
            .map_err(AlloyAuthoringError::from_script_error)?
            .into_iter()
            .filter(|script| self.owns(script))
            .collect::<Vec<_>>();
        let total = scripts.len();
        let offset = (page.saturating_sub(1) as usize).saturating_mul(per_page as usize);
        let items = scripts
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .map(RedactedAlloyScript::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(RedactedAlloyScriptPage {
            scripts: items,
            total,
            page,
            per_page,
            total_pages: total_pages(total, per_page),
        })
    }

    pub async fn get_script(
        &self,
        request: GetAlloyScriptCommand,
    ) -> Result<RedactedAlloyScript, AlloyAuthoringError> {
        RedactedAlloyScript::try_from(self.script_for_tenant(request.script_id).await?)
    }

    pub async fn list_source_revisions(
        &self,
        request: ListAlloyScriptRevisionsCommand,
    ) -> Result<Vec<RedactedAlloySourceRevision>, AlloyAuthoringError> {
        self.script_for_tenant(request.script_id).await?;
        self.registry
            .list_source_revisions(request.script_id)
            .await
            .map_err(AlloyAuthoringError::from_script_error)?
            .into_iter()
            .filter(|revision| revision.tenant_id == self.tenant_id)
            .map(RedactedAlloySourceRevision::try_from)
            .collect()
    }

    pub async fn create_script(
        &self,
        actor_id: &str,
        request: CreateAlloyScriptCommand,
    ) -> Result<RedactedAlloyScript, AlloyAuthoringError> {
        self.validate_actor(actor_id)?;
        self.validate_workspace(&request.name, &request.workspace, &request.trigger)?;
        let duplicate = self
            .registry
            .find(ScriptQuery::ByName(request.name.clone()))
            .await
            .map_err(AlloyAuthoringError::from_script_error)?
            .into_iter()
            .any(|script| self.owns(&script));
        if duplicate {
            return Err(AlloyAuthoringError::Invalid);
        }

        let mut script = Script::new(request.name, request.workspace, request.trigger);
        script.tenant_id = self.tenant_id;
        script.description = request.description;
        script.permissions = request.permissions;
        script.run_as_system = request.run_as_system;
        script.author_id = Some(actor_id.to_owned());
        script.source_provenance = SourceProvenance::remote_mcp("alloy_create_script");
        let saved = self
            .registry
            .save(script)
            .await
            .map_err(AlloyAuthoringError::from_script_error)?;
        self.require_owned(&saved)?;
        RedactedAlloyScript::try_from(saved)
    }

    pub async fn update_script(
        &self,
        actor_id: &str,
        request: UpdateAlloyScriptCommand,
    ) -> Result<RedactedAlloyScript, AlloyAuthoringError> {
        self.validate_actor(actor_id)?;
        let mut script = self.script_for_tenant(request.script_id).await?;
        self.require_expected_revision(&script, request.expected_version)?;

        if let Some(name) = request.name {
            self.engine.invalidate(&script.name);
            script.name = name;
        }
        if let Some(description) = request.description {
            script.description = Some(description);
        }
        if let Some(workspace) = request.workspace {
            self.engine.invalidate(&script.name);
            self.validate_workspace(&script.name, &workspace, &script.trigger)?;
            script.workspace = workspace;
        }
        if let Some(trigger) = request.trigger {
            self.validate_trigger(&trigger)?;
            script.trigger = trigger;
        }
        if let Some(status) = request.status {
            script.status = status;
        }
        if let Some(run_as_system) = request.run_as_system {
            script.run_as_system = run_as_system;
        }
        if let Some(permissions) = request.permissions {
            script.permissions = permissions;
        }
        script.author_id = Some(actor_id.to_owned());
        script.source_provenance = SourceProvenance::remote_mcp("alloy_update_script");
        let saved = self
            .registry
            .save(script)
            .await
            .map_err(AlloyAuthoringError::from_script_error)?;
        self.require_owned(&saved)?;
        RedactedAlloyScript::try_from(saved)
    }

    pub async fn delete_script(
        &self,
        actor_id: &str,
        request: DeleteAlloyScriptCommand,
    ) -> Result<DeletedAlloyScript, AlloyAuthoringError> {
        self.validate_actor(actor_id)?;
        let script = match self.script_for_tenant(request.script_id).await {
            Ok(script) => {
                self.require_expected_revision(&script, request.expected_version)?;
                Some(script)
            }
            Err(AlloyAuthoringError::NotFound) => None,
            Err(error) => return Err(error),
        };
        self.registry
            .delete(ScriptDeletionCommand {
                script_id: request.script_id,
                expected_revision: request.expected_version,
                actor_id: actor_id.to_owned(),
                reason: request.reason,
                idempotency_key: request.idempotency_key,
            })
            .await
            .map_err(AlloyAuthoringError::from_script_error)?;
        if let Some(script) = script {
            self.engine.invalidate(&script.name);
        }
        Ok(DeletedAlloyScript { deleted: true })
    }

    pub async fn get_deleted_evidence_retention(
        &self,
        request: GetAlloyDeletedEvidenceRetentionCommand,
    ) -> Result<RedactedAlloyEvidenceRetention, AlloyAuthoringError> {
        let state = self
            .registry
            .get_deleted_evidence_retention(request.script_id)
            .await
            .map_err(AlloyAuthoringError::from_script_error)?;
        self.require_retention_owned(&state)?;
        Ok(state.into())
    }

    pub async fn change_deleted_evidence_retention(
        &self,
        actor_id: &str,
        request: ChangeAlloyDeletedEvidenceRetentionCommand,
    ) -> Result<RedactedAlloyEvidenceRetention, AlloyAuthoringError> {
        self.validate_actor(actor_id)?;
        let current = self
            .registry
            .get_deleted_evidence_retention(request.script_id)
            .await
            .map_err(AlloyAuthoringError::from_script_error)?;
        self.require_retention_owned(&current)?;
        if current.deletion_request_digest != request.deletion_request_digest {
            return Err(AlloyAuthoringError::NotFound);
        }
        let state = self
            .registry
            .update_deleted_evidence_retention(ScriptEvidenceRetentionCommand {
                script_id: request.script_id,
                deletion_request_digest: request.deletion_request_digest,
                expected_retention_revision: request.expected_retention_revision,
                action: request.action,
                actor_id: actor_id.to_owned(),
                reason: request.reason,
                idempotency_key: request.idempotency_key,
            })
            .await
            .map_err(AlloyAuthoringError::from_script_error)?;
        self.require_retention_owned(&state)?;
        Ok(state.into())
    }

    pub fn validate_script(
        &self,
        request: ValidateAlloyScriptCommand,
    ) -> Result<AlloyScriptValidation, AlloyAuthoringError> {
        let source_digest = workspace_digest(&request.workspace)?;
        self.validate_trigger(&request.trigger)?;
        if request.workspace.validate_rhai_workspace().is_err() {
            return Ok(AlloyScriptValidation {
                valid: false,
                source_digest,
            });
        }
        let source = request
            .workspace
            .entrypoint_source()
            .map_err(|_| AlloyAuthoringError::Invalid)?;
        let mut scope = rhai::Scope::new();
        let valid = self
            .engine
            .compile(&request.name, source, &mut scope)
            .is_ok();
        Ok(AlloyScriptValidation {
            valid,
            source_digest,
        })
    }

    pub async fn run_script(
        &self,
        actor_id: &str,
        request: RunAlloyScriptCommand,
    ) -> Result<RedactedAlloyExecution, AlloyAuthoringError> {
        self.validate_actor(actor_id)?;
        let script = self.script_for_tenant(request.script_id).await?;
        self.require_expected_revision(&script, request.expected_version)?;
        let params = request
            .params
            .into_iter()
            .map(|(key, value)| (key, json_to_dynamic(value)))
            .collect::<HashMap<_, _>>();
        let entity = request.entity.map(AuthoringEntityInput::into_proxy);
        let result = self
            .orchestrator
            .run_manual_snapshot(&script, params, entity, Some(actor_id.to_owned()))
            .await;
        let (success, outcome) = match result.outcome {
            ExecutionOutcome::Success { .. } => (true, AlloyExecutionOutcome::Succeeded),
            ExecutionOutcome::Aborted { .. } => (false, AlloyExecutionOutcome::Aborted),
            ExecutionOutcome::Failed { .. } => (false, AlloyExecutionOutcome::Failed),
        };
        Ok(RedactedAlloyExecution {
            execution_id: result.execution_id,
            script_id: result.script_id,
            success,
            outcome,
            duration_ms: result.duration_ms(),
        })
    }

    pub async fn review_script(
        &self,
        actor_id: &str,
        request: ReviewAlloyScriptCommand,
    ) -> Result<RedactedAlloyReview, AlloyAuthoringError> {
        self.validate_actor(actor_id)?;
        self.script_for_tenant(request.script_id).await?;
        let decision = self
            .registry
            .review(ReviewCommand {
                script_id: request.script_id,
                expected_revision: request.expected_version,
                status: request.status,
                policy_revision: request.policy_revision,
                actor_id: actor_id.to_owned(),
                reason: request.reason,
                idempotency_key: request.idempotency_key,
            })
            .await
            .map_err(AlloyAuthoringError::from_script_error)?;
        self.require_review_owned(&decision)?;
        Ok(decision.into())
    }

    pub async fn list_reviews(
        &self,
        request: ListAlloyScriptReviewsCommand,
    ) -> Result<Vec<RedactedAlloyReview>, AlloyAuthoringError> {
        self.script_for_tenant(request.script_id).await?;
        self.registry
            .list_reviews(request.script_id, request.revision)
            .await
            .map_err(AlloyAuthoringError::from_script_error)?
            .into_iter()
            .filter(|decision| decision.tenant_id == self.tenant_id)
            .map(|decision| {
                self.require_review_owned(&decision)?;
                Ok(decision.into())
            })
            .collect()
    }

    pub async fn run_workspace_test(
        &self,
        actor_id: &str,
        request: RunAlloyWorkspaceTestCommand,
    ) -> Result<RedactedAlloyTestRun, AlloyAuthoringError> {
        self.validate_actor(actor_id)?;
        self.script_for_tenant(request.script_id).await?;
        let run = RevisionedTestRunner::new(self.sandbox.clone(), self.registry.clone())
            .execute(TestCommand {
                script_id: request.script_id,
                expected_revision: request.expected_version,
                test_path: request.test_path,
                actor_id: actor_id.to_owned(),
                idempotency_key: request.idempotency_key,
            })
            .await
            .map_err(AlloyAuthoringError::from_script_error)?;
        self.require_test_owned(&run)?;
        Ok(run.into())
    }

    pub async fn change_lifecycle(
        &self,
        actor_id: &str,
        request: ChangeAlloyScriptLifecycleCommand,
    ) -> Result<RedactedAlloyScript, AlloyAuthoringError> {
        self.validate_actor(actor_id)?;
        let mut script = self.script_for_tenant(request.script_id).await?;
        self.require_expected_revision(&script, request.expected_version)?;
        match request.action {
            AlloyScriptLifecycleAction::Activate => script.activate(),
            AlloyScriptLifecycleAction::Pause => script.status = ScriptStatus::Paused,
            AlloyScriptLifecycleAction::Disable => script.disable(),
            AlloyScriptLifecycleAction::Archive => script.archive(),
            AlloyScriptLifecycleAction::ResetErrors => script.reset_errors(),
        }
        script.author_id = Some(actor_id.to_owned());
        let saved = self
            .registry
            .save(script)
            .await
            .map_err(AlloyAuthoringError::from_script_error)?;
        self.require_owned(&saved)?;
        RedactedAlloyScript::try_from(saved)
    }

    async fn script_for_tenant(&self, script_id: Uuid) -> Result<Script, AlloyAuthoringError> {
        let script = self
            .registry
            .get(script_id)
            .await
            .map_err(AlloyAuthoringError::from_script_error)?;
        self.require_owned(&script)?;
        Ok(script)
    }

    fn validate_workspace(
        &self,
        name: &str,
        workspace: &RhaiWorkspace,
        trigger: &ScriptTrigger,
    ) -> Result<(), AlloyAuthoringError> {
        workspace
            .validate_rhai_workspace()
            .map_err(|_| AlloyAuthoringError::Invalid)?;
        self.validate_trigger(trigger)?;
        let source = workspace
            .entrypoint_source()
            .map_err(|_| AlloyAuthoringError::Invalid)?;
        let mut scope = rhai::Scope::new();
        self.engine
            .compile(name, source, &mut scope)
            .map_err(|_| AlloyAuthoringError::Invalid)
    }

    fn validate_trigger(&self, trigger: &ScriptTrigger) -> Result<(), AlloyAuthoringError> {
        if let ScriptTrigger::Cron { expression } = trigger {
            validate_cron_expression(expression).map_err(|_| AlloyAuthoringError::Invalid)?;
        }
        Ok(())
    }

    fn validate_actor(&self, actor_id: &str) -> Result<(), AlloyAuthoringError> {
        (actor_id.trim() == actor_id
            && !actor_id.is_empty()
            && actor_id.len() <= crate::model::MAX_REVIEW_ACTOR_ID_LENGTH
            && !actor_id.chars().any(char::is_control))
        .then_some(())
        .ok_or(AlloyAuthoringError::Invalid)
    }

    fn require_owned(&self, script: &Script) -> Result<(), AlloyAuthoringError> {
        (self.owns(script))
            .then_some(())
            .ok_or(AlloyAuthoringError::NotFound)
    }

    fn require_review_owned(&self, decision: &ReviewDecision) -> Result<(), AlloyAuthoringError> {
        (decision.tenant_id == self.tenant_id)
            .then_some(())
            .ok_or(AlloyAuthoringError::NotFound)
    }

    fn require_test_owned(&self, run: &TestRun) -> Result<(), AlloyAuthoringError> {
        (run.tenant_id == self.tenant_id)
            .then_some(())
            .ok_or(AlloyAuthoringError::NotFound)
    }

    fn require_retention_owned(
        &self,
        state: &ScriptEvidenceRetentionState,
    ) -> Result<(), AlloyAuthoringError> {
        (state.tenant_id == self.tenant_id)
            .then_some(())
            .ok_or(AlloyAuthoringError::NotFound)
    }

    fn require_expected_revision(
        &self,
        script: &Script,
        expected_version: u32,
    ) -> Result<(), AlloyAuthoringError> {
        (script.version == expected_version)
            .then_some(())
            .ok_or(AlloyAuthoringError::RevisionConflict { expected_version })
    }

    fn owns(&self, script: &Script) -> bool {
        script.tenant_id == self.tenant_id
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AlloyAuthoringError {
    #[error("Alloy script was not found")]
    NotFound,
    #[error("Alloy script revision conflict")]
    RevisionConflict { expected_version: u32 },
    #[error("Alloy evidence retention revision conflict")]
    RetentionRevisionConflict { expected_retention_revision: u32 },
    #[error("Alloy authoring command is invalid")]
    Invalid,
    #[error("Alloy authoring operation failed")]
    Failed,
}

impl AlloyAuthoringError {
    fn from_script_error(error: ScriptError) -> Self {
        match error {
            ScriptError::NotFound { .. } => Self::NotFound,
            ScriptError::RevisionConflict { expected } => Self::RevisionConflict {
                expected_version: expected,
            },
            ScriptError::RetentionRevisionConflict { expected } => {
                Self::RetentionRevisionConflict {
                    expected_retention_revision: expected,
                }
            }
            ScriptError::Compilation(_)
            | ScriptError::InvalidTrigger(_)
            | ScriptError::InvalidStatus(_)
            | ScriptError::InvalidWorkspace(_)
            | ScriptError::InvalidLineage(_)
            | ScriptError::ImportIdempotencyConflict
            | ScriptError::ImportDraftNameConflict
            | ScriptError::Deletion(_)
            | ScriptError::EvidenceRetention(_)
            | ScriptError::Review(_)
            | ScriptError::TestRun(_) => Self::Invalid,
            ScriptError::Runtime(_)
            | ScriptError::Aborted(_)
            | ScriptError::Timeout { .. }
            | ScriptError::OperationLimit { .. }
            | ScriptError::ResourceLimit { .. }
            | ScriptError::MaxDepthExceeded { .. }
            | ScriptError::Storage(_)
            | ScriptError::Release(_) => Self::Failed,
        }
    }
}

/// A source-bearing command accepted only at an authenticated owner boundary.
/// Responses use the redacted shapes below and never echo this workspace.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAlloyScriptCommand {
    pub name: String,
    pub description: Option<String>,
    pub workspace: RhaiWorkspace,
    pub trigger: ScriptTrigger,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub run_as_system: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateAlloyScriptCommand {
    pub script_id: Uuid,
    pub expected_version: u32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub workspace: Option<RhaiWorkspace>,
    pub trigger: Option<ScriptTrigger>,
    pub status: Option<ScriptStatus>,
    pub run_as_system: Option<bool>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteAlloyScriptCommand {
    pub script_id: Uuid,
    pub expected_version: u32,
    pub reason: String,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetAlloyDeletedEvidenceRetentionCommand {
    pub script_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeAlloyDeletedEvidenceRetentionCommand {
    pub script_id: Uuid,
    pub deletion_request_digest: String,
    pub expected_retention_revision: u32,
    pub action: ScriptEvidenceRetentionAction,
    pub reason: String,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetAlloyScriptCommand {
    pub script_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListAlloyScriptsCommand {
    pub status: Option<ScriptStatus>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl ListAlloyScriptsCommand {
    fn normalized_page(&self) -> u32 {
        self.page.unwrap_or(1).max(1)
    }

    fn normalized_per_page(&self) -> u32 {
        self.per_page.unwrap_or(20).clamp(1, 100)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListAlloyScriptRevisionsCommand {
    pub script_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidateAlloyScriptCommand {
    pub name: String,
    pub workspace: RhaiWorkspace,
    pub trigger: ScriptTrigger,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAlloyScriptCommand {
    pub script_id: Uuid,
    pub expected_version: u32,
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
    pub entity: Option<AuthoringEntityInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringEntityInput {
    pub id: String,
    pub entity_type: String,
    pub data: HashMap<String, serde_json::Value>,
}

impl AuthoringEntityInput {
    fn into_proxy(self) -> EntityProxy {
        EntityProxy::new(
            self.id,
            self.entity_type,
            self.data
                .into_iter()
                .map(|(key, value)| (key, json_to_dynamic(value)))
                .collect(),
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewAlloyScriptCommand {
    pub script_id: Uuid,
    pub expected_version: u32,
    pub status: ReviewStatus,
    pub policy_revision: String,
    pub reason: Option<String>,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListAlloyScriptReviewsCommand {
    pub script_id: Uuid,
    pub revision: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAlloyWorkspaceTestCommand {
    pub script_id: Uuid,
    pub expected_version: u32,
    pub test_path: String,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeAlloyScriptLifecycleCommand {
    pub script_id: Uuid,
    pub expected_version: u32,
    pub action: AlloyScriptLifecycleAction,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlloyScriptLifecycleAction {
    Activate,
    Pause,
    Disable,
    Archive,
    ResetErrors,
}

/// Source-redacted script metadata. In particular it has no `workspace`,
/// source file, test body, or compiler/runtime diagnostic field.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RedactedAlloyScript {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub trigger: ScriptTrigger,
    pub status: ScriptStatus,
    pub version: u32,
    pub run_as_system: bool,
    pub permissions: Vec<String>,
    pub parent_release: Option<rustok_modules::ArtifactReleaseRef>,
    pub source_digest: String,
    pub error_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<Script> for RedactedAlloyScript {
    type Error = AlloyAuthoringError;

    fn try_from(script: Script) -> Result<Self, Self::Error> {
        Ok(Self {
            id: script.id,
            name: script.name,
            description: script.description,
            trigger: script.trigger,
            status: script.status,
            version: script.version,
            run_as_system: script.run_as_system,
            permissions: script.permissions,
            parent_release: script.parent_release,
            source_digest: workspace_digest(&script.workspace)?,
            error_count: script.error_count,
            created_at: script.created_at.to_rfc3339(),
            updated_at: script.updated_at.to_rfc3339(),
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RedactedAlloyScriptPage {
    pub scripts: Vec<RedactedAlloyScript>,
    pub total: usize,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RedactedAlloySourceRevision {
    pub script_id: Uuid,
    pub revision: u32,
    pub parent_revision: Option<u32>,
    pub source_digest: String,
    pub parent_release: Option<rustok_modules::ArtifactReleaseRef>,
    pub created_at: String,
}

impl TryFrom<crate::ScriptSourceRevision> for RedactedAlloySourceRevision {
    type Error = AlloyAuthoringError;

    fn try_from(revision: crate::ScriptSourceRevision) -> Result<Self, Self::Error> {
        Ok(Self {
            script_id: revision.script_id,
            revision: revision.revision,
            parent_revision: revision.parent_revision,
            source_digest: revision.source_digest,
            parent_release: revision.parent_release,
            created_at: revision.created_at.to_rfc3339(),
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AlloyScriptValidation {
    pub valid: bool,
    pub source_digest: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeletedAlloyScript {
    pub deleted: bool,
}

/// The retention state is intentionally source-free and omits actor and reason
/// fields from the durable audit record.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RedactedAlloyEvidenceRetention {
    pub script_id: Uuid,
    pub deletion_request_digest: String,
    pub policy: rustok_core::RetentionPolicy,
    pub retain_until: Option<String>,
    pub retention_revision: u32,
}

impl From<ScriptEvidenceRetentionState> for RedactedAlloyEvidenceRetention {
    fn from(state: ScriptEvidenceRetentionState) -> Self {
        Self {
            script_id: state.script_id,
            deletion_request_digest: state.deletion_request_digest,
            policy: state.policy,
            retain_until: state.retain_until.map(|value| value.to_rfc3339()),
            retention_revision: state.retention_revision,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlloyExecutionOutcome {
    Succeeded,
    Aborted,
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RedactedAlloyExecution {
    pub execution_id: Uuid,
    pub script_id: Uuid,
    pub success: bool,
    pub outcome: AlloyExecutionOutcome,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RedactedAlloyReview {
    pub id: Uuid,
    pub script_id: Uuid,
    pub revision: u32,
    pub source_digest: String,
    pub status: ReviewStatus,
    pub policy_revision: String,
    pub idempotency_key: Uuid,
    pub created_at: String,
}

impl From<ReviewDecision> for RedactedAlloyReview {
    fn from(decision: ReviewDecision) -> Self {
        Self {
            id: decision.id,
            script_id: decision.script_id,
            revision: decision.revision,
            source_digest: decision.source_digest,
            status: decision.status,
            policy_revision: decision.policy_revision,
            idempotency_key: decision.idempotency_key,
            created_at: decision.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RedactedAlloyTestRun {
    pub id: Uuid,
    pub script_id: Uuid,
    pub revision: u32,
    pub source_digest: String,
    pub test_path: String,
    pub status: TestRunStatus,
    pub passed: Option<bool>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl From<TestRun> for RedactedAlloyTestRun {
    fn from(run: TestRun) -> Self {
        Self {
            id: run.id,
            script_id: run.script_id,
            revision: run.revision,
            source_digest: run.source_digest,
            test_path: run.test_path,
            status: run.status,
            passed: run.passed,
            created_at: run.created_at.to_rfc3339(),
            completed_at: run.completed_at.map(|time| time.to_rfc3339()),
        }
    }
}

fn workspace_digest(workspace: &RhaiWorkspace) -> Result<String, AlloyAuthoringError> {
    workspace.digest().map_err(|_| AlloyAuthoringError::Failed)
}

fn total_pages(total: usize, per_page: u32) -> u32 {
    ((total as u64).div_ceil(per_page as u64)).min(u32::MAX as u64) as u32
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sea_orm::Database;
    use sea_orm_migration::prelude::SchemaManager;

    use super::*;
    use crate::{InMemoryStorage, ScriptRegistry};

    fn service(
        tenant_id: Uuid,
        registry: Arc<InMemoryStorage>,
    ) -> AlloyAuthoringService<InMemoryStorage> {
        AlloyAuthoringService::new(
            tenant_id,
            Arc::new(crate::create_default_engine()),
            crate::create_test_alloy_draft_runtime(),
            registry,
        )
    }

    fn command(name: &str, source: &str) -> CreateAlloyScriptCommand {
        CreateAlloyScriptCommand {
            name: name.to_string(),
            description: Some("Tenant-owned draft".to_string()),
            workspace: RhaiWorkspace::single_source(source),
            trigger: ScriptTrigger::Manual,
            permissions: Vec::new(),
            run_as_system: false,
        }
    }

    #[tokio::test]
    async fn tenant_scoped_authoring_hides_source_and_rejects_cross_tenant_reads() {
        let registry = Arc::new(InMemoryStorage::new());
        let first_tenant = Uuid::new_v4();
        let second_tenant = Uuid::new_v4();
        let first = service(first_tenant, registry.clone());
        let second = service(second_tenant, registry.clone());
        let created = first
            .create_script(
                "mcp-client-a",
                command("private_rule", "let secret = 42; secret"),
            )
            .await
            .expect("first tenant may create its script");

        let serialized = serde_json::to_string(&created).expect("redacted script serializes");
        assert!(!serialized.contains("let secret = 42"));
        assert!(!serialized.contains("workspace"));
        assert_eq!(
            first
                .list_scripts(ListAlloyScriptsCommand {
                    status: None,
                    page: None,
                    per_page: None
                })
                .await
                .expect("first tenant list")
                .total,
            1
        );
        assert_eq!(
            second
                .list_scripts(ListAlloyScriptsCommand {
                    status: None,
                    page: None,
                    per_page: None
                })
                .await
                .expect("second tenant list")
                .total,
            0
        );
        let revision = registry
            .get_source_revision(created.id, created.version)
            .await
            .expect("remote MCP source revision should persist");
        assert_eq!(
            revision.source_provenance,
            crate::SourceProvenance::remote_mcp("alloy_create_script")
        );
        assert_eq!(
            second
                .get_script(GetAlloyScriptCommand {
                    script_id: created.id
                })
                .await
                .expect_err("cross-tenant script read must fail closed"),
            AlloyAuthoringError::NotFound
        );
        assert!(
            registry.get(created.id).await.is_ok(),
            "the fixture proves the denial is tenant enforcement, not a missing row"
        );
    }

    #[test]
    fn validation_never_echoes_source_or_compiler_diagnostics() {
        let tenant_id = Uuid::new_v4();
        let service = service(tenant_id, Arc::new(InMemoryStorage::new()));
        let result = service
            .validate_script(ValidateAlloyScriptCommand {
                name: "invalid_rule".to_string(),
                workspace: RhaiWorkspace::single_source("let secret = ;"),
                trigger: ScriptTrigger::Manual,
            })
            .expect("syntactically invalid but structurally valid source has redacted validation");
        let serialized = serde_json::to_string(&result).expect("validation serializes");
        assert!(!result.valid);
        assert!(!serialized.contains("let secret"));
        assert!(!serialized.contains("message"));
    }

    #[tokio::test]
    async fn remote_delete_replays_only_the_same_attributed_command() {
        let tenant_id = Uuid::new_v4();
        let service = service(tenant_id, Arc::new(InMemoryStorage::new()));
        let created = service
            .create_script("mcp-delete", command("deletable_rule", "40 + 2"))
            .await
            .expect("remote MCP author may create a draft");
        let deletion = DeleteAlloyScriptCommand {
            script_id: created.id,
            expected_version: created.version,
            reason: "The draft was superseded by a reviewed replacement.".into(),
            idempotency_key: Uuid::new_v4(),
        };

        service
            .delete_script("mcp-client", deletion.clone())
            .await
            .expect("first owner deletion should succeed");
        service
            .delete_script("mcp-client", deletion.clone())
            .await
            .expect("the exact remote MCP retry should replay from the tombstone");

        let mut conflicting_replay = deletion;
        conflicting_replay.reason = "A different retention reason.".into();
        assert_eq!(
            service
                .delete_script("mcp-client", conflicting_replay)
                .await
                .expect_err("a changed command must not replay"),
            AlloyAuthoringError::Invalid
        );
    }

    #[tokio::test]
    async fn remote_retention_commands_are_tenant_bound_and_source_free() {
        let registry = Arc::new(InMemoryStorage::new());
        let owner_tenant = Uuid::new_v4();
        let other_tenant = Uuid::new_v4();
        let owner = service(owner_tenant, registry.clone());
        let other = service(other_tenant, registry);
        let created = owner
            .create_script("mcp-retention", command("retained_rule", "40 + 2"))
            .await
            .expect("owner should create a draft");
        owner
            .delete_script(
                "mcp-owner",
                DeleteAlloyScriptCommand {
                    script_id: created.id,
                    expected_version: created.version,
                    reason: "The draft was superseded by a reviewed replacement.".into(),
                    idempotency_key: Uuid::new_v4(),
                },
            )
            .await
            .expect("owner should delete its draft");
        let retention = owner
            .get_deleted_evidence_retention(GetAlloyDeletedEvidenceRetentionCommand {
                script_id: created.id,
            })
            .await
            .expect("owner should read source-free retention state");
        let serialized = serde_json::to_string(&retention).expect("retention state serializes");
        assert!(!serialized.contains("mcp-owner"));
        assert!(!serialized.contains("superseded"));

        let command = ChangeAlloyDeletedEvidenceRetentionCommand {
            script_id: created.id,
            deletion_request_digest: retention.deletion_request_digest.clone(),
            expected_retention_revision: retention.retention_revision,
            action: ScriptEvidenceRetentionAction::ApplyLegalHold,
            reason: "A legal investigation requires preservation.".into(),
            idempotency_key: Uuid::new_v4(),
        };
        assert_eq!(
            other
                .change_deleted_evidence_retention("mcp-other", command.clone())
                .await
                .expect_err("another tenant must not change retained evidence"),
            AlloyAuthoringError::NotFound
        );
        let held = owner
            .change_deleted_evidence_retention("mcp-owner", command)
            .await
            .expect("owner should apply legal hold");
        assert_eq!(held.policy, rustok_core::RetentionPolicy::LegalHold);
        assert_eq!(held.retain_until, None);
    }

    #[tokio::test]
    async fn production_scoped_storage_rejects_cross_tenant_authoring() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database should connect");
        let manager = SchemaManager::new(&database);
        for migration in crate::migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("Alloy migrations should apply");
        }
        let owner_tenant = Uuid::new_v4();
        let other_tenant = Uuid::new_v4();
        let owner_storage =
            Arc::new(crate::SeaOrmStorage::new(database.clone()).for_tenant(owner_tenant));
        let other_storage = Arc::new(crate::SeaOrmStorage::new(database).for_tenant(other_tenant));
        let owner = AlloyAuthoringService::new(
            owner_tenant,
            Arc::new(crate::create_default_engine()),
            crate::create_test_alloy_draft_runtime(),
            owner_storage,
        );
        let other = AlloyAuthoringService::new(
            other_tenant,
            Arc::new(crate::create_default_engine()),
            crate::create_test_alloy_draft_runtime(),
            other_storage,
        );

        let created = owner
            .create_script("remote-mcp-owner", command("production_private", "41 + 1"))
            .await
            .expect("owner tenant should create its draft");
        assert_eq!(
            other
                .update_script(
                    "remote-mcp-other",
                    UpdateAlloyScriptCommand {
                        script_id: created.id,
                        expected_version: created.version,
                        name: Some("attempted_takeover".to_string()),
                        description: None,
                        workspace: None,
                        trigger: None,
                        status: None,
                        run_as_system: None,
                        permissions: None,
                    },
                )
                .await
                .expect_err("other tenant must not update owner draft"),
            AlloyAuthoringError::NotFound
        );
        assert_eq!(
            owner
                .get_script(GetAlloyScriptCommand {
                    script_id: created.id
                })
                .await
                .expect("owner draft remains available")
                .name,
            "production_private"
        );
    }
}
